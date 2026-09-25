//! Live connections, and the sessions that run SQL on them.
//!
//! One [`LiveDb`] per open connection. Inside it, one [`Session`] per purpose:
//! each query tab gets its own server session, so a `BEGIN` or a `SET` in one
//! tab stays in that tab, and the object tree and data grids share another.
//! Every session remembers its server-side id, which is what makes Stop work:
//! the cancel goes out on a different session while the busy one is locked.
//!
//! Everything comes back as text (see [`super::types`]). PostgreSQL and MySQL
//! are driven through sqlx's simple-query path, which returns text for every
//! type without a decoder per type; SQL Server values are rendered from
//! tiberius's column data; SQLite goes through libsql.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use futures_util::TryStreamExt;
use sqlx::mysql::{MySqlConnectOptions, MySqlPoolOptions, MySqlSslMode};
use sqlx::pool::PoolConnection;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions, PgSslMode};
use sqlx::{AssertSqlSafe, Column, Either, Executor, MySql, Postgres, Row, SqlSafeStr, TypeInfo, ValueRef};
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio_util::compat::{Compat, TokioAsyncWriteCompatExt};

use super::guard::{self, Risk};
use super::types::{ColumnInfo, DbConnection, Engine, ResultSet, TlsMode};
use crate::ssh::client::{HeadlessConnection, SharedHandle};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);

/// A session key: the database it is bound to, and who it belongs to.
fn key(database: &str, owner: &str) -> String {
    format!("{database}\u{0}{owner}")
}

/// The owner name for the tree, grids and designers.
pub const META: &str = "__meta__";

// ---------------------------------------------------------------------------
// SSH forwarding

/// A local port that relays to the database through SSH. Dropping it stops
/// the listener and, for a connection Reach opened itself, logs out.
pub struct Forward {
    pub port: u16,
    task: tokio::task::JoinHandle<()>,
    _ssh: Option<HeadlessConnection>,
}

impl Drop for Forward {
    fn drop(&mut self) {
        self.task.abort();
    }
}

/// Listen on an ephemeral loopback port and relay each accepted connection
/// through `handle` to `host:port` as the SSH server sees it.
pub(crate) async fn open_forward(
    handle: SharedHandle,
    keep: Option<HeadlessConnection>,
    host: String,
    port: u16,
) -> Result<Forward, String> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|e| format!("Could not open a local port for the SSH tunnel: {e}"))?;
    let local = listener.local_addr().map_err(|e| e.to_string())?.port();
    let task = tokio::spawn(async move {
        while let Ok((tcp, _)) = listener.accept().await {
            let handle = handle.clone();
            let host = host.clone();
            tokio::spawn(async move {
                if let Err(e) = crate::tunnel::manager::relay_connection(tcp, &handle, &host, port).await {
                    tracing::warn!("database tunnel relay ended: {e}");
                }
            });
        }
    });
    Ok(Forward { port: local, task, _ssh: keep })
}

// ---------------------------------------------------------------------------
// Sessions

type MsClient = tiberius::Client<Compat<TcpStream>>;

pub enum Session {
    Pg(PoolConnection<Postgres>),
    My(PoolConnection<MySql>),
    Lite(libsql::Connection),
    Ms(Box<MsClient>),
}

struct Slot {
    session: Arc<Mutex<Session>>,
    /// Server-side id: backend pid, connection id, or SPID.
    server_id: Option<i64>,
}

pub struct LiveDb {
    pub config: DbConnection,
    /// Where to dial: the server itself, or the local end of the tunnel.
    host: String,
    port: u16,
    _forward: Option<Forward>,
    pg_pools: Mutex<HashMap<String, sqlx::PgPool>>,
    my_pool: Option<sqlx::MySqlPool>,
    lite: Option<(libsql::Database, libsql::Connection)>,
    slots: Mutex<HashMap<String, Slot>>,
}

