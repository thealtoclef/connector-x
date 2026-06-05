//! Source implementation for Google Spanner

mod errors;
mod typesystem;

pub use self::errors::SpannerSourceError;
use crate::{
    data_order::DataOrder,
    errors::ConnectorXError,
    sources::{PartitionParser, Produce, Source, SourcePartition},
    sql::{count_query, limit1_query, CXQuery},
};
use anyhow::anyhow;
use chrono::{DateTime, NaiveDate, Utc};
use fehler::{throw, throws};
use google_cloud_spanner::{
    client::{DatabaseClient, Spanner},
    model::PartitionOptions,
    statement::Statement,
};
use log::debug;
use sqlparser::dialect::Dialect;
use std::sync::Arc;
use tokio::runtime::Runtime;
pub use typesystem::SpannerTypeSystem;
use url::Url;

#[derive(Debug)]
pub struct SpannerDialect {}

impl Dialect for SpannerDialect {
    // Spanner uses GoogleSQL which uses backtick for identifiers
    fn is_delimited_identifier_start(&self, ch: char) -> bool {
        ch == '`'
    }

    fn is_identifier_start(&self, ch: char) -> bool {
        ch.is_ascii_lowercase() || ch.is_ascii_uppercase() || ch == '_'
    }

    fn is_identifier_part(&self, ch: char) -> bool {
        self.is_identifier_start(ch) || ch.is_ascii_digit()
    }
}

#[derive(Debug)]
pub struct SpannerSource {
    rt: Arc<Runtime>,
    db_client: DatabaseClient,
    origin_query: Option<String>,
    queries: Vec<CXQuery<String>>,
    names: Vec<String>,
    schema: Vec<SpannerTypeSystem>,
    data_boost: bool,
    partition_query: bool,
}

impl SpannerSource {
    #[throws(SpannerSourceError)]
    pub fn new(rt: Arc<Runtime>, conn: &str) -> Self {
        let url = Url::parse(conn)?;

        // Parse database path from URL
        // Format: spanner://projects/{project}/instances/{instance}/databases/{database}
        // Url parser treats "projects" as the host, so reconstruct the full resource path
        let database_path = format!(
            "{}{}",
            url.host_str().unwrap_or(""),
            url.path()
        );

        // Check for data_boost query parameter
        let data_boost = url
            .query_pairs()
            .find(|(k, _)| k == "data_boost")
            .map(|(_, v)| v == "true")
            .unwrap_or(false);

        // Check for partition_query query parameter
        // When false, skip partition_query() and use single_use() + execute_query() instead.
        // This is needed for non-root-partitionable queries (e.g. aggregates).
        let partition_query = url
            .query_pairs()
            .find(|(k, _)| k == "partition_query")
            .map(|(_, v)| v != "false")
            .unwrap_or(true);

        // Create Spanner client and DatabaseClient
        let spanner = rt.block_on(Spanner::builder().build())
            .map_err(|e| anyhow::anyhow!("Failed to create Spanner client: {}", e))?;
        let db_client = rt.block_on(
            spanner.database_client(&database_path).build()
        ).map_err(|e| anyhow::anyhow!("Failed to create DatabaseClient: {}", e))?;

        debug!("Spanner client created for database: {}", database_path);

        Self {
            rt,
            db_client,
            origin_query: None,
            queries: vec![],
            names: vec![],
            schema: vec![],
            data_boost,
            partition_query,
        }
    }
}

