use crate::errors::ConnectorXPythonError;
use crate::pandas::destination::PandasDestination;
use crate::pandas::typesystem::{DateTimeWrapperMicro, PandasTypeSystem};
use chrono::{DateTime, NaiveDate, Utc};
use connectorx::{
    impl_transport,
    sources::spanner::{SpannerSource, SpannerTypeSystem},
    typesystem::TypeConversion,
};

#[allow(dead_code)]
pub struct SpannerPandasTransport<'py>(&'py ());

impl_transport!(
    name = SpannerPandasTransport<'tp>,
    error = ConnectorXPythonError,
    systems = SpannerTypeSystem => PandasTypeSystem,
    route = SpannerSource => PandasDestination<'tp>,
    mappings = {
        { Bool[bool]                 => Bool[bool]              | conversion auto }
        { Int64[i64]                 => I64[i64]                | conversion auto }
        { Float64[f64]               => F64[f64]                | conversion auto }
        { Float32[f32]               => F64[f64]                | conversion option }
        { Numeric[f64]               => F64[f64]                | conversion none }
        { String[String]             => String[String]          | conversion auto }
        { Bytes[Vec<u8>]             => Bytes[Vec<u8>]          | conversion auto }
        { Date[NaiveDate]            => DateTimeMicro[DateTimeWrapperMicro] | conversion option }
        { Timestamp[DateTime<Utc>]   => DateTimeMicro[DateTimeWrapperMicro] | conversion option }
        { Json[String]               => String[String]          | conversion none }
        { Uuid[String]               => String[String]          | conversion none }
        { Interval[String]           => String[String]          | conversion none }
        { Array[String]              => String[String]          | conversion none }
        { Struct[String]             => String[String]          | conversion none }
    }
);

impl<'py> TypeConversion<f32, f64> for SpannerPandasTransport<'py> {
    fn convert(val: f32) -> f64 {
        val as f64
    }
}

impl<'py> TypeConversion<NaiveDate, DateTimeWrapperMicro> for SpannerPandasTransport<'py> {
    fn convert(val: NaiveDate) -> DateTimeWrapperMicro {
        DateTimeWrapperMicro(DateTime::from_naive_utc_and_offset(
            val.and_hms_opt(0, 0, 0)
                .unwrap_or_else(|| panic!("and_hms_opt got None from {:?}", val)),
            Utc,
        ))
    }
}

impl<'py> TypeConversion<DateTime<Utc>, DateTimeWrapperMicro> for SpannerPandasTransport<'py> {
    fn convert(val: DateTime<Utc>) -> DateTimeWrapperMicro {
        DateTimeWrapperMicro(val)
    }
}
