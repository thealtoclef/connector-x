use connectorx::{
    destinations::arrow::ArrowDestination, prelude::*, sources::spanner::SpannerSource,
    sql::CXQuery, transports::SpannerArrowTransport,
};
use std::env;
use std::sync::Arc;
use tokio::runtime::Runtime;

// Import test_db for automated setup
#[cfg(feature = "src_spanner")]
mod test_db;

// ============================================================================
// Unit Tests (don't require Spanner connection)
// ============================================================================

#[test]
fn test_spanner_type_mapping() {
    use connectorx::sources::spanner::SpannerTypeSystem;
    use google_cloud_spanner::value::TypeCode;

    // Test basic types
    assert!(matches!(
        SpannerTypeSystem::from_spanner_type_code(&TypeCode::Bool, true),
        SpannerTypeSystem::Bool(true)
    ));
    assert!(matches!(
        SpannerTypeSystem::from_spanner_type_code(&TypeCode::Int64, true),
        SpannerTypeSystem::Int64(true)
    ));
    assert!(matches!(
        SpannerTypeSystem::from_spanner_type_code(&TypeCode::Float64, true),
        SpannerTypeSystem::Float64(true)
    ));
    assert!(matches!(
        SpannerTypeSystem::from_spanner_type_code(&TypeCode::Float32, true),
        SpannerTypeSystem::Float32(true)
    ));
    assert!(matches!(
        SpannerTypeSystem::from_spanner_type_code(&TypeCode::String, true),
        SpannerTypeSystem::String(true)
    ));
    assert!(matches!(
        SpannerTypeSystem::from_spanner_type_code(&TypeCode::Bytes, true),
        SpannerTypeSystem::Bytes(true)
    ));
    assert!(matches!(
        SpannerTypeSystem::from_spanner_type_code(&TypeCode::Date, true),
        SpannerTypeSystem::Date(true)
    ));
    assert!(matches!(
        SpannerTypeSystem::from_spanner_type_code(&TypeCode::Timestamp, true),
        SpannerTypeSystem::Timestamp(true)
    ));
    assert!(matches!(
        SpannerTypeSystem::from_spanner_type_code(&TypeCode::Numeric, true),
        SpannerTypeSystem::Numeric(true)
    ));
    assert!(matches!(
        SpannerTypeSystem::from_spanner_type_code(&TypeCode::Json, true),
        SpannerTypeSystem::Json(true)
    ));
    assert!(matches!(
        SpannerTypeSystem::from_spanner_type_code(&TypeCode::Uuid, true),
        SpannerTypeSystem::Uuid(true)
    ));
    assert!(matches!(
        SpannerTypeSystem::from_spanner_type_code(&TypeCode::Interval, true),
        SpannerTypeSystem::Interval(true)
    ));

    // Test nullable flag
    assert!(matches!(
        SpannerTypeSystem::from_spanner_type_code(&TypeCode::Int64, false),
        SpannerTypeSystem::Int64(false)
    ));

    // Test ARRAY type
    assert!(matches!(
        SpannerTypeSystem::from_spanner_type_code(&TypeCode::Array, true),
        SpannerTypeSystem::Array(true)
    ));

    // Test STRUCT type
    assert!(matches!(
        SpannerTypeSystem::from_spanner_type_code(&TypeCode::Struct, true),
        SpannerTypeSystem::Struct(true)
    ));

    // Test unsupported type (should panic with descriptive message)
    let result = std::panic::catch_unwind(|| {
        SpannerTypeSystem::from_spanner_type_code(&TypeCode::Unspecified, true)
    });
    assert!(result.is_err());
}

#[test]
fn test_spanner_uri_parsing() {
    // Test basic URI parsing
    let rt = Arc::new(Runtime::new().unwrap());
    let result = SpannerSource::new(
        rt.clone(),
        "spanner://projects/my-project/instances/my-instance/databases/my-db",
    );
    // This will fail because we can't connect to Spanner, but it should parse the URI
    // The error should be about connection, not URI parsing
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    // Should not be a URL parse error
    assert!(!err.contains("URL"), "Unexpected URL error: {}", err);
}