impl Source for SpannerSource
where
    SpannerSourcePartition:
        SourcePartition<TypeSystem = SpannerTypeSystem, Error = SpannerSourceError>,
{
    const DATA_ORDERS: &'static [DataOrder] = &[DataOrder::RowMajor];
    type Partition = SpannerSourcePartition;
    type TypeSystem = SpannerTypeSystem;
    type Error = SpannerSourceError;

    #[throws(SpannerSourceError)]
    fn set_data_order(&mut self, data_order: DataOrder) {
        if !matches!(data_order, DataOrder::RowMajor) {
            throw!(ConnectorXError::UnsupportedDataOrder(data_order));
        }
    }

    fn set_queries<Q: ToString>(&mut self, queries: &[CXQuery<Q>]) {
        self.queries = queries.iter().map(|q| q.map(Q::to_string)).collect();
    }

    fn set_origin_query(&mut self, query: Option<String>) {
        self.origin_query = query;
    }

    #[throws(SpannerSourceError)]
    fn fetch_metadata(&mut self) {
        assert!(!self.queries.is_empty());
        let query = &self.queries[0];
        let l1query = limit1_query(query, &SpannerDialect {})?;

        // Use single_use() for metadata queries (not batch_read_only_transaction)
        // single_use() supports execute_query directly
        let tx = self.db_client.single_use().build();
        let stmt = Statement::builder(l1query.as_str()).build();
        let rs = self.rt.block_on(tx.execute_query(stmt))?;

        // Extract column names and types from metadata
        if let Some(metadata) = rs.metadata() {
            let column_types = metadata.column_types();
            self.names = metadata.column_names().to_vec();
            self.schema = metadata
                .column_names()
                .iter()
                .zip(column_types.iter())
                .map(|(_, col_type)| {
                    SpannerTypeSystem::from_spanner_type_code(&col_type.code(), true)
                })
                .collect();
        }
    }

    #[throws(SpannerSourceError)]
    fn result_rows(&mut self) -> Option<usize> {
        match &self.origin_query {
            Some(q) => {
                let cxq = CXQuery::Naked(q.clone());
                let cquery = count_query(&cxq, &SpannerDialect {})?;

                // Use single_use() for count queries (not batch_read_only_transaction)
                // single_use() supports execute_query directly
                let tx = self.db_client.single_use().build();
                let stmt = Statement::builder(cquery.as_str()).build();
                let mut rs = self.rt.block_on(tx.execute_query(stmt))?;

                if let Some(row) = self.rt.block_on(rs.next()).transpose()? {
                    let nrows: i64 = row.try_get(0)?;
                    Some(nrows as usize)
                } else {
                    None
                }
            }
            None => None,
        }
    }

    fn names(&self) -> Vec<String> {
        self.names.clone()
    }

    fn schema(&self) -> Vec<Self::TypeSystem> {
        self.schema.clone()
    }

    #[throws(SpannerSourceError)]
    fn partition(self) -> Vec<Self::Partition> {
        if self.partition_query {
            // batch_read_only_transaction + partition_query (parallel)
            let read_tx = self.rt.block_on(
                self.db_client.batch_read_only_transaction().build()
            )?;

            let stmt = Statement::builder(self.queries[0].as_str()).build();

            let partitions = self.rt.block_on(
                read_tx.partition_query(stmt, PartitionOptions::default())
            )?;

            debug!("Spanner returned {} partitions", partitions.len());

            partitions
                .into_iter()
                .map(|p| {
                    SpannerSourcePartition::new(
                        self.rt.clone(),
                        self.db_client.clone(),
                        p,
                        &self.schema,
                        self.data_boost,
                    )
                })
                .collect()
        } else {
            // single_use + execute_query for non-root-partitionable queries (e.g. aggregates)
            debug!("partition_query=false, using single_use transaction");
            vec![SpannerSourcePartition::new_single(
                self.rt.clone(),
                self.db_client.clone(),
                self.queries[0].as_str().to_string(),
                &self.schema,
                self.data_boost,
            )]
        }
    }
}

enum PartitionSource {
    Partition(google_cloud_spanner::batch::Partition),
    Query(String),
}

pub struct SpannerSourcePartition {
    rt: Arc<Runtime>,
    db_client: DatabaseClient,
    source: PartitionSource,
    schema: Vec<SpannerTypeSystem>,
    nrows: usize,
    ncols: usize,
    data_boost: bool,
}

impl SpannerSourcePartition {
    pub fn new(
        rt: Arc<Runtime>,
        db_client: DatabaseClient,
        partition: google_cloud_spanner::batch::Partition,
        schema: &[SpannerTypeSystem],
        data_boost: bool,
    ) -> Self {
        Self {
            rt,
            db_client,
            source: PartitionSource::Partition(partition),
            schema: schema.to_vec(),
            nrows: 0,
            ncols: schema.len(),
            data_boost,
        }
    }

    pub fn new_single(
        rt: Arc<Runtime>,
        db_client: DatabaseClient,
        query: String,
        schema: &[SpannerTypeSystem],
        data_boost: bool,
    ) -> Self {
        Self {
            rt,
            db_client,
            source: PartitionSource::Query(query),
            schema: schema.to_vec(),
            nrows: 0,
            ncols: schema.len(),
            data_boost,
        }
    }
}

impl SourcePartition for SpannerSourcePartition {
    type TypeSystem = SpannerTypeSystem;
    type Parser<'a> = SpannerSourceParser;
    type Error = SpannerSourceError;

    #[throws(SpannerSourceError)]
    fn result_rows(&mut self) {
        // No-op: parser() already sets self.nrows when it buffers all rows.
        // We avoid executing the partition twice (once to count, once to read).
        // The Dispatcher will use nrows() after parser() is called.
    }

    #[throws(SpannerSourceError)]
    fn parser(&mut self) -> Self::Parser<'_> {
        let mut rs = match &self.source {
            PartitionSource::Partition(p) => {
                debug!("Executing Spanner partition (data_boost={})", self.data_boost);
                let mut p = p.clone();
                if self.data_boost {
                    p = p.set_data_boost(true);
                }
                self.rt.block_on(p.execute(&self.db_client))?
            }
            PartitionSource::Query(q) => {
                debug!("Executing Spanner single_use query");
                let tx = self.db_client.single_use().build();
                let stmt = Statement::builder(q.as_str()).build();
                self.rt.block_on(tx.execute_query(stmt))?
            }
        };

        // Buffer all rows (same pattern as BigQuery)
        let mut rows = Vec::new();
        while let Some(row) = self.rt.block_on(rs.next()).transpose()? {
            rows.push(row);
        }

        debug!("Buffered {} rows from Spanner partition", rows.len());
        self.nrows = rows.len();
        SpannerSourceParser::new(rows, &self.schema)
    }

    fn nrows(&self) -> usize {
        self.nrows
    }

    fn ncols(&self) -> usize {
        self.ncols
    }
}