impl LiveDb {
    /// Connect and prove it with a trivial query, so a wrong password fails
    /// here rather than on the first click in the tree.
    pub async fn open(config: DbConnection, forward: Option<Forward>) -> Result<Self, String> {
        let (host, port) = match &forward {
            Some(f) => ("127.0.0.1".to_string(), f.port),
            None => (config.host.clone(), if config.port == 0 { config.engine.default_port() } else { config.port }),
        };
        let mut live = LiveDb {
            host,
            port,
            _forward: forward,
            pg_pools: Mutex::new(HashMap::new()),
            my_pool: None,
            lite: None,
            slots: Mutex::new(HashMap::new()),
            config,
        };
        match live.config.engine {
            Engine::Mysql | Engine::Mariadb => {
                let pool = MySqlPoolOptions::new()
                    .max_connections(16)
                    .acquire_timeout(CONNECT_TIMEOUT)
                    .connect_with(live.mysql_options())
                    .await
                    .map_err(describe_error)?;
                live.my_pool = Some(pool);
            }
            Engine::Sqlite => {
                let path = live.config.file_path.clone().filter(|p| !p.is_empty()).ok_or("Choose a database file")?;
                if !std::path::Path::new(&path).exists() {
                    return Err(format!("{path} does not exist"));
                }
                let db = libsql::Builder::new_local(&path).build().await.map_err(|e| e.to_string())?;
                let conn = db.connect().map_err(|e| e.to_string())?;
                live.lite = Some((db, conn));
            }
            Engine::Redis => return Err("Redis connections are opened by the Redis module".into()),
            Engine::Postgres | Engine::Mssql => {}
        }
        let db = live.default_database();
        live.session(&db, META).await?;
        Ok(live)
    }

    pub fn engine(&self) -> Engine {
        self.config.engine
    }

    /// The database a session binds to when none is named.
    pub fn default_database(&self) -> String {
        match self.config.engine {
            Engine::Sqlite => "main".into(),
            // `postgres` exists on every server; a database named after the
            // user (psql's guess) often does not.
            Engine::Postgres => self.config.database.clone().filter(|d| !d.is_empty()).unwrap_or_else(|| "postgres".into()),
            Engine::Mssql => self.config.database.clone().filter(|d| !d.is_empty()).unwrap_or_else(|| "master".into()),
            // MySQL sessions are not bound to a database; names are qualified.
            _ => self.config.database.clone().unwrap_or_default(),
        }
    }

    fn mysql_options(&self) -> MySqlConnectOptions {
        let mut o = MySqlConnectOptions::new()
            .host(&self.host)
            .port(self.port)
            .username(&self.config.username)
            .ssl_mode(match self.config.tls {
                TlsMode::Prefer => MySqlSslMode::Preferred,
                TlsMode::Require => MySqlSslMode::Required,
                TlsMode::Disable => MySqlSslMode::Disabled,
            });
        if let Some(p) = self.config.password.as_deref().filter(|p| !p.is_empty()) {
            o = o.password(p);
        }
        if let Some(d) = self.config.database.as_deref().filter(|d| !d.is_empty()) {
            o = o.database(d);
        }
        o
    }

    async fn pg_pool(&self, database: &str) -> Result<sqlx::PgPool, String> {
        let mut pools = self.pg_pools.lock().await;
        if let Some(p) = pools.get(database) {
            return Ok(p.clone());
        }
        let mut o = PgConnectOptions::new()
            .host(&self.host)
            .port(self.port)
            .username(&self.config.username)
            .database(database)
            .application_name("Reach")
            .ssl_mode(match self.config.tls {
                TlsMode::Prefer => PgSslMode::Prefer,
                TlsMode::Require => PgSslMode::Require,
                TlsMode::Disable => PgSslMode::Disable,
            });
        if let Some(p) = self.config.password.as_deref().filter(|p| !p.is_empty()) {
            o = o.password(p);
        }
        let pool = PgPoolOptions::new()
            .max_connections(16)
            .acquire_timeout(CONNECT_TIMEOUT)
            .connect_with(o)
            .await
            .map_err(describe_error)?;
        pools.insert(database.to_string(), pool.clone());
        Ok(pool)
    }

    async fn mssql_client(&self, database: &str) -> Result<MsClient, String> {
        let mut cfg = tiberius::Config::new();
        cfg.host(&self.host);
        cfg.port(self.port);
        cfg.database(database);
        cfg.application_name("Reach");
        cfg.authentication(tiberius::AuthMethod::sql_server(
            &self.config.username,
            self.config.password.as_deref().unwrap_or(""),
        ));
        // Most SQL Servers present a self-signed certificate. Encrypting
        // without checking it is what "Trust server certificate" does in
        // Microsoft's own tools; Require refuses a server without TLS.
        match self.config.tls {
            TlsMode::Disable => cfg.encryption(tiberius::EncryptionLevel::NotSupported),
            TlsMode::Prefer => cfg.encryption(tiberius::EncryptionLevel::On),
            TlsMode::Require => cfg.encryption(tiberius::EncryptionLevel::Required),
        }
        cfg.trust_cert();
        let tcp = tokio::time::timeout(CONNECT_TIMEOUT, TcpStream::connect((self.host.as_str(), self.port)))
            .await
            .map_err(|_| format!("Timed out reaching {}:{}", self.host, self.port))?
            .map_err(|e| format!("Could not reach {}:{}: {e}", self.host, self.port))?;
        tcp.set_nodelay(true).ok();
        tokio::time::timeout(CONNECT_TIMEOUT, tiberius::Client::connect(cfg, tcp.compat_write()))
            .await
            .map_err(|_| "Timed out logging in to SQL Server".to_string())?
            .map_err(|e| e.to_string())
    }