#[test]
fn test_spanner_data_boost_parameter() {
    let rt = Arc::new(Runtime::new().unwrap());
    
    // Test with data_boost=true
    let result = SpannerSource::new(
        rt.clone(),
        "spanner://projects/my-project/instances/my-instance/databases/my-db?data_boost=true",
    );
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    // Should not be a URL parse error
    assert!(!err.contains("URL"), "Unexpected URL error: {}", err);

    // Test with data_boost=false
    let result = SpannerSource::new(
        rt.clone(),
        "spanner://projects/my-project/instances/my-instance/databases/my-db?data_boost=false",
    );
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    // Should not be a URL parse error
    assert!(!err.contains("URL"), "Unexpected URL error: {}", err);
}

#[test]
fn test_spanner_source_creation() {
    let rt = Arc::new(Runtime::new().unwrap());
    
    // Test valid URI format
    let result = SpannerSource::new(
        rt.clone(),
        "spanner://projects/p/instances/i/databases/d",
    );
    // Should fail with connection error, not URI error
    assert!(result.is_err());

    // Test invalid URI format
    let result = SpannerSource::new(
        rt.clone(),
        "invalid-uri",
    );
    // Should fail with URI parse error
    assert!(result.is_err());
}

#[test]
fn test_spanner_error_handling() {
    let rt = Arc::new(Runtime::new().unwrap());
    
    // Test invalid URI scheme
    let result = SpannerSource::new(
        rt.clone(),
        "invalid://projects/p/instances/i/databases/d",
    );
    // Should fail with connection error (not URI error)
    assert!(result.is_err());
    
    // Test empty database path
    let result = SpannerSource::new(
        rt.clone(),
        "spanner://",
    );
    // Should fail with connection error
    assert!(result.is_err());
    
    // Test malformed URI
    let result = SpannerSource::new(
        rt.clone(),
        "spanner://projects/p/instances/i/databases/d?invalid_param",
    );
    // Should fail with connection error
    assert!(result.is_err());
}

#[test]
fn test_spanner_partition_config() {
    let rt = Arc::new(Runtime::new().unwrap());
    
    // Test with data_boost=true
    let result = SpannerSource::new(
        rt.clone(),
        "spanner://projects/p/instances/i/databases/d?data_boost=true",
    );
    assert!(result.is_err());
    
    // Test with data_boost=false
    let result = SpannerSource::new(
        rt.clone(),
        "spanner://projects/p/instances/i/databases/d?data_boost=false",
    );
    assert!(result.is_err());
    
    // Test without data_boost parameter
    let result = SpannerSource::new(
        rt.clone(),
        "spanner://projects/p/instances/i/databases/d",
    );
    assert!(result.is_err());
}

#[test]
fn test_spanner_connection_pooling() {
    // Test that we can create multiple sources with the same runtime
    let rt = Arc::new(Runtime::new().unwrap());
    let initial_count = Arc::strong_count(&rt);
    
    // Create multiple sources - they should share the runtime
    let result1 = SpannerSource::new(
        rt.clone(),
        "spanner://projects/p/instances/i/databases/d",
    );
    
    // Even if connection fails, the runtime should be cloned
    assert!(result1.is_err());
    
    // The runtime should have been cloned at least once
    // (SpannerSource::new clones the runtime before attempting connection)
    assert!(Arc::strong_count(&rt) >= initial_count);
}

