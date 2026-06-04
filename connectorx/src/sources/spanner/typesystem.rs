use chrono::{DateTime, NaiveDate, Utc};
use google_cloud_spanner::value::TypeCode;

#[derive(Copy, Clone, Debug)]
pub enum SpannerTypeSystem {
    Bool(bool),
    Int64(bool),
    Float64(bool),
    Float32(bool),
    String(bool),
    Bytes(bool),
    Date(bool),
    Timestamp(bool),
    Numeric(bool),
    Json(bool),
    Uuid(bool),
    Interval(bool),
    Array(bool),   // ARRAY types - represented as JSON string for v1
    Struct(bool),  // STRUCT types - represented as JSON string for v1
}

impl_typesystem! {
    system = SpannerTypeSystem,
    mappings = {
        { Bool => bool }
        { Int64 => i64 }
        { Float64 | Numeric => f64 }
        { Float32 => f32 }
        { String | Json | Uuid | Interval | Array | Struct => String }
        { Bytes => Vec<u8> }
        { Date => NaiveDate }
        { Timestamp => DateTime<Utc> }
    }
}

impl SpannerTypeSystem {
    pub fn from_spanner_type_code(tc: &TypeCode, nullable: bool) -> Self {
        use SpannerTypeSystem::*;
        match tc {
            TypeCode::Bool => Bool(nullable),
            TypeCode::Int64 => Int64(nullable),
            TypeCode::Float64 => Float64(nullable),
            TypeCode::Float32 => Float32(nullable),
            TypeCode::String => String(nullable),
            TypeCode::Bytes => Bytes(nullable),
            TypeCode::Date => Date(nullable),
            TypeCode::Timestamp => Timestamp(nullable),
            TypeCode::Numeric => Numeric(nullable),
            TypeCode::Json => Json(nullable),
            TypeCode::Uuid => Uuid(nullable),
            TypeCode::Interval => Interval(nullable),
            TypeCode::Array => Array(nullable),
            TypeCode::Struct => Struct(nullable),
            _ => unimplemented!("Unsupported Spanner type: {:?}", tc),
        }
    }
}