    /// The session for `owner` on `database`, created on first use.
    pub async fn session(&self, database: &str, owner: &str) -> Result<Arc<Mutex<Session>>, String> {
        let k = key(database, owner);
        if let Some(slot) = self.slots.lock().await.get(&k) {
            return Ok(slot.session.clone());
        }
        let mut session = match self.config.engine {
            Engine::Postgres => Session::Pg(self.pg_pool(database).await?.acquire().await.map_err(describe_error)?),
            Engine::Mysql | Engine::Mariadb => {
                Session::My(self.my_pool.as_ref().ok_or("Not connected")?.acquire().await.map_err(describe_error)?)
            }
            Engine::Sqlite => {
                let (db, _) = self.lite.as_ref().ok_or("Not connected")?;
                Session::Lite(db.connect().map_err(|e| e.to_string())?)
            }
            Engine::Mssql => Session::Ms(Box::new(self.mssql_client(database).await?)),
            Engine::Redis => return Err("Not a SQL connection".into()),
        };

        // MySQL sessions are not bound to a database at connect time; a tab
        // opened on one should still find its tables unqualified.
        if self.config.engine.is_mysql_family() && !database.is_empty() {
            session.run(&format!("USE {}", super::dialect::ident(self.config.engine, database)), 0).await?;
        }

        let id_sql = match self.config.engine {
            Engine::Postgres => Some("SELECT pg_backend_pid()"),
            Engine::Mysql | Engine::Mariadb => Some("SELECT CONNECTION_ID()"),
            Engine::Mssql => Some("SELECT @@SPID"),
            _ => None,
        };
        let server_id = match id_sql {
            Some(sql) => session
                .run(sql, 1)
                .await
                .ok()
                .and_then(|rs| rs.rows.first().and_then(|r| r.first().cloned().flatten()))
                .and_then(|v| v.parse().ok()),
            None => None,
        };
        if self.config.read_only {
            let ro = match self.config.engine {
                Engine::Postgres => Some("SET SESSION CHARACTERISTICS AS TRANSACTION READ ONLY"),
                Engine::Mysql | Engine::Mariadb => Some("SET SESSION TRANSACTION READ ONLY"),
                Engine::Sqlite => Some("PRAGMA query_only = ON"),
                _ => None,
            };
            if let Some(sql) = ro {
                session.run(sql, 0).await?;
            }
        }

        let session = Arc::new(Mutex::new(session));
        self.slots.lock().await.insert(k, Slot { session: session.clone(), server_id });
        Ok(session)
    }

    /// Forget a session; the next use opens a fresh one.
    pub async fn drop_session(&self, database: &str, owner: &str) {
        self.slots.lock().await.remove(&key(database, owner));
    }

    /// Stop whatever `owner`'s session is running, from another session.
    pub async fn cancel(&self, database: &str, owner: &str) -> Result<(), String> {
        let id = self
            .slots
            .lock()
            .await
            .get(&key(database, owner))
            .and_then(|s| s.server_id)
            .ok_or("Nothing to stop")?;
        let sql = match self.config.engine {
            Engine::Postgres => format!("SELECT pg_cancel_backend({id})"),
            Engine::Mysql | Engine::Mariadb => format!("KILL QUERY {id}"),
            // SQL Server has no statement-only cancel over TDS here; ending the
            // session stops the batch, and the tab reconnects on next use.
            Engine::Mssql => format!("KILL {id}"),
            _ => return Err("This database cannot stop a running statement".into()),
        };
        let control = self.session(database, "__control__").await?;
        control.lock().await.run(&sql, 1).await?;
        if self.config.engine == Engine::Mssql {
            self.drop_session(database, owner).await;
        }
        Ok(())
    }

