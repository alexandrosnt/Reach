//! End-to-end checks against real servers. Skipped unless pointed at one:
//!
//! ```text
//! REACH_TEST_PG=127.0.0.1:5432:postgres:secret:postgres
//! REACH_TEST_MYSQL=127.0.0.1:3306:root:secret:test
//! REACH_TEST_MARIADB=127.0.0.1:3307:root:secret:test
//! REACH_TEST_MSSQL=127.0.0.1:1433:sa:Secret_123:master
//! REACH_TEST_REDIS=127.0.0.1:6379::secret:0
//! cargo test --lib db::live_tests -- --ignored --test-threads=1
//! ```
//!
//! Each test creates its own table, walks it through create, edit, alter,
//! backup and restore, and drops it again.

use super::backup::{self, DumpOptions};
use super::conn::{LiveDb, META};
use super::design::{self, ColumnDef, ForeignKeyDef, IndexColumn, IndexDef, TableDesign};
use super::dialect::{self, Cell, RowEdit};
use super::introspect;
use super::redis::LiveRedis;
use super::types::{DbConnection, Engine, Route, TlsMode};

fn config(var: &str, engine: Engine) -> Option<DbConnection> {
    let raw = std::env::var(var).ok()?;
    let p: Vec<&str> = raw.splitn(5, ':').collect();
    Some(DbConnection {
        id: format!("test-{var}"),
        name: var.into(),
        engine,
        host: p.first()?.to_string(),
        port: p.get(1)?.parse().ok()?,
        username: p.get(2).unwrap_or(&"").to_string(),
        password: p.get(3).map(|s| s.to_string()).filter(|s| !s.is_empty()),
        database: p.get(4).map(|s| s.to_string()).filter(|s| !s.is_empty()),
        file_path: None,
        route: Route::Direct,
        tls: TlsMode::Prefer,
        color: None,
        read_only: false,
        production: false,
        last_used_at: 0,
    })
}

fn col(name: &str, ty: &str) -> ColumnDef {
    ColumnDef { name: name.into(), data_type: ty.into(), ..Default::default() }
}

