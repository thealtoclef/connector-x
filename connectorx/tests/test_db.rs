use std::{
    env, fs,
    net::{SocketAddr, TcpStream, ToSocketAddrs},
    path::PathBuf,
    sync::Once,
    thread,
    time::{Duration, Instant},
};

use testcontainers::{
    core::{CmdWaitFor, ExecCommand, IntoContainerPort, Mount, WaitFor},
    runners::SyncRunner,
    GenericImage, ImageExt,
};

static POSTGRES_INIT: Once = Once::new();
static MYSQL_INIT: Once = Once::new();
static MSSQL_INIT: Once = Once::new();
static TRINO_INIT: Once = Once::new();
static CLICKHOUSE_INIT: Once = Once::new();
static ORACLE_INIT: Once = Once::new();
static SPANNER_INIT: Once = Once::new();

fn scripts_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../scripts")
}

fn wait_for_tcp_ready(host: &str, port: u16, timeout: Duration, label: &str) {
    let deadline = Instant::now() + timeout;
    let addr_str = format!("{host}:{port}");
    let addrs: Vec<SocketAddr> = addr_str
        .to_socket_addrs()
        .expect("resolve service address")
        .collect();

    loop {
        for addr in &addrs {
            if TcpStream::connect_timeout(addr, Duration::from_secs(2)).is_ok() {
                return;
            }
        }

        if Instant::now() >= deadline {
            panic!(
                "{label} is not reachable at {host}:{port} after {:?}",
                timeout
            );
        }

        thread::sleep(Duration::from_millis(500));
    }
}

#[cfg(feature = "src_postgres")]
pub fn postgres_url() -> String {
    POSTGRES_INIT.call_once(|| {
        if env::var("POSTGRES_URL").is_ok() {
            return;
        }

        let init_script = scripts_dir().join("postgres.sql");
        let image = GenericImage::new("pgvector/pgvector", "pg17")
            .with_exposed_port(5432.tcp())
            .with_wait_for(WaitFor::message_on_stdout(
                "database system is ready to accept connections",
            ))
            .with_startup_timeout(Duration::from_secs(180))
            .with_env_var("POSTGRES_USER", "postgres")
            .with_env_var("POSTGRES_PASSWORD", "postgres")
            .with_env_var("POSTGRES_DB", "postgres")
            .with_mount(Mount::bind_mount(
                init_script.to_string_lossy().into_owned(),
                "/docker-entrypoint-initdb.d/postgres.sql".to_string(),
            ));

        let container = image.start().expect("start postgres testcontainer");
        let host = container
            .get_host()
            .expect("get postgres container host")
            .to_string();
        let port = container
            .get_host_port_ipv4(5432)
            .expect("get postgres exposed port");

        env::set_var(
            "POSTGRES_URL",
            format!("postgresql://postgres:postgres@{host}:{port}/postgres"),
        );

        // Keep the container alive for the test process lifetime.
        std::mem::forget(container);
    });

    env::var("POSTGRES_URL").expect("POSTGRES_URL must be set")
}

#[cfg(feature = "src_mysql")]
pub fn mysql_url() -> String {
    MYSQL_INIT.call_once(|| {
        if env::var("MYSQL_URL").is_ok() {
            return;
        }

        let init_script = scripts_dir().join("mysql.sql");
        let image = GenericImage::new("ghcr.io/wangxiaoying/mysql", "latest")
            .with_exposed_port(3306.tcp())
            .with_wait_for(WaitFor::message_on_stderr("ready for connections"))
            .with_startup_timeout(Duration::from_secs(180))
            .with_env_var("MYSQL_ROOT_PASSWORD", "mysql")
            .with_env_var("MYSQL_DATABASE", "mysql")
            .with_env_var("LANG", "C.UTF-8")
            .with_mount(Mount::bind_mount(
                init_script.to_string_lossy().into_owned(),
                "/docker-entrypoint-initdb.d/mysql.sql".to_string(),
            ));

        let container = image.start().expect("start mysql testcontainer");
        let host = container
            .get_host()
            .expect("get mysql container host")
            .to_string();
        let port = container
            .get_host_port_ipv4(3306)
            .expect("get mysql exposed port");

        wait_for_tcp_ready(&host, port, Duration::from_secs(180), "mysql");
        let mysql_host_for_url = if host.eq_ignore_ascii_case("localhost") {
            "127.0.0.1"
        } else {
            host.as_str()
        };

        env::set_var(
            "MYSQL_URL",
            format!("mysql://root:mysql@{mysql_host_for_url}:{port}/mysql"),
        );

        // Keep the container alive for the test process lifetime.
        std::mem::forget(container);
    });

    env::var("MYSQL_URL").expect("MYSQL_URL must be set")
}