    /// Server ids of Reach's own sessions, so the monitor can mark them.
    pub async fn own_server_ids(&self) -> Vec<i64> {
        self.slots.lock().await.values().filter_map(|s| s.server_id).collect()
    }

    pub async fn close(&self) {
        self.slots.lock().await.clear();
        for (_, p) in self.pg_pools.lock().await.drain() {
            p.close().await;
        }
        if let Some(p) = &self.my_pool {
            p.close().await;
        }
    }
}

impl Session {
    /// Run one statement. At most `max_rows` rows are kept; the rest are read
    /// and discarded so the session is ready for the next statement.
    pub async fn run(&mut self, sql: &str, max_rows: usize) -> Result<ResultSet, String> {
        Ok(self.run_all(sql, max_rows).await?.into_iter().next().unwrap_or_default())
    }

    /// Like [`run`](Self::run), but keeps every result set a SQL Server batch
    /// returns. Other engines run one statement, so return one set.
    pub async fn run_all(&mut self, sql: &str, max_rows: usize) -> Result<Vec<ResultSet>, String> {
        let started = Instant::now();
        let mut sets = match self {
            Session::Pg(conn) => vec![run_sqlx_pg(conn, sql, max_rows).await?],
            Session::My(conn) => vec![run_sqlx_my(conn, sql, max_rows).await?],
            Session::Lite(conn) => vec![run_lite(conn, sql, max_rows).await?],
            Session::Ms(client) => run_ms(client, sql, max_rows).await?,
        };
        let elapsed = started.elapsed().as_millis() as u64;
        for rs in &mut sets {
            rs.statement = sql.to_string();
            rs.elapsed_ms = elapsed;
        }
        Ok(sets)
    }

    /// Run statements in one transaction; roll back on the first error.
    pub async fn run_in_transaction(&mut self, engine: Engine, statements: &[String]) -> Result<Vec<ResultSet>, String> {
        let (begin, commit, rollback) = match engine {
            Engine::Mssql => ("BEGIN TRANSACTION", "COMMIT TRANSACTION", "ROLLBACK TRANSACTION"),
            _ => ("BEGIN", "COMMIT", "ROLLBACK"),
        };
        self.run(begin, 0).await?;
        let mut out = Vec::with_capacity(statements.len());
        for (i, s) in statements.iter().enumerate() {
            match self.run(s, 0).await {
                Ok(rs) => out.push(rs),
                Err(e) => {
                    let _ = self.run(rollback, 0).await;
                    return Err(format!("Statement {} failed, nothing was changed: {e}", i + 1));
                }
            }
        }
        self.run(commit, 0).await?;
        Ok(out)
    }
}

fn describe_error(e: sqlx::Error) -> String {
    let text = e.to_string();
    // The server asked for a login method the driver cannot speak. Say which
    // and what to do, rather than passing on a protocol error.
    if let Some(plugin) = text.split("unknown authentication plugin: ").nth(1) {
        let plugin = plugin.split_whitespace().next().unwrap_or(plugin);
        let how = match plugin {
            "auth_gssapi_client" => "Windows or Kerberos sign-in (GSSAPI)",
            "client_ed25519" => "ed25519 keys",
            "unix_socket" | "auth_socket" => "the server's own system accounts (unix_socket)",
            "dialog" | "auth_pam" => "PAM",
            _ => "a method Reach does not support",
        };
        return format!(
            "The server offered {how} ({plugin}), which Reach cannot use. If this account also has a password,              enter it and the server will accept that instead. If it has none, connect as a user that does,              e.g. CREATE USER 'reach'@'localhost' IDENTIFIED BY '…'; and GRANT it what it needs."
        );
    }
    match e {
        sqlx::Error::Database(d) => d.message().to_string(),
        sqlx::Error::PoolTimedOut => "Timed out connecting to the server".into(),
        other => other.to_string(),
    }
}

fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(2 + bytes.len() * 2);
    s.push_str("0x");
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

