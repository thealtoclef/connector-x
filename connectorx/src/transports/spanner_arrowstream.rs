//! Transport from Spanner Source to Arrow Destination.

use crate::{
    destinations::arrowstream::{
        typesystem::{
            ArrowTypeSystem, DateTimeWrapperMicro,
        },
        ArrowDestination, ArrowDestinationError,
    },
    impl_transport,
    sources::spanner::{SpannerSource, SpannerSourceError, SpannerTypeSystem},
    typesystem::TypeConversion,
};
use chrono::{DateTime, NaiveDate, Utc};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SpannerArrowStreamTransportError {
    #[error(transparent)]
    Source(#[from] SpannerSourceError),

    #[error(transparent)]
    Destination(#[from] ArrowDestinationError),

    #[error(transparent)]
    ConnectorX(#[from] crate::errors::ConnectorXError),
}

/// Convert Spanner data types to Arrow data types.
pub struct SpannerArrowStreamTransport;

impl_transport!(
    name = SpannerArrowStreamTransport,
    error = SpannerArrowStreamTransportError,
    systems = SpannerTypeSystem => ArrowTypeSystem,
    route = SpannerSource => ArrowDestination,
    mappings = {
        { Bool[bool]                 => Boolean[bool]             | conversion auto }
        { Int64[i64]                 => Int64[i64]                | conversion auto }
        { Float64[f64]               => Float64[f64]              | conversion auto }
        { Float32[f32]               => Float32[f32]              | conversion auto }
        { String[String]             => LargeUtf8[String]         | conversion auto }
        { Bytes[Vec<u8>]             => LargeBinary[Vec<u8>]      | conversion auto }
        { Date[NaiveDate]            => Date32[NaiveDate]         | conversion auto }
        { Timestamp[DateTime<Utc>]   => DateTimeTzMicro[DateTimeWrapperMicro]  | conversion option }
        { Numeric[f64]               => Float64[f64]              | conversion none }
        { Json[String]               => LargeUtf8[String]         | conversion none }
        { Uuid[String]               => LargeUtf8[String]         | conversion none }
        { Interval[String]           => LargeUtf8[String]         | conversion none }
        { Array[String]              => LargeUtf8[String]         | conversion none }
        { Struct[String]             => LargeUtf8[String]         | conversion none }
    }
);

impl TypeConversion<DateTime<Utc>, DateTimeWrapperMicro> for SpannerArrowStreamTransport {
    fn convert(val: DateTime<Utc>) -> DateTimeWrapperMicro {
        DateTimeWrapperMicro(val)
    }
}