#[cfg(feature = "src_mssql")]
pub fn mssql_url() -> String {
    MSSQL_INIT.call_once(|| {
        if env::var("MSSQL_URL").is_ok() {
            return;
        }

        let init_script = scripts_dir().join("mssql.sql");
        let patched_script = std::env::temp_dir().join("connectorx-mssql-test.sql");
        let script = fs::read_to_string(&init_script).expect("read mssql.sql");
        // sqlcmd requires CREATE FUNCTION at start of batch.
        fs::write(
            &patched_script,
            script
                .replace('\u{200B}', "")
                .replace("\nCREATE FUNCTION increment", "\nGO\n\nCREATE FUNCTION increment"),
        )
        .expect("write patched mssql sql");

        let image = GenericImage::new("mcr.microsoft.com/mssql/server", "2022-CU12-ubuntu-22.04")
            .with_exposed_port(1433.tcp())
            .with_wait_for(WaitFor::seconds(60))
            .with_startup_timeout(Duration::from_secs(180))
            .with_env_var("ACCEPT_EULA", "Y")
            .with_env_var("SA_PASSWORD", "1Secure*Password1")
            .with_env_var("SQLSERVER_USER", "SA")
            .with_env_var("SQLSERVER_DBNAME", "tempdb")
            .with_mount(Mount::bind_mount(
                patched_script.to_string_lossy().into_owned(),
                "/tmp/mssql.sql".to_string(),
            ));

        let container = image.start().expect("start mssql testcontainer");
        let mut exec = container
            .exec(
                ExecCommand::new([
                    "bash",
                    "-c",
                    "/opt/mssql-tools*/bin/sqlcmd -S localhost -U \"$SQLSERVER_USER\" -P \"$SA_PASSWORD\" -d \"$SQLSERVER_DBNAME\" -C -b -i /tmp/mssql.sql",
                ])
                .with_cmd_ready_condition(CmdWaitFor::exit_code(0)),
            )
            .expect("exec mssql init script");
        let code = exec.exit_code().expect("read mssql init exit code").unwrap_or(-1);
        if code != 0 {
            let stdout =
                String::from_utf8(exec.stdout_to_vec().unwrap_or_default()).unwrap_or_default();
            let stderr =
                String::from_utf8(exec.stderr_to_vec().unwrap_or_default()).unwrap_or_default();
            panic!("mssql init failed: {}\n{}", stdout, stderr);
        }

        let host = container
            .get_host()
            .expect("get mssql container host")
            .to_string();
        let port = container
            .get_host_port_ipv4(1433)
            .expect("get mssql exposed port");
        env::set_var(
            "MSSQL_URL",
            format!(
                "mssql://SA:1Secure%2APassword1@{host}:{port}/tempdb?trust_server_certificate=true"
            ),
        );

        std::mem::forget(container);
    });

    env::var("MSSQL_URL").expect("MSSQL_URL must be set")
}

#[cfg(feature = "src_clickhouse")]
pub fn clickhouse_url() -> String {
    CLICKHOUSE_INIT.call_once(|| {
        if env::var("CLICKHOUSE_URL").is_ok() {
            return;
        }

        let init_script = scripts_dir().join("clickhouse.sql");
        let image = GenericImage::new("clickhouse/clickhouse-server", "latest")
            .with_exposed_port(8123.tcp())
            .with_wait_for(WaitFor::seconds(30))
            .with_startup_timeout(Duration::from_secs(180))
            .with_env_var("CLICKHOUSE_USER", "default")
            .with_env_var("CLICKHOUSE_PASSWORD", "clickhouse")
            .with_mount(Mount::bind_mount(
                init_script.to_string_lossy().into_owned(),
                "/docker-entrypoint-initdb.d/clickhouse.sql".to_string(),
            ));

        let container = image.start().expect("start clickhouse testcontainer");
        let host = container
            .get_host()
            .expect("get clickhouse container host")
            .to_string();
        let port = container
            .get_host_port_ipv4(8123)
            .expect("get clickhouse exposed port");

        env::set_var(
            "CLICKHOUSE_URL",
            format!("clickhouse://default:clickhouse@{host}:{port}/default"),
        );
        std::mem::forget(container);
    });

    env::var("CLICKHOUSE_URL").expect("CLICKHOUSE_URL must be set")
}