async fn run_sqlx_pg(conn: &mut PoolConnection<Postgres>, sql: &str, max_rows: usize) -> Result<ResultSet, String> {
    let mut rs = ResultSet::default();
    let mut stream = sqlx::raw_sql(AssertSqlSafe(sql.to_string())).fetch_many(&mut **conn);
    while let Some(item) = stream.try_next().await.map_err(describe_error)? {
        match item {
            Either::Left(done) => {
                rs.rows_affected = Some(rs.rows_affected.unwrap_or(0) + done.rows_affected());
            }
            Either::Right(row) => {
                if rs.columns.is_empty() {
                    rs.columns = row
                        .columns()
                        .iter()
                        .map(|c| ColumnInfo { name: c.name().to_string(), type_name: c.type_info().name().to_lowercase() })
                        .collect();
                }
                if rs.rows.len() >= max_rows {
                    rs.truncated = true;
                    continue;
                }
                let mut out = Vec::with_capacity(row.len());
                for i in 0..row.len() {
                    let v = row.try_get_raw(i).map_err(describe_error)?;
                    out.push(if v.is_null() {
                        None
                    } else {
                        Some(<&str as sqlx::Decode<Postgres>>::decode(v).map(str::to_string).map_err(|e| e.to_string())?)
                    });
                }
                rs.rows.push(out);
            }
        }
    }
    drop(stream);
    // A query that returned no rows still has columns worth showing.
    if rs.columns.is_empty() && guard::classify(Engine::Postgres, sql) == Risk::Read {
        if let Ok(stmt) = (&mut **conn).prepare(AssertSqlSafe(sql.to_string()).into_sql_str()).await {
            use sqlx::Statement;
            rs.columns = stmt
                .columns()
                .iter()
                .map(|c| ColumnInfo { name: c.name().to_string(), type_name: c.type_info().name().to_lowercase() })
                .collect();
        }
    }
    if !rs.columns.is_empty() && guard::classify(Engine::Postgres, sql) == Risk::Read {
        rs.rows_affected = None;
    }
    Ok(rs)
}

async fn run_sqlx_my(conn: &mut PoolConnection<MySql>, sql: &str, max_rows: usize) -> Result<ResultSet, String> {
    let mut rs = ResultSet::default();
    let mut stream = sqlx::raw_sql(AssertSqlSafe(sql.to_string())).fetch_many(&mut **conn);
    while let Some(item) = stream.try_next().await.map_err(describe_error)? {
        match item {
            Either::Left(done) => {
                rs.rows_affected = Some(rs.rows_affected.unwrap_or(0) + done.rows_affected());
            }
            Either::Right(row) => {
                if rs.columns.is_empty() {
                    rs.columns = row
                        .columns()
                        .iter()
                        .map(|c| ColumnInfo { name: c.name().to_string(), type_name: c.type_info().name().to_lowercase() })
                        .collect();
                }
                if rs.rows.len() >= max_rows {
                    rs.truncated = true;
                    continue;
                }
                let mut out = Vec::with_capacity(row.len());
                for i in 0..row.len() {
                    let v = row.try_get_raw(i).map_err(describe_error)?;
                    if v.is_null() {
                        out.push(None);
                        continue;
                    }
                    let text = match <&str as sqlx::Decode<MySql>>::decode(v.clone()) {
                        Ok(s) => s.to_string(),
                        Err(_) => hex(<&[u8] as sqlx::Decode<MySql>>::decode(v).map_err(|e| e.to_string())?),
                    };
                    out.push(Some(text));
                }
                rs.rows.push(out);
            }
        }
    }
    drop(stream);
    if rs.columns.is_empty() && guard::classify(Engine::Mysql, sql) == Risk::Read {
        if let Ok(stmt) = (&mut **conn).prepare(AssertSqlSafe(sql.to_string()).into_sql_str()).await {
            use sqlx::Statement;
            rs.columns = stmt
                .columns()
                .iter()
                .map(|c| ColumnInfo { name: c.name().to_string(), type_name: c.type_info().name().to_lowercase() })
                .collect();
        }
    }
    if !rs.columns.is_empty() && guard::classify(Engine::Mysql, sql) == Risk::Read {
        rs.rows_affected = None;
    }
    Ok(rs)
}