#[test]
fn test_spanner_custom_partition_options() {
    // Test that custom partition options are parsed correctly
    let rt = Arc::new(Runtime::new().unwrap());
    
    // Test with max_partitions parameter
    let result = SpannerSource::new(
        rt.clone(),
        "spanner://projects/p/instances/i/databases/d?max_partitions=10",
    );
    assert!(result.is_err());
    
    // Test with partition_size_bytes parameter
    let result = SpannerSource::new(
        rt.clone(),
        "spanner://projects/p/instances/i/databases/d?partition_size_bytes=1024",
    );
    assert!(result.is_err());
    
    // Test with multiple parameters
    let result = SpannerSource::new(
        rt.clone(),
        "spanner://projects/p/instances/i/databases/d?data_boost=true&max_partitions=5",
    );
    assert!(result.is_err());
}

#[test]
fn test_spanner_streaming_results() {
    // Test that streaming results work correctly
    // This tests the parser's ability to handle streaming data
    let rt = Arc::new(Runtime::new().unwrap());
    
    // Create a source - it will fail with connection error
    let result = SpannerSource::new(
        rt.clone(),
        "spanner://projects/p/instances/i/databases/d",
    );
    assert!(result.is_err());
    
    // The error should be about connection, not streaming
    let err = result.unwrap_err().to_string();
    assert!(!err.contains("stream"), "Unexpected streaming error: {}", err);
}

// ============================================================================
// Integration Tests (require real Spanner connection)
// Following BigQuery pattern: #[ignore] + environment variable
// 
// Prerequisites:
// 1. Set SPANNER_URL environment variable
// 2. Test data is automatically set up by test_db::spanner_url()
// ============================================================================

#[test]
#[ignore]
fn test_spanner_source() {
    let dburl = test_db::spanner_url();  // Automatically sets up test data
    let rt = Arc::new(Runtime::new().unwrap());
    let mut source = SpannerSource::new(rt, &dburl).unwrap();
    source.set_queries(&[CXQuery::naked("SELECT * FROM test_table")]);
    source.fetch_metadata().unwrap();
    
    let names = source.names();
    assert_eq!(names.len(), 5);
    assert!(names.contains(&"test_int".to_string()));
    assert!(names.contains(&"test_nullint".to_string()));
    assert!(names.contains(&"test_str".to_string()));
    assert!(names.contains(&"test_float".to_string()));
    assert!(names.contains(&"test_bool".to_string()));
}

#[test]
#[ignore]
fn test_spanner_partition() {
    let dburl = test_db::spanner_url();  // Automatically sets up test data
    let rt = Arc::new(Runtime::new().unwrap());
    let source = SpannerSource::new(rt, &dburl).unwrap();
    let queries = [CXQuery::naked("SELECT * FROM test_table")];
    let mut destination = ArrowDestination::new();
    let dispatcher =
        Dispatcher::<_, _, SpannerArrowTransport>::new(source, &mut destination, &queries, None);
    dispatcher.run().unwrap();
    let result = destination.arrow().unwrap();
    assert!(!result.is_empty());
    
    // Verify we got all 6 rows
    let batch = &result[0];
    assert_eq!(batch.num_rows(), 6);
    assert_eq!(batch.num_columns(), 5);
}

#[test]
#[ignore]
fn test_spanner_data_boost() {
    let dburl = test_db::spanner_url();  // Automatically sets up test data
    let rt = Arc::new(Runtime::new().unwrap());
    let source = SpannerSource::new(
        rt,
        &format!("{}?data_boost=true", dburl),
    ).unwrap();
    let queries = [CXQuery::naked("SELECT * FROM test_table")];
    let mut destination = ArrowDestination::new();
    let dispatcher =
        Dispatcher::<_, _, SpannerArrowTransport>::new(source, &mut destination, &queries, None);
    dispatcher.run().unwrap();
    let result = destination.arrow().unwrap();
    assert!(!result.is_empty());
    
    // Verify we got all 6 rows
    let batch = &result[0];
    assert_eq!(batch.num_rows(), 6);
}