#[cfg(feature = "src_trino")]
fn run_trino_statement(base_url: &str, statement: &str) {
    let stmt = statement.trim();
    if stmt.is_empty() || stmt.to_uppercase().starts_with("DELETE FROM") {
        return;
    }

    let mut payload: serde_json::Value = ureq::post(&format!("{base_url}/v1/statement"))
        .set("X-Trino-User", "test")
        .set("X-Trino-Catalog", "test")
        .set("X-Trino-Schema", "test")
        .send_string(stmt)
        .expect("execute trino statement")
        .into_json()
        .expect("parse trino response");

    while let Some(next) = payload.get("nextUri").and_then(|v| v.as_str()) {
        payload = ureq::get(next)
            .set("X-Trino-User", "test")
            .set("X-Trino-Catalog", "test")
            .set("X-Trino-Schema", "test")
            .call()
            .expect("poll trino query")
            .into_json()
            .expect("parse trino poll response");
        if payload.get("error").is_some() {
            panic!("trino query failed: {}", payload);
        }
    }
}

#[cfg(feature = "src_trino")]
pub fn trino_url() -> String {
    TRINO_INIT.call_once(|| {
        if env::var("TRINO_URL").is_ok() {
            return;
        }

        let catalog_file = std::env::temp_dir().join("connectorx-trino-test.properties");
        fs::write(&catalog_file, "connector.name=memory\n").expect("write trino catalog");

        let image = GenericImage::new("trinodb/trino", "latest")
            .with_exposed_port(8080.tcp())
            .with_wait_for(WaitFor::message_on_stderr("SERVER STARTED"))
            .with_startup_timeout(Duration::from_secs(180))
            .with_mount(Mount::bind_mount(
                catalog_file.to_string_lossy().into_owned(),
                "/etc/trino/catalog/test.properties".to_string(),
            ));

        let container = image.start().expect("start trino testcontainer");
        let host = container
            .get_host()
            .expect("get trino container host")
            .to_string();
        let port = container
            .get_host_port_ipv4(8080)
            .expect("get trino exposed port");
        let base_url = format!("http://{host}:{port}");
        let trino_conn = format!("trino://test@{host}:{port}/test");
        env::set_var("TRINO_URL", trino_conn);

        let init_script = scripts_dir().join("trino.sql");
        let script = fs::read_to_string(init_script).expect("read trino.sql");
        for stmt in script.split(';') {
            run_trino_statement(&base_url, stmt);
        }

        std::mem::forget(container);
    });

    env::var("TRINO_URL").expect("TRINO_URL must be set")
}

#[cfg(feature = "src_oracle")]
pub fn oracle_url() -> String {
    ORACLE_INIT.call_once(|| {
        if env::var("ORACLE_URL").is_ok() {
            return;
        }

        let init_script = fs::read_to_string(scripts_dir().join("oracle.sql")).expect("read oracle.sql");
        let cleaned_sql = init_script
            .lines()
            .filter(|line| !line.trim().to_uppercase().starts_with("DROP TABLE"))
            .collect::<Vec<_>>()
            .join("\n");

        let wrapped_sql = format!(
            "WHENEVER SQLERROR EXIT SQL.SQLCODE\n\
BEGIN EXECUTE IMMEDIATE 'DROP USER admin CASCADE'; EXCEPTION WHEN OTHERS THEN NULL; END;\n\
/\n\
CREATE USER admin IDENTIFIED BY admin;\n\
GRANT CONNECT, RESOURCE TO admin;\n\
ALTER USER admin QUOTA UNLIMITED ON USERS;\n\
CONNECT admin/admin@localhost:1521/FREEPDB1\n\
{cleaned_sql}\n\
EXIT;\n"
        );
        let script_path = std::env::temp_dir().join("connectorx-oracle-init.sql");
        fs::write(&script_path, wrapped_sql).expect("write oracle init script");

        let image = GenericImage::new("gvenzl/oracle-free", "latest")
            .with_exposed_port(1521.tcp())
            .with_wait_for(WaitFor::message_on_stdout("DATABASE IS READY TO USE"))
            .with_startup_timeout(Duration::from_secs(180))
            .with_env_var("ORACLE_PASSWORD", "oracle")
            .with_mount(Mount::bind_mount(
                script_path.to_string_lossy().into_owned(),
                "/tmp/oracle-init.sql".to_string(),
            ));

        let container = image.start().expect("start oracle testcontainer");

        let mut exec = container
            .exec(
                ExecCommand::new([
                    "bash",
                    "-lc",
                    "/opt/oracle/product/26ai/dbhomeFree/bin/sqlplus -s system/oracle@localhost:1521/FREEPDB1 @/tmp/oracle-init.sql",
                ])
                .with_cmd_ready_condition(CmdWaitFor::exit_code(0)),
            )
            .expect("exec oracle init script");
        let code = exec.exit_code().expect("read oracle init exit code").unwrap_or(-1);
        if code != 0 {
            let stdout =
                String::from_utf8(exec.stdout_to_vec().unwrap_or_default()).unwrap_or_default();
            let stderr =
                String::from_utf8(exec.stderr_to_vec().unwrap_or_default()).unwrap_or_default();
            panic!("oracle init failed: {}\n{}", stdout, stderr);
        }

        let host = container.get_host().expect("get oracle container host").to_string();
        let port = container.get_host_port_ipv4(1521).expect("get oracle exposed port");

        wait_for_tcp_ready(&host, port, Duration::from_secs(300), "oracle");

        env::set_var("ORACLE_URL", format!("oracle://admin:admin@{host}:{port}/FREEPDB1"));
        std::mem::forget(container);
    });

    env::var("ORACLE_URL").expect("ORACLE_URL must be set")
}