pub struct SpannerSourceParser {
    rows: Vec<google_cloud_spanner::result::Row>,
    ncols: usize,
    current_row: usize,
    current_col: usize,
}

impl SpannerSourceParser {
    fn new(
        rows: Vec<google_cloud_spanner::result::Row>,
        schema: &[SpannerTypeSystem],
    ) -> Self {
        Self {
            rows,
            ncols: schema.len(),
            current_row: 0,
            current_col: 0,
        }
    }

    #[throws(SpannerSourceError)]
    fn next_loc(&mut self) -> (usize, usize) {
        let ret = (self.current_row, self.current_col);
        self.current_row += (self.current_col + 1) / self.ncols;
        self.current_col = (self.current_col + 1) % self.ncols;
        ret
    }
}

impl<'a> PartitionParser<'a> for SpannerSourceParser {
    type TypeSystem = SpannerTypeSystem;
    type Error = SpannerSourceError;

    #[throws(SpannerSourceError)]
    fn fetch_next(&mut self) -> (usize, bool) {
        assert!(self.current_col == 0);
        (self.rows.len() - self.current_row, true)
    }
}

// Implement Produce for basic types using a macro
macro_rules! impl_produce {
    ($($t: ty,)+) => {
        $(
            impl<'r> Produce<'r, $t> for SpannerSourceParser {
                type Error = SpannerSourceError;

                #[throws(SpannerSourceError)]
                fn produce(&'r mut self) -> $t {
                    let (ridx, cidx) = self.next_loc()?;
                    let row = &self.rows[ridx];
                    row.try_get::<$t, _>(cidx)?
                }
            }

            impl<'r> Produce<'r, Option<$t>> for SpannerSourceParser {
                type Error = SpannerSourceError;

                #[throws(SpannerSourceError)]
                fn produce(&'r mut self) -> Option<$t> {
                    let (ridx, cidx) = self.next_loc()?;
                    let row = &self.rows[ridx];
                    if row.try_is_null(cidx)? {
                        None
                    } else {
                        Some(row.try_get::<$t, _>(cidx)?)
                    }
                }
            }
        )+
    };
}

impl_produce!(i64, f64, f32, String, bool, Vec<u8>,);

// Implement Produce for chrono types with conversion from time crate
impl<'r> Produce<'r, NaiveDate> for SpannerSourceParser {
    type Error = SpannerSourceError;

    #[throws(SpannerSourceError)]
    fn produce(&mut self) -> NaiveDate {
        let (ridx, cidx) = self.next_loc()?;
        let row = &self.rows[ridx];
        let d: time::Date = row.try_get(cidx)?;
        let (year, month, day) = d.to_calendar_date();
        NaiveDate::from_ymd_opt(year, month as u32, day as u32)
            .ok_or_else(|| anyhow!("invalid date: {}-{}-{}", year, month as u32, day))?
    }
}

impl<'r> Produce<'r, Option<NaiveDate>> for SpannerSourceParser {
    type Error = SpannerSourceError;

    #[throws(SpannerSourceError)]
    fn produce(&mut self) -> Option<NaiveDate> {
        let (ridx, cidx) = self.next_loc()?;
        let row = &self.rows[ridx];
        if row.try_is_null(cidx)? {
            None
        } else {
            let d: time::Date = row.try_get(cidx)?;
            let (year, month, day) = d.to_calendar_date();
            Some(
                NaiveDate::from_ymd_opt(year, month as u32, day as u32)
                    .ok_or_else(|| anyhow!("invalid date: {}-{}-{}", year, month as u32, day))?,
            )
        }
    }
}

impl<'r> Produce<'r, DateTime<Utc>> for SpannerSourceParser {
    type Error = SpannerSourceError;

    #[throws(SpannerSourceError)]
    fn produce(&mut self) -> DateTime<Utc> {
        let (ridx, cidx) = self.next_loc()?;
        let row = &self.rows[ridx];
        let odt: time::OffsetDateTime = row.try_get(cidx)?;
        // Convert time::OffsetDateTime to chrono::DateTime<Utc>
        let unix_ts = odt.unix_timestamp();
        let nanos = odt.nanosecond();
        DateTime::from_timestamp(unix_ts, nanos as u32)
            .ok_or_else(|| anyhow!("timestamp out of range"))?
    }
}

impl<'r> Produce<'r, Option<DateTime<Utc>>> for SpannerSourceParser {
    type Error = SpannerSourceError;

    #[throws(SpannerSourceError)]
    fn produce(&mut self) -> Option<DateTime<Utc>> {
        let (ridx, cidx) = self.next_loc()?;
        let row = &self.rows[ridx];
        if row.try_is_null(cidx)? {
            None
        } else {
            let odt: time::OffsetDateTime = row.try_get(cidx)?;
            let unix_ts = odt.unix_timestamp();
            let nanos = odt.nanosecond();
            Some(
                DateTime::from_timestamp(unix_ts, nanos as u32)
                    .ok_or_else(|| anyhow!("timestamp out of range"))?,
            )
        }
    }
}