async fn roundtrip(conn: DbConnection) {
    let e = conn.engine;
    let live = LiveDb::open(conn, None).await.expect("connect");
    let database = live.default_database();
    let schema: Option<String> = match e {
        Engine::Postgres => Some("public".into()),
        Engine::Mssql => Some("dbo".into()),
        Engine::Mysql | Engine::Mariadb => Some(database.clone()),
        _ => None,
    };
    let session = live.session(&database, META).await.unwrap();
    let mut s = session.lock().await;

    let info = introspect::server_info(e, &mut s).await.unwrap();
    println!("{e:?}: {}", info.version);

    let (int, text, ts) = match e {
        Engine::Mssql => ("int", "nvarchar", "datetime2"),
        Engine::Postgres => ("integer", "varchar", "timestamp"),
        _ => ("int", "varchar", "datetime"),
    };
    let parent_name = "reach_test_parent";
    let child_name = "reach_test_child";
    for t in [child_name, parent_name] {
        let _ = s.run(&format!("DROP TABLE {}", dialect::qualified(e, schema.as_deref(), t)), 0).await;
    }

    // Create two tables through the designer's SQL.
    let parent = TableDesign {
        schema: schema.clone(),
        name: parent_name.into(),
        columns: vec![
            ColumnDef { not_null: true, auto_increment: true, ..col("id", int) },
            ColumnDef { length: Some(50), comment: Some("Display name".into()), ..col("name", text) },
        ],
        primary_key: vec!["id".into()],
        ..Default::default()
    };
    for st in design::create_table(e, &parent).unwrap().statements {
        s.run(&st, 0).await.unwrap_or_else(|err| panic!("{st}: {err}"));
    }
    let child = TableDesign {
        schema: schema.clone(),
        name: child_name.into(),
        columns: vec![
            ColumnDef { not_null: true, ..col("id", int) },
            col("parent_id", int),
            ColumnDef { length: Some(20), default: Some("'new'".into()), ..col("status", text) },
            col("created_at", ts),
        ],
        primary_key: vec!["id".into()],
        indexes: vec![IndexDef {
            name: "ix_reach_child_status".into(),
            columns: vec![IndexColumn { name: "status".into(), ..Default::default() }],
            ..Default::default()
        }],
        foreign_keys: vec![ForeignKeyDef {
            name: "fk_reach_child_parent".into(),
            columns: vec!["parent_id".into()],
            ref_schema: if e.is_mysql_family() { None } else { schema.clone() },
            ref_table: parent_name.into(),
            ref_columns: vec!["id".into()],
            on_delete: Some("CASCADE".into()),
            ..Default::default()
        }],
        ..Default::default()
    };
    for st in design::create_table(e, &child).unwrap().statements {
        s.run(&st, 0).await.unwrap_or_else(|err| panic!("{st}: {err}"));
    }

    // Rows through the grid's edit path.
    let edits = vec![
        RowEdit::Insert { values: vec![Cell { column: "name".into(), value: Some("O'Brien \\ ünïcode".into()) }] },
        RowEdit::Insert { values: vec![Cell { column: "name".into(), value: None }] },
    ];
    for st in dialect::row_edits(e, schema.as_deref(), parent_name, &edits).unwrap() {
        s.run(&st, 0).await.unwrap_or_else(|err| panic!("{st}: {err}"));
    }
    let rs = s
        .run(&format!("SELECT id, name FROM {} ORDER BY id", dialect::qualified(e, schema.as_deref(), parent_name)), 10)
        .await
        .unwrap();
    assert_eq!(rs.rows.len(), 2);
    assert_eq!(rs.rows[0][1].as_deref(), Some("O'Brien \\ ünïcode"), "literal escaping round-trips");
    assert_eq!(rs.rows[1][1], None);
    let first_id = rs.rows[0][0].clone();

    let update = vec![RowEdit::Update {
        key: vec![Cell { column: "id".into(), value: first_id.clone() }],
        values: vec![Cell { column: "name".into(), value: Some("Ann".into()) }],
    }];
    let results = s.run_in_transaction(e, &dialect::row_edits(e, schema.as_deref(), parent_name, &update).unwrap()).await.unwrap();
    assert_eq!(results[0].rows_affected, Some(1), "an update reports the row it matched");

    let child_rows = format!(
        "INSERT INTO {} (id, parent_id, created_at) VALUES (1, {}, '2026-01-02 03:04:05')",
        dialect::qualified(e, schema.as_deref(), child_name),
        first_id.clone().unwrap()
    );
    s.run(&child_rows, 0).await.unwrap();

    // Read the table back and compare with what was designed.
    let loaded = introspect::table(e, &mut s, schema.as_deref(), child_name).await.unwrap();
    assert_eq!(loaded.columns.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), ["id", "parent_id", "status", "created_at"]);
    assert_eq!(loaded.primary_key, ["id"]);
    assert!(loaded.indexes.iter().any(|i| i.name == "ix_reach_child_status"), "{:?}", loaded.indexes);
    assert_eq!(loaded.foreign_keys.len(), 1, "{:?}", loaded.foreign_keys);
    assert_eq!(loaded.foreign_keys[0].on_delete.as_deref().map(str::to_uppercase).as_deref(), Some("CASCADE"));
    let status = loaded.columns.iter().find(|c| c.name == "status").unwrap();
    assert_eq!(status.length, Some(20));
    assert!(status.default.as_deref().unwrap_or("").contains("new"), "default read back: {:?}", status.default);

    // Unchanged design → no statements: introspection and generation agree.
    let noop = design::alter_table(e, &loaded, &loaded).unwrap();
    assert!(noop.statements.is_empty(), "{e:?} would alter an unchanged table: {:?}", noop.statements);

    // Rename a column, widen another, add one; rows must survive.
    let mut next = loaded.clone();
    next.columns[2].name = "state".into();
    next.columns[2].length = Some(40);
    next.columns.push(ColumnDef { length: Some(10), ..col("note", text) });
    let ix = next.indexes.iter_mut().find(|i| i.name == "ix_reach_child_status").unwrap();
    ix.columns[0].name = "state".into();
    let plan = design::alter_table(e, &loaded, &next).unwrap();
    if plan.transactional {
        s.run_in_transaction(e, &plan.statements).await.unwrap_or_else(|err| panic!("{:?}: {err}", plan.statements));
    } else {
        for st in &plan.statements {
            s.run(st, 0).await.unwrap_or_else(|err| panic!("{st}: {err}"));
        }
    }
    let after = introspect::table(e, &mut s, schema.as_deref(), child_name).await.unwrap();
    assert!(after.columns.iter().any(|c| c.name == "state" && c.length == Some(40)), "{:?}", after.columns);
    let kept = s.run(&format!("SELECT state FROM {}", dialect::qualified(e, schema.as_deref(), child_name)), 5).await.unwrap();
    assert_eq!(kept.rows[0][0].as_deref(), Some("new"), "the renamed column kept its data");

    // Page through the table the way the grid does.
    let page = dialect::page(e, &format!("SELECT * FROM {}", dialect::qualified(e, schema.as_deref(), parent_name)), Some("id"), 1, 1);
    assert_eq!(s.run(&page, 10).await.unwrap().rows.len(), 1);

    // Back up, drop, restore, compare.
    let mut dump = Vec::new();
    let opts = DumpOptions { tables: Some(vec![parent_name.into(), child_name.into()]), data: true, structure: true, views: false, routines: false };
    let summary = backup::dump(e, &mut s, &database, if e.is_mysql_family() { None } else { schema.as_deref() }, &opts, &mut dump, |_| {}).await.unwrap();
    assert_eq!(summary.rows, 3);
    let script = String::from_utf8(dump).unwrap();
    for t in [child_name, parent_name] {
        s.run(&format!("DROP TABLE {}", dialect::qualified(e, schema.as_deref(), t)), 0).await.unwrap();
    }
    let restored = backup::restore(e, &mut s, &script, false, || false, |_| {}).await.unwrap_or_else(|err| panic!("{err}\n---\n{script}"));
    assert!(restored.failed.is_empty(), "{:?}", restored.failed);
    let back = s.run(&format!("SELECT name FROM {} ORDER BY id", dialect::qualified(e, schema.as_deref(), parent_name)), 10).await.unwrap();
    assert_eq!(back.rows[0][0].as_deref(), Some("Ann"));
    let fk_back = introspect::table(e, &mut s, schema.as_deref(), child_name).await.unwrap();
    assert_eq!(fk_back.foreign_keys.len(), 1, "foreign key restored");

    // The tree and the monitor.
    let objects = introspect::objects(e, &mut s, schema.as_deref().unwrap_or(&database)).await.unwrap();
    assert!(objects.iter().any(|o| o.name == parent_name));
    if e != Engine::Sqlite {
        let own = live.own_server_ids().await;
        let sessions = introspect::sessions(e, &mut s, &own).await.unwrap();
        assert!(sessions.iter().any(|x| x.is_self), "Reach finds its own session in the monitor");
    }

    for t in [child_name, parent_name] {
        s.run(&format!("DROP TABLE {}", dialect::qualified(e, schema.as_deref(), t)), 0).await.unwrap();
    }
}