#[cfg(feature = "src_spanner")]
pub fn spanner_url() -> String {
    SPANNER_INIT.call_once(|| {
        if env::var("SPANNER_URL").is_ok() {
            // URL already set, setup test data
            let dburl = env::var("SPANNER_URL").unwrap();
            setup_spanner_test_data(&dburl);
            return;
        }

        // If no URL set, we can't proceed
        panic!("SPANNER_URL environment variable must be set for Spanner tests");
    });

    env::var("SPANNER_URL").expect("SPANNER_URL must be set")
}

#[cfg(feature = "src_spanner")]
const SPANNER_DROP_DDL: &[&str] = &[
    "DROP TABLE IF EXISTS test_table",
    "DROP TABLE IF EXISTS test_types",
    "DROP TABLE IF EXISTS test_str",
];

#[cfg(feature = "src_spanner")]
fn spanner_drop_tables(database_path: &str) {
    use google_cloud_lro::Poller;
    use google_cloud_spanner::client::Spanner;
    use tokio::runtime::Runtime;

    let rt = Runtime::new().unwrap();
    let spanner = rt.block_on(Spanner::builder().build())
        .expect("Failed to create Spanner client");
    let admin_client = rt.block_on(spanner.database_admin_builder().build())
        .expect("Failed to create DatabaseAdmin client");

    let stmts: Vec<String> = SPANNER_DROP_DDL.iter().map(|s| s.to_string()).collect();
    let result = rt.block_on(
        admin_client.update_database_ddl()
            .set_database(database_path)
            .set_statements(stmts)
            .poller()
            .until_done()
    );
    match result {
        Ok(_) => println!("Spanner teardown: tables dropped"),
        Err(e) => println!("Spanner teardown failed: {}", e),
    }
}

#[cfg(feature = "src_spanner")]
fn spanner_database_path(dburl: &str) -> String {
    let url = url::Url::parse(dburl).expect("Failed to parse Spanner URL");
    format!("{}{}", url.host_str().unwrap_or(""), url.path())
}

#[cfg(feature = "src_spanner")]
extern "C" fn spanner_atexit() {
    if let Ok(dburl) = env::var("SPANNER_URL") {
        let database_path = spanner_database_path(&dburl);
        spanner_drop_tables(&database_path);
    }
}