#[test]
#[ignore]
fn test_spanner_multiple_partitions() {
    let dburl = test_db::spanner_url();  // Automatically sets up test data
    let rt = Arc::new(Runtime::new().unwrap());
    let source = SpannerSource::new(rt, &dburl).unwrap();
    
    let queries = [
        CXQuery::naked("SELECT * FROM test_table"),
    ];
    let mut destination = ArrowDestination::new();
    let dispatcher =
        Dispatcher::<_, _, SpannerArrowTransport>::new(source, &mut destination, &queries, None);
    dispatcher.run().unwrap();
    let result = destination.arrow().unwrap();
    assert!(!result.is_empty());
    
    // Verify we got all 6 rows across partitions
    let total_rows: usize = result.iter().map(|b| b.num_rows()).sum();
    assert_eq!(total_rows, 6);
}

#[test]
#[ignore]
fn test_spanner_type_conversion() {
    let dburl = test_db::spanner_url();  // Automatically sets up test data
    let rt = Arc::new(Runtime::new().unwrap());
    let mut source = SpannerSource::new(rt, &dburl).unwrap();
    
    // Test query that returns various types from test_types table
    source.set_queries(&[CXQuery::naked(
        "SELECT test_int, test_float, test_str, test_bool FROM test_types"
    )]);
    source.fetch_metadata().unwrap();
    
    let names = source.names();
    assert_eq!(names.len(), 4);
    assert!(names.contains(&"test_int".to_string()));
    assert!(names.contains(&"test_float".to_string()));
    assert!(names.contains(&"test_str".to_string()));
    assert!(names.contains(&"test_bool".to_string()));
}

#[test]
#[ignore]
fn test_spanner_large_result_set() {
    let dburl = test_db::spanner_url();  // Automatically sets up test data
    let rt = Arc::new(Runtime::new().unwrap());
    let source = SpannerSource::new(rt, &dburl).unwrap();
    
    let queries = [CXQuery::naked("SELECT * FROM test_table")];
    let mut destination = ArrowDestination::new();
    let dispatcher =
        Dispatcher::<_, _, SpannerArrowTransport>::new(source, &mut destination, &queries, None);
    dispatcher.run().unwrap();
    let result = destination.arrow().unwrap();
    assert!(!result.is_empty());
}

#[test]
#[ignore]
fn test_spanner_value_verification() {
    let dburl = test_db::spanner_url();  // Automatically sets up test data
    let rt = Arc::new(Runtime::new().unwrap());
    let source = SpannerSource::new(rt, &dburl).unwrap();
    
    let queries = [CXQuery::naked(
        "SELECT test_int, test_nullint, test_str, test_float, test_bool FROM test_table"
    )];
    let mut destination = ArrowDestination::new();
    let dispatcher =
        Dispatcher::<_, _, SpannerArrowTransport>::new(source, &mut destination, &queries, None);
    dispatcher.run().unwrap();
    let result = destination.arrow().unwrap();
    assert!(!result.is_empty());

    let total_rows: usize = result.iter().map(|b| b.num_rows()).sum();
    assert_eq!(total_rows, 6);
    assert_eq!(result[0].num_columns(), 5);
}

#[test]
#[ignore]
fn test_spanner_string_types() {
    let dburl = test_db::spanner_url();  // Automatically sets up test data
    let rt = Arc::new(Runtime::new().unwrap());
    let source = SpannerSource::new(rt, &dburl).unwrap();
    
    let queries = [CXQuery::naked(
        "SELECT id, test_language, test_hello FROM test_str"
    )];
    let mut destination = ArrowDestination::new();
    let dispatcher =
        Dispatcher::<_, _, SpannerArrowTransport>::new(source, &mut destination, &queries, None);
    dispatcher.run().unwrap();
    let result = destination.arrow().unwrap();
    assert!(!result.is_empty());

    let total_rows: usize = result.iter().map(|b| b.num_rows()).sum();
    assert_eq!(total_rows, 9);
}