async fn run_lite(conn: &libsql::Connection, sql: &str, max_rows: usize) -> Result<ResultSet, String> {
    let mut rs = ResultSet::default();
    let before = conn.total_changes();
    let mut rows = conn.query(sql, ()).await.map_err(|e| e.to_string())?;
    let n = rows.column_count();
    rs.columns = (0..n)
        .map(|i| ColumnInfo {
            name: rows.column_name(i).unwrap_or("").to_string(),
            type_name: String::new(),
        })
        .collect();
    while let Some(row) = rows.next().await.map_err(|e| e.to_string())? {
        if rs.rows.len() >= max_rows {
            rs.truncated = true;
            continue;
        }
        let mut out = Vec::with_capacity(n as usize);
        for i in 0..n {
            let v = row.get_value(i).map_err(|e| e.to_string())?;
            if rs.columns[i as usize].type_name.is_empty() {
                rs.columns[i as usize].type_name = match &v {
                    libsql::Value::Integer(_) => "integer",
                    libsql::Value::Real(_) => "real",
                    libsql::Value::Text(_) => "text",
                    libsql::Value::Blob(_) => "blob",
                    libsql::Value::Null => "",
                }
                .into();
            }
            out.push(match v {
                libsql::Value::Null => None,
                libsql::Value::Integer(i) => Some(i.to_string()),
                libsql::Value::Real(f) => Some(f.to_string()),
                libsql::Value::Text(t) => Some(t),
                libsql::Value::Blob(b) => Some(hex(&b)),
            });
        }
        rs.rows.push(out);
    }
    if rs.columns.is_empty() {
        rs.rows_affected = Some(conn.total_changes().saturating_sub(before));
    }
    Ok(rs)
}

fn ms_type_name(t: tiberius::ColumnType) -> String {
    format!("{t:?}").to_lowercase()
}

fn ms_value(data: tiberius::ColumnData<'static>) -> Option<String> {
    use tiberius::{ColumnData as D, FromSql};
    match &data {
        D::U8(v) => v.map(|x| x.to_string()),
        D::I16(v) => v.map(|x| x.to_string()),
        D::I32(v) => v.map(|x| x.to_string()),
        D::I64(v) => v.map(|x| x.to_string()),
        D::F32(v) => v.map(|x| x.to_string()),
        D::F64(v) => v.map(|x| x.to_string()),
        D::Bit(v) => v.map(|x| if x { "1".into() } else { "0".into() }),
        D::String(v) => v.as_ref().map(|s| s.to_string()),
        D::Guid(v) => v.map(|g| g.to_string().to_uppercase()),
        D::Binary(v) => v.as_ref().map(|b| hex(b)),
        D::Numeric(v) => v.map(|n| n.to_string()),
        D::Xml(v) => v.as_ref().map(|x| x.to_string()),
        D::DateTime(_) | D::SmallDateTime(_) | D::DateTime2(_) => {
            chrono::NaiveDateTime::from_sql(&data).ok().flatten().map(|d| d.to_string())
        }
        D::Date(_) => chrono::NaiveDate::from_sql(&data).ok().flatten().map(|d| d.to_string()),
        D::Time(_) => chrono::NaiveTime::from_sql(&data).ok().flatten().map(|d| d.to_string()),
        D::DateTimeOffset(_) => chrono::DateTime::<chrono::FixedOffset>::from_sql(&data)
            .ok()
            .flatten()
            .map(|d| d.to_rfc3339()),
    }
}

/// Every result set of a batch, in order. SQL Server has no per-statement
/// affected count on this path, so a batch that returns no rows is followed
/// by `SELECT @@ROWCOUNT`, which reports its last statement.
async fn run_ms(client: &mut MsClient, sql: &str, max_rows: usize) -> Result<Vec<ResultSet>, String> {
    let mut sets: Vec<ResultSet> = Vec::new();
    {
        let mut stream = client.simple_query(sql).await.map_err(|e| e.to_string())?;
        while let Some(item) = stream.try_next().await.map_err(|e| e.to_string())? {
            match item {
                tiberius::QueryItem::Metadata(meta) => sets.push(ResultSet {
                    columns: meta
                        .columns()
                        .iter()
                        .map(|c| ColumnInfo { name: c.name().to_string(), type_name: ms_type_name(c.column_type()) })
                        .collect(),
                    ..Default::default()
                }),
                tiberius::QueryItem::Row(row) => {
                    let Some(rs) = sets.last_mut() else { continue };
                    if rs.rows.len() >= max_rows {
                        rs.truncated = true;
                        continue;
                    }
                    rs.rows.push(row.into_iter().map(ms_value).collect());
                }
            }
        }
    }
    if sets.is_empty() {
        let count = client
            .simple_query("SELECT @@ROWCOUNT")
            .await
            .map_err(|e| e.to_string())?
            .into_row()
            .await
            .map_err(|e| e.to_string())?
            .and_then(|r| r.get::<i32, _>(0));
        sets.push(ResultSet { rows_affected: count.map(|c| c.max(0) as u64), ..Default::default() });
    }
    Ok(sets)
}