#[cfg(feature = "src_spanner")]
fn setup_spanner_test_data(dburl: &str) {
    use google_cloud_lro::Poller;
    use google_cloud_spanner::client::Spanner;
    use google_cloud_spanner::statement::Statement;
    use std::sync::Arc;
    use tokio::runtime::Runtime;

    let rt = Arc::new(Runtime::new().unwrap());
    let database_path = spanner_database_path(dburl);

    let spanner = rt.block_on(Spanner::builder().build())
        .expect("Failed to create Spanner client");
    let admin_client = rt.block_on(spanner.database_admin_builder().build())
        .expect("Failed to create DatabaseAdmin client");
    let db_client = rt.block_on(spanner.database_client(&database_path).build())
        .expect("Failed to create DatabaseClient");

    // Drop tables from any previous run
    let stmts: Vec<String> = SPANNER_DROP_DDL.iter().map(|s| s.to_string()).collect();
    let result = rt.block_on(
        admin_client.update_database_ddl()
            .set_database(&database_path)
            .set_statements(stmts)
            .poller()
            .until_done()
    );
    match result {
        Ok(_) => println!("DDL drop executed"),
        Err(e) => println!("DDL drop failed (may be expected): {}", e),
    }

    let create_ddl = vec![
        "CREATE TABLE test_table (
            test_int INT64 NOT NULL,
            test_nullint INT64,
            test_str STRING(100),
            test_float FLOAT64,
            test_bool BOOL
        ) PRIMARY KEY (test_int)".to_string(),
        "CREATE TABLE test_types (
            test_bool BOOL,
            test_date DATE,
            test_timestamp TIMESTAMP,
            test_int INT64,
            test_float FLOAT64,
            test_numeric NUMERIC,
            test_str STRING(100),
            test_bytes BYTES(MAX),
            test_json JSON
        ) PRIMARY KEY (test_int)".to_string(),
        "CREATE TABLE test_str (
            id INT64 NOT NULL,
            test_language STRING(50),
            test_hello STRING(100)
        ) PRIMARY KEY (id)".to_string(),
    ];
    rt.block_on(
        admin_client.update_database_ddl()
            .set_database(&database_path)
            .set_statements(create_ddl)
            .poller()
            .until_done()
    ).expect("Failed to create tables via DDL");
    println!("DDL create executed");

    // Register teardown for process exit
    extern "C" { fn atexit(f: extern "C" fn()) -> i32; }
    unsafe { atexit(spanner_atexit); }

    // DML: insert test data (via read-write transactions)
    let dml_statements = vec![
        "INSERT INTO test_table (test_int, test_nullint, test_str, test_float, test_bool) VALUES (1, 3, 'str1', NULL, TRUE)",
        "INSERT INTO test_table (test_int, test_nullint, test_str, test_float, test_bool) VALUES (2, NULL, 'str2', 2.2, FALSE)",
        "INSERT INTO test_table (test_int, test_nullint, test_str, test_float, test_bool) VALUES (0, 5, 'a', 3.1, NULL)",
        "INSERT INTO test_table (test_int, test_nullint, test_str, test_float, test_bool) VALUES (3, 7, 'b', 3.0, FALSE)",
        "INSERT INTO test_table (test_int, test_nullint, test_str, test_float, test_bool) VALUES (4, 9, 'c', 7.8, NULL)",
        "INSERT INTO test_table (test_int, test_nullint, test_str, test_float, test_bool) VALUES (1314, 2, NULL, -10.0, TRUE)",
        "INSERT INTO test_types (test_bool, test_date, test_timestamp, test_int, test_float, test_numeric, test_str, test_bytes, test_json) VALUES (TRUE, '1937-01-28', '1970-01-01T00:00:01Z', 1, 1.23, 1.23, '😁😂😜', B'\\x01\\x02\\x03', JSON '{\"key\": \"value\"}')",
        "INSERT INTO test_types (test_bool, test_date, test_timestamp, test_int, test_float, test_numeric, test_str, test_bytes, test_json) VALUES (NULL, '2053-07-25', NULL, 2, 234.56, 234.56, 'こんにちはЗдра́в', B'\\x04\\x05\\x06', NULL)",
        "INSERT INTO test_types (test_bool, test_date, test_timestamp, test_int, test_float, test_numeric, test_str, test_bytes, test_json) VALUES (FALSE, NULL, '2004-02-29T12:00:01.30Z', 3, NULL, NULL, NULL, NULL, JSON '[1, 2, 3]')",
        "INSERT INTO test_str (id, test_language, test_hello) VALUES (0, 'English', 'Hello')",
        "INSERT INTO test_str (id, test_language, test_hello) VALUES (1, '中文', '你好')",
        "INSERT INTO test_str (id, test_language, test_hello) VALUES (2, '日本語', 'こんにちは')",
        "INSERT INTO test_str (id, test_language, test_hello) VALUES (3, 'русский', 'Здра́вствуйте')",
        "INSERT INTO test_str (id, test_language, test_hello) VALUES (4, 'Emoji', '😁😂😜')",
        "INSERT INTO test_str (id, test_language, test_hello) VALUES (5, 'Latin1', '¥§¤®ð')",
        "INSERT INTO test_str (id, test_language, test_hello) VALUES (6, 'Extra', 'y̆')",
        "INSERT INTO test_str (id, test_language, test_hello) VALUES (7, 'Mixed', 'Ha好ち😁ðy̆')",
        "INSERT INTO test_str (id, test_language, test_hello) VALUES (8, '', NULL)",
    ];

    for stmt in dml_statements {
        let statement = Statement::builder(stmt).build();
        let result = rt.block_on(async {
            let runner = db_client.read_write_transaction().build().await?;
            runner.run(|tx: google_cloud_spanner::transaction::ReadWriteTransaction| {
                let statement = statement.clone();
                async move {
                    tx.execute_update(statement).await
                }
            }).await
        });
        match result {
            Ok(_) => println!("DML executed: {}", stmt),
            Err(e) => println!("DML failed: {} - {}", stmt, e),
        }
    }

    println!("Spanner test data setup complete");
}