#[tokio::test]
#[ignore]
async fn postgres_end_to_end() {
    if let Some(c) = config("REACH_TEST_PG", Engine::Postgres) {
        roundtrip(c).await;
    }
}

#[tokio::test]
#[ignore]
async fn mysql_end_to_end() {
    if let Some(c) = config("REACH_TEST_MYSQL", Engine::Mysql) {
        roundtrip(c).await;
    }
}

#[tokio::test]
#[ignore]
async fn mariadb_end_to_end() {
    if let Some(c) = config("REACH_TEST_MARIADB", Engine::Mariadb) {
        roundtrip(c).await;
    }
}

#[tokio::test]
#[ignore]
async fn mssql_end_to_end() {
    if let Some(c) = config("REACH_TEST_MSSQL", Engine::Mssql) {
        roundtrip(c).await;
    }
}

#[tokio::test]
#[ignore]
async fn sqlite_end_to_end() {
    let dir = std::env::temp_dir().join(format!("reach-sqlite-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("test.db");
    // libsql creates the file on open; LiveDb refuses a missing one on purpose.
    libsql::Builder::new_local(&path).build().await.unwrap().connect().unwrap();
    let conn = DbConnection {
        id: "sqlite".into(),
        name: "sqlite".into(),
        engine: Engine::Sqlite,
        host: String::new(),
        port: 0,
        username: String::new(),
        password: None,
        database: None,
        file_path: Some(path.to_string_lossy().into()),
        route: Route::Direct,
        tls: TlsMode::Disable,
        color: None,
        read_only: false,
        production: false,
        last_used_at: 0,
    };
    roundtrip(conn).await;
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
#[ignore]
async fn redis_end_to_end() {
    let Some(mut c) = config("REACH_TEST_REDIS", Engine::Redis) else { return };
    c.tls = TlsMode::Disable;
    let r = LiveRedis::open(c, None).await.expect("connect");
    let args = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    r.command(0, &args(&["DEL", "reach:test:h", "reach:test:s"])).await.unwrap();
    r.command(0, &args(&["HSET", "reach:test:h", "a field", "a value with spaces"])).await.unwrap();
    r.command(0, &args(&["SET", "reach:test:s", "hello", "EX", "100"])).await.unwrap();
    let page = r.scan(0, "reach:test:*", "0", 1000).await.unwrap();
    assert!(page.keys.iter().any(|k| k.key == "reach:test:h" && k.kind == "hash"));
    let v = r.get(0, "reach:test:h").await.unwrap();
    assert!(matches!(v.value, super::redis::RedisValue::Hash { ref entries } if entries[0].1 == "a value with spaces"));
    let s = r.get(0, "reach:test:s").await.unwrap();
    assert!(s.ttl_ms.is_some_and(|t| t > 0));
    assert!(!r.info().await.unwrap().is_empty());
    r.command(0, &args(&["DEL", "reach:test:h", "reach:test:s"])).await.unwrap();
}

/// Stop, end-session and read-only against a real server.
async fn control(mut conn: DbConnection) {
    let e = conn.engine;
    let live = std::sync::Arc::new(LiveDb::open(conn.clone(), None).await.expect("connect"));
    let db = live.default_database();

    // Stop: a long statement on one session, cancelled from another.
    if e != Engine::Mssql {
        let sleep = match e {
            Engine::Postgres => "SELECT pg_sleep(20)",
            _ => "SELECT SLEEP(20)",
        };
        let busy = live.session(&db, "tab").await.unwrap();
        let started = std::time::Instant::now();
        let runner = tokio::spawn(async move { busy.lock().await.run(sleep, 1).await });
        tokio::time::sleep(std::time::Duration::from_millis(700)).await;
        live.cancel(&db, "tab").await.expect("cancel");
        let _ = runner.await.unwrap();
        assert!(started.elapsed().as_secs() < 10, "{e:?}: Stop did not interrupt the statement");
    }

    // End session: the monitor kills another session, which then fails.
    let victim = live.session(&db, "victim").await.unwrap();
    let id: i64 = {
        let sql = match e {
            Engine::Postgres => "SELECT pg_backend_pid()",
            Engine::Mssql => "SELECT @@SPID",
            _ => "SELECT CONNECTION_ID()",
        };
        victim.lock().await.run(sql, 1).await.unwrap().rows[0][0].clone().unwrap().parse().unwrap()
    };
    let meta = live.session(&db, META).await.unwrap();
    meta.lock().await.run(&introspect::kill_sql(e, id, false).unwrap(), 1).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    assert!(victim.lock().await.run("SELECT 1", 1).await.is_err(), "{e:?}: killed session still answers");

    // Read-only: the server itself refuses a write, not only Reach.
    if e != Engine::Mssql {
        conn.read_only = true;
        let ro = LiveDb::open(conn, None).await.unwrap();
        let s = ro.session(&ro.default_database(), META).await.unwrap();
        let err = s.lock().await.run("CREATE TABLE reach_ro_probe (a int)", 0).await;
        assert!(err.is_err(), "{e:?}: a read-only session created a table");
    }
}

#[tokio::test]
#[ignore]
async fn control_on_every_server() {
    for (var, e) in [
        ("REACH_TEST_PG", Engine::Postgres),
        ("REACH_TEST_MYSQL", Engine::Mysql),
        ("REACH_TEST_MARIADB", Engine::Mariadb),
        ("REACH_TEST_MSSQL", Engine::Mssql),
    ] {
        if let Some(c) = config(var, e) {
            control(c).await;
        }
    }
}
