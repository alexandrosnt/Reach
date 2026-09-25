//! IPC for the Databases workspace. Every command checks the DevOps switch
//! first; see `crate::devops`.

use std::io::{BufWriter, Write};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::{Emitter, State};

use crate::db::backup::{self, DumpOptions, Progress, Summary};
use crate::db::conn::{open_forward, Forward, LiveDb, META};
use crate::db::design::{self, DdlPlan, TableDesign};
use crate::db::detect::{self, Detected};
use crate::db::dialect::{self, RowEdit};
use crate::db::guard::{self, Risk, Statement};
use crate::db::introspect;
use crate::db::redis::{self as rds, KeyValue, LiveRedis, Reply, ScanPage};
use crate::db::types::{
    DbConnection, DbConnectionView, DbObject, Engine, ResultSet, Route, ServerInfo, ServerSession,
};
use crate::db::{csv, Live};
use crate::devops::{self, Tool};
use crate::ssh::client::{AuthParams, JumpHostParams, KeySource, SharedHandle, SshManager};
use crate::state::{AppState, AuthMethod};

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

// ---------------------------------------------------------------------------
// Saved connections

#[tauri::command]
pub async fn db_list_connections(state: State<'_, AppState>) -> Result<Vec<DbConnectionView>, String> {
    devops::require(Tool::Databases)?;
    let mut vault = state.vault_manager.lock().await;
    let mut db = state.db.lock().await;
    db.store.ensure_loaded(&mut vault).await?;
    Ok(db.store.list().iter().map(DbConnectionView::from).collect())
}

/// Fill in what the form leaves out: a new id, and the stored password when
/// the user did not type a new one (the form never receives it back).
async fn complete(state: &State<'_, AppState>, mut conn: DbConnection, keep_password: bool) -> Result<DbConnection, String> {
    if conn.id.is_empty() {
        conn.id = uuid::Uuid::new_v4().to_string();
    }
    if keep_password && conn.password.as_deref().is_none_or(str::is_empty) {
        let mut vault = state.vault_manager.lock().await;
        let mut db = state.db.lock().await;
        db.store.ensure_loaded(&mut vault).await?;
        conn.password = db.store.get(&conn.id).and_then(|c| c.password.clone());
    }
    if conn.engine == Engine::Sqlite && conn.route != Route::Direct {
        return Err("A SQLite file is opened on this device; it cannot go through SSH".into());
    }
    Ok(conn)
}

#[tauri::command]
pub async fn db_save_connection(
    state: State<'_, AppState>,
    connection: DbConnection,
    keep_password: bool,
) -> Result<DbConnectionView, String> {
    devops::require(Tool::Databases)?;
    if connection.name.trim().is_empty() {
        return Err("Give the connection a name".into());
    }
    if matches!(connection.route, Route::Live { .. }) {
        return Err("A terminal tab's connection ends with the tab. Save it through its saved session instead.".into());
    }
    let conn = complete(&state, connection, keep_password).await?;
    let mut vault = state.vault_manager.lock().await;
    let mut db = state.db.lock().await;
    db.store.ensure_loaded(&mut vault).await?;
    db.store.save(conn.clone(), &mut vault).await?;
    Ok(DbConnectionView::from(&conn))
}

#[tauri::command]
pub async fn db_delete_connection(state: State<'_, AppState>, id: String) -> Result<(), String> {
    devops::require(Tool::Databases)?;
    let live = state.db.lock().await.remove(&id);
    close(live).await;
    let mut vault = state.vault_manager.lock().await;
    let mut db = state.db.lock().await;
    db.store.ensure_loaded(&mut vault).await?;
    db.store.delete(&id, &mut vault).await
}

// ---------------------------------------------------------------------------
// Connecting

async fn auth_for(state: &State<'_, AppState>, method: &AuthMethod) -> Result<AuthParams, String> {
    use crate::ipc::ssh_commands::{build_auth, resolve_key_source};
    match method {
        AuthMethod::Password { password } => build_auth("password", password.clone(), None, None),
        AuthMethod::Key { path, passphrase, key_content, key_id } => {
            let (source, stored) = match key_content.clone().filter(|k| !k.trim().is_empty()) {
                Some(k) => (Some(KeySource::Material(k)), None),
                None => resolve_key_source(state, key_id.clone(), path.clone()).await?,
            };
            build_auth("key", None, source, passphrase.clone().filter(|p| !p.is_empty()).or(stored))
        }
        AuthMethod::Agent => build_auth("agent", None, None, None),
    }
}

/// The SSH leg, if the connection has one.
async fn forward_for(app: &tauri::AppHandle, state: &State<'_, AppState>, conn: &DbConnection) -> Result<Option<Forward>, String> {
    let host = if conn.host.trim().is_empty() { "127.0.0.1".to_string() } else { conn.host.trim().to_string() };
    let port = if conn.port == 0 { conn.engine.default_port() } else { conn.port };
    match &conn.route {
        Route::Direct => Ok(None),
        Route::Live { connection_id } => {
            let handle = state
                .ssh_manager
                .lock()
                .await
                .get_handle(connection_id)
                .map_err(|_| "That terminal tab is no longer connected".to_string())?;
            open_forward(handle, None, host, port).await.map(Some)
        }
        Route::Session { session_id } => {
            let s = crate::ipc::session_commands::session_get(state.clone(), session_id.clone()).await?;
            let auth = auth_for(state, &s.auth_method).await?;
            let mut jumps = Vec::new();
            for j in s.jump_chain.clone().unwrap_or_default() {
                jumps.push(JumpHostParams { auth: auth_for(state, &j.auth_method).await?, host: j.host, port: j.port, username: j.username });
            }
            let ssh = SshManager::open_headless(&s.host, s.port, &s.username, auth, jumps, s.proxy.clone(), app.clone())
                .await
                .map_err(|e| format!("SSH to {}: {e}", s.name))?;
            let handle = ssh.handle.clone();
            open_forward(handle, Some(ssh), host, port).await.map(Some)
        }
    }
}

async fn open_live(app: &tauri::AppHandle, state: &State<'_, AppState>, conn: DbConnection) -> Result<Live, String> {
    let forward = forward_for(app, state, &conn).await?;
    Ok(match conn.engine {
        Engine::Redis => Live::Redis(Arc::new(LiveRedis::open(conn, forward).await?)),
        _ => Live::Sql(Arc::new(LiveDb::open(conn, forward).await?)),
    })
}

async fn close(live: Option<Live>) {
    if let Some(Live::Sql(l)) = live {
        l.close().await;
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Connected {
    pub engine: Engine,
    pub databases: Vec<String>,
    pub default_database: String,
    pub server: ServerInfo,
}

async fn describe_live(live: &Live) -> Result<Connected, String> {
    match live {
        Live::Sql(l) => {
            let e = l.engine();
            let db = l.default_database();
            let session = l.session(&db, META).await?;
            let mut s = session.lock().await;
            let server = introspect::server_info(e, &mut s).await?;
            let databases = introspect::databases(e, &mut s).await?;
            Ok(Connected { engine: e, databases, default_database: db, server })
        }
        Live::Redis(r) => {
            let info = r.info().await?;
            let version = info
                .iter()
                .flat_map(|(_, kv)| kv.iter())
                .find(|(k, _)| k == "redis_version" || k == "valkey_version")
                .map(|(k, v)| format!("{} {v}", if k.starts_with("valkey") { "Valkey" } else { "Redis" }))
                .unwrap_or_else(|| "Redis".into());
            let databases = r.databases().await?.into_iter().map(|(n, _)| n.to_string()).collect();
            Ok(Connected {
                engine: Engine::Redis,
                databases,
                default_database: r.default_db().to_string(),
                server: ServerInfo { version, current_database: Some(r.default_db().to_string()), current_user: None },
            })
        }
    }
}

/// Try a connection from the form without saving it.
#[tauri::command]
pub async fn db_test_connection(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    connection: DbConnection,
    keep_password: bool,
) -> Result<ServerInfo, String> {
    devops::require(Tool::Databases)?;
    let conn = complete(&state, connection, keep_password).await?;
    let live = open_live(&app, &state, conn).await?;
    let info = describe_live(&live).await.map(|c| c.server);
    close(Some(live)).await;
    info
}

/// Open a saved connection, or an unsaved one (a database found on a
/// terminal tab's server). Reconnecting replaces the old connection.
#[tauri::command]
pub async fn db_connect(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    id: String,
    connection: Option<DbConnection>,
) -> Result<Connected, String> {
    devops::require(Tool::Databases)?;
    let conn = match connection {
        Some(c) => complete(&state, DbConnection { id: id.clone(), ..c }, true).await?,
        None => {
            let mut vault = state.vault_manager.lock().await;
            let mut db = state.db.lock().await;
            db.store.ensure_loaded(&mut vault).await?;
            let mut c = db.store.get(&id).cloned().ok_or("That connection no longer exists")?;
            c.last_used_at = now_ms();
            let _ = db.store.save(c.clone(), &mut vault).await;
            c
        }
    };
    let target = format!("{:?} {}@{}:{} via {:?}", conn.engine, conn.username, conn.host, conn.port, conn.route);
    let live = open_live(&app, &state, conn).await.inspect_err(|e| tracing::warn!("db_connect {target}: {e}"))?;
    let described = describe_live(&live).await;
    let old = state.db.lock().await.insert(id, live);
    close(old).await;
    described
}

#[tauri::command]
pub async fn db_disconnect(state: State<'_, AppState>, id: String) -> Result<(), String> {
    devops::require(Tool::Databases)?;
    let live = state.db.lock().await.remove(&id);
    close(live).await;
    Ok(())
}

// ---------------------------------------------------------------------------
// Tree

async fn sql(state: &State<'_, AppState>, id: &str) -> Result<Arc<LiveDb>, String> {
    devops::require(Tool::Databases)?;
    state.db.lock().await.sql(id)
}

#[tauri::command]
pub async fn db_databases(state: State<'_, AppState>, id: String) -> Result<Vec<String>, String> {
    let l = sql(&state, &id).await?;
    let session = l.session(&l.default_database(), META).await?;
    let mut s = session.lock().await;
    introspect::databases(l.engine(), &mut s).await
}

#[tauri::command]
pub async fn db_schemas(state: State<'_, AppState>, id: String, database: String) -> Result<Vec<String>, String> {
    let l = sql(&state, &id).await?;
    let session = l.session(&database, META).await?;
    let mut s = session.lock().await;
    introspect::schemas(l.engine(), &mut s).await
}

#[tauri::command]
pub async fn db_objects(
    state: State<'_, AppState>,
    id: String,
    database: String,
    schema: Option<String>,
) -> Result<Vec<DbObject>, String> {
    let l = sql(&state, &id).await?;
    let session = l.session(&database, META).await?;
    let mut s = session.lock().await;
    introspect::objects(l.engine(), &mut s, schema.as_deref().unwrap_or(&database)).await
}

#[tauri::command]
pub async fn db_completion(
    state: State<'_, AppState>,
    id: String,
    database: String,
    schema: Option<String>,
) -> Result<std::collections::HashMap<String, Vec<String>>, String> {
    let l = sql(&state, &id).await?;
    let session = l.session(&database, META).await?;
    let mut s = session.lock().await;
    introspect::completion_columns(l.engine(), &mut s, schema.as_deref().unwrap_or(&database)).await
}

#[tauri::command]
pub async fn db_definition(
    state: State<'_, AppState>,
    id: String,
    database: String,
    schema: Option<String>,
    name: String,
    kind: String,
) -> Result<String, String> {
    let l = sql(&state, &id).await?;
    let session = l.session(&database, META).await?;
    let mut s = session.lock().await;
    let sch = schema.or_else(|| l.engine().is_mysql_family().then(|| database.clone()));
    introspect::definition(l.engine(), &mut s, sch.as_deref(), &name, &kind).await
}

// ---------------------------------------------------------------------------
// Running SQL

#[tauri::command]
pub fn db_analyze(engine: Engine, script: String) -> Result<Vec<Statement>, String> {
    devops::require(Tool::Databases)?;
    Ok(guard::analyze(engine, &script))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecError {
    pub index: usize,
    pub statement: String,
    pub message: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase", tag = "status")]
pub enum ExecOutcome {
    /// Nothing ran; these statements need a yes first.
    Confirm { statements: Vec<Statement> },
    /// Ran until done or until `error`; `results` holds what finished.
    Done { results: Vec<ResultSet>, error: Option<ExecError> },
}

/// Refuse or hold back a script according to the connection's settings.
fn gate(conn: &DbConnection, statements: &[Statement], confirmed: bool) -> Result<Option<Vec<Statement>>, String> {
    if conn.read_only {
        if let Some(s) = statements.iter().find(|s| !matches!(s.risk, Risk::Read | Risk::Session)) {
            let head: String = s.sql.chars().take(60).collect();
            return Err(format!("This connection is read-only, so \"{head}…\" was not run"));
        }
    }
    if confirmed {
        return Ok(None);
    }
    let risky: Vec<Statement> = statements
        .iter()
        .filter(|s| matches!(s.risk, Risk::Danger(_)) || (conn.production && s.risk == Risk::Write))
        .cloned()
        .collect();
    Ok((!risky.is_empty()).then_some(risky))
}

#[tauri::command]
pub async fn db_execute(
    state: State<'_, AppState>,
    id: String,
    database: String,
    owner: String,
    script: String,
    confirmed: bool,
    max_rows: Option<usize>,
) -> Result<ExecOutcome, String> {
    let l = sql(&state, &id).await?;
    let statements = guard::analyze(l.engine(), &script);
    if statements.is_empty() {
        return Err("There is no SQL to run".into());
    }
    if let Some(risky) = gate(&l.config, &statements, confirmed)? {
        return Ok(ExecOutcome::Confirm { statements: risky });
    }
    let session = l.session(&database, &owner).await?;
    let mut s = session.lock().await;
    let mut results = Vec::new();
    for (i, st) in statements.iter().enumerate() {
        match s.run_all(&st.sql, max_rows.unwrap_or(1000)).await {
            Ok(sets) => results.extend(sets),
            Err(message) => {
                return Ok(ExecOutcome::Done { results, error: Some(ExecError { index: i, statement: st.sql.clone(), message }) });
            }
        }
    }
    Ok(ExecOutcome::Done { results, error: None })
}

#[tauri::command]
pub async fn db_cancel(state: State<'_, AppState>, id: String, database: String, owner: String) -> Result<(), String> {
    let l = sql(&state, &id).await?;
    l.cancel(&database, &owner).await
}

/// A query tab closed: give its server session back.
#[tauri::command]
pub async fn db_close_session(state: State<'_, AppState>, id: String, database: String, owner: String) -> Result<(), String> {
    let l = sql(&state, &id).await?;
    l.drop_session(&database, &owner).await;
    Ok(())
}

// ---------------------------------------------------------------------------
// Table data

#[derive(Deserialize)]
pub struct SortKey {
    pub column: String,
    pub desc: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TablePage {
    pub result: ResultSet,
    /// Only counted for the first page.
    pub total: Option<i64>,
}

/// The owning schema to qualify a table with. MySQL calls its databases
/// schemas; SQLite has none worth naming.
fn owner_schema(engine: Engine, database: &str, schema: Option<String>) -> Option<String> {
    match engine {
        Engine::Mysql | Engine::Mariadb => Some(schema.unwrap_or_else(|| database.to_string())),
        Engine::Sqlite => None,
        _ => schema,
    }
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn db_table_rows(
    state: State<'_, AppState>,
    id: String,
    database: String,
    schema: Option<String>,
    table: String,
    offset: u64,
    limit: u64,
    order: Vec<SortKey>,
    filter: Option<String>,
) -> Result<TablePage, String> {
    let l = sql(&state, &id).await?;
    let e = l.engine();
    let sch = owner_schema(e, &database, schema);
    let t = dialect::qualified(e, sch.as_deref(), &table);
    let filter = filter.map(|f| f.trim().to_string()).filter(|f| !f.is_empty());
    let wher = filter.as_ref().map(|f| format!(" WHERE {f}")).unwrap_or_default();
    let order_sql = (!order.is_empty()).then(|| {
        order
            .iter()
            .map(|k| format!("{}{}", dialect::ident(e, &k.column), if k.desc { " DESC" } else { "" }))
            .collect::<Vec<_>>()
            .join(", ")
    });
    let select = dialect::page(e, &format!("SELECT * FROM {t}{wher}"), order_sql.as_deref(), limit, offset);

    // The filter is typed by the user; it must not smuggle in a second
    // statement or a write.
    let check = guard::analyze(e, &select);
    if check.len() != 1 || check[0].risk != Risk::Read {
        return Err("The filter must be a condition, like  status = 'open'".into());
    }

    let session = l.session(&database, META).await?;
    let mut s = session.lock().await;
    let result = s.run(&select, limit as usize).await?;
    let total = if offset == 0 {
        s.run(&format!("SELECT COUNT(*) FROM {t}{wher}"), 1)
            .await
            .ok()
            .and_then(|rs| rs.rows.first().and_then(|r| r.first().cloned().flatten()))
            .and_then(|v| v.parse().ok())
    } else {
        None
    };
    Ok(TablePage { result, total })
}

#[tauri::command]
pub async fn db_table_design(
    state: State<'_, AppState>,
    id: String,
    database: String,
    schema: Option<String>,
    table: String,
) -> Result<TableDesign, String> {
    let l = sql(&state, &id).await?;
    let e = l.engine();
    let session = l.session(&database, META).await?;
    let mut s = session.lock().await;
    introspect::table(e, &mut s, owner_schema(e, &database, schema).as_deref(), &table).await
}

#[tauri::command]
pub async fn db_preview_edits(
    state: State<'_, AppState>,
    id: String,
    database: String,
    schema: Option<String>,
    table: String,
    edits: Vec<RowEdit>,
) -> Result<Vec<String>, String> {
    let l = sql(&state, &id).await?;
    let e = l.engine();
    dialect::row_edits(e, owner_schema(e, &database, schema).as_deref(), &table, &edits)
}

#[tauri::command]
pub async fn db_apply_edits(
    state: State<'_, AppState>,
    id: String,
    database: String,
    schema: Option<String>,
    table: String,
    edits: Vec<RowEdit>,
) -> Result<Vec<ResultSet>, String> {
    let l = sql(&state, &id).await?;
    if l.config.read_only {
        return Err("This connection is read-only".into());
    }
    let e = l.engine();
    let statements = dialect::row_edits(e, owner_schema(e, &database, schema).as_deref(), &table, &edits)?;
    let session = l.session(&database, META).await?;
    let mut s = session.lock().await;
    let (begin, commit, rollback) = if e == Engine::Mssql {
        ("BEGIN TRANSACTION", "COMMIT TRANSACTION", "ROLLBACK TRANSACTION")
    } else {
        ("BEGIN", "COMMIT", "ROLLBACK")
    };
    s.run(begin, 0).await?;
    let mut results = Vec::with_capacity(statements.len());
    for (i, st) in statements.iter().enumerate() {
        let outcome = s.run(st, 0).await;
        let failure = match &outcome {
            Err(err) => Some(format!("Change {} failed: {err}", i + 1)),
            // An UPDATE or DELETE that matched nothing means the row was
            // changed or removed since it was loaded. Saving the rest would
            // leave the table half-edited, so nothing is saved.
            Ok(rs) if rs.rows_affected == Some(0) && !matches!(edits[i], RowEdit::Insert { .. }) => Some(format!(
                "Change {} matched no row; someone else changed or removed it since it was loaded",
                i + 1
            )),
            Ok(_) => None,
        };
        if let Some(msg) = failure {
            let _ = s.run(rollback, 0).await;
            return Err(format!("{msg}. Nothing was saved."));
        }
        results.push(outcome?);
    }
    s.run(commit, 0).await?;
    Ok(results)
}

// ---------------------------------------------------------------------------
// Designer

#[tauri::command]
pub fn db_preview_design(engine: Engine, old: Option<TableDesign>, new: TableDesign) -> Result<DdlPlan, String> {
    devops::require(Tool::Databases)?;
    match old {
        Some(o) => design::alter_table(engine, &o, &new),
        None => design::create_table(engine, &new),
    }
}

#[tauri::command]
pub async fn db_apply_design(
    state: State<'_, AppState>,
    id: String,
    database: String,
    old: Option<TableDesign>,
    new: TableDesign,
) -> Result<TableDesign, String> {
    let l = sql(&state, &id).await?;
    if l.config.read_only {
        return Err("This connection is read-only".into());
    }
    let e = l.engine();
    // MySQL names its databases where others name schemas; a table made in
    // one is qualified with it, so it lands there and can be read back.
    let mut new = new;
    if e.is_mysql_family() && new.schema.is_none() {
        new.schema = Some(database.clone());
    }
    let plan = match &old {
        Some(o) => design::alter_table(e, o, &new)?,
        None => design::create_table(e, &new)?,
    };
    let session = l.session(&database, META).await?;
    let mut s = session.lock().await;
    if plan.transactional {
        s.run_in_transaction(e, &plan.statements).await?;
    } else {
        for (i, st) in plan.statements.iter().enumerate() {
            if let Err(err) = s.run(st, 0).await {
                if e == Engine::Sqlite {
                    let _ = s.run("ROLLBACK", 0).await;
                    let _ = s.run("PRAGMA foreign_keys = ON", 0).await;
                    return Err(format!("Step {} failed and the table was left as it was: {err}", i + 1));
                }
                return Err(format!(
                    "Step {} of {} failed: {err}. MySQL applies each step on its own, so the ones before it are in place; reopen the table to see where it stands.",
                    i + 1,
                    plan.statements.len()
                ));
            }
        }
    }
    introspect::table(e, &mut s, new.schema.as_deref(), &new.name).await
}

// ---------------------------------------------------------------------------
// Server

#[tauri::command]
pub async fn db_server_info(state: State<'_, AppState>, id: String, database: String) -> Result<ServerInfo, String> {
    let l = sql(&state, &id).await?;
    let session = l.session(&database, META).await?;
    let mut s = session.lock().await;
    introspect::server_info(l.engine(), &mut s).await
}

#[tauri::command]
pub async fn db_sessions(state: State<'_, AppState>, id: String) -> Result<Vec<ServerSession>, String> {
    let l = sql(&state, &id).await?;
    let own = l.own_server_ids().await;
    let session = l.session(&l.default_database(), "__monitor__").await?;
    let mut s = session.lock().await;
    introspect::sessions(l.engine(), &mut s, &own).await
}

#[tauri::command]
pub async fn db_kill(state: State<'_, AppState>, id: String, server_id: i64, only_query: bool) -> Result<(), String> {
    let l = sql(&state, &id).await?;
    let stmt = introspect::kill_sql(l.engine(), server_id, only_query)?;
    let session = l.session(&l.default_database(), "__monitor__").await?;
    let mut s = session.lock().await;
    s.run(&stmt, 1).await.map(|_| ())
}

// ---------------------------------------------------------------------------
// Jobs: backup, restore, import, export

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase", tag = "kind")]
enum JobEvent {
    Progress(Progress),
    Done(Summary),
    Failed { message: String },
}

fn emit(app: &tauri::AppHandle, job: &str, ev: JobEvent) {
    let _ = app.emit(&format!("db-job-{job}"), ev);
}

#[tauri::command]
pub async fn db_cancel_job(state: State<'_, AppState>, job: String) -> Result<(), String> {
    devops::require(Tool::Databases)?;
    if let Some(flag) = state.db.lock().await.job(&job) {
        flag.store(true, Ordering::Relaxed);
    }
    Ok(())
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn db_backup(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    id: String,
    database: String,
    schema: Option<String>,
    options: DumpOptions,
    path: String,
) -> Result<String, String> {
    let l = sql(&state, &id).await?;
    let job = uuid::Uuid::new_v4().to_string();
    state.db.lock().await.start_job(&job);
    let db = state.db.clone();
    let job_id = job.clone();
    tokio::spawn(async move {
        let result = async {
            let file = std::fs::File::create(&path).map_err(|e| format!("Could not write {path}: {e}"))?;
            let mut out = BufWriter::new(file);
            let session = l.session(&database, "__backup__").await?;
            let mut s = session.lock().await;
            let e = l.engine();
            let sch = owner_schema(e, &database, schema);
            let sch = if e.is_mysql_family() { None } else { sch };
            backup::dump(e, &mut s, &database, sch.as_deref(), &options, &mut out, |p| emit(&app, &job_id, JobEvent::Progress(p))).await
        }
        .await;
        emit(&app, &job_id, match result {
            Ok(s) => JobEvent::Done(s),
            Err(message) => JobEvent::Failed { message },
        });
        db.lock().await.end_job(&job_id);
    });
    Ok(job)
}

#[tauri::command]
pub async fn db_restore(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    id: String,
    database: String,
    path: String,
    keep_going: bool,
) -> Result<String, String> {
    let l = sql(&state, &id).await?;
    if l.config.read_only {
        return Err("This connection is read-only".into());
    }
    let script = std::fs::read_to_string(&path).map_err(|e| format!("Could not read {path}: {e}"))?;
    let job = uuid::Uuid::new_v4().to_string();
    let flag = state.db.lock().await.start_job(&job);
    let db = state.db.clone();
    let job_id = job.clone();
    tokio::spawn(async move {
        let result = async {
            let session = l.session(&database, "__restore__").await?;
            let mut s = session.lock().await;
            backup::restore(l.engine(), &mut s, &script, keep_going, || flag.load(Ordering::Relaxed), |p| {
                emit(&app, &job_id, JobEvent::Progress(p))
            })
            .await
        }
        .await;
        emit(&app, &job_id, match result {
            Ok(s) => JobEvent::Done(s),
            Err(message) => JobEvent::Failed { message },
        });
        db.lock().await.end_job(&job_id);
    });
    Ok(job)
}

/// The first rows of a CSV file, for the import mapping screen.
#[tauri::command]
pub fn db_csv_preview(path: String, delimiter: char) -> Result<Vec<Vec<String>>, String> {
    devops::require(Tool::Databases)?;
    let text = std::fs::read_to_string(&path).map_err(|e| format!("Could not read {path}: {e}"))?;
    let mut rows = csv::parse(&text, delimiter)?;
    rows.truncate(20);
    Ok(rows)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportSpec {
    pub path: String,
    pub delimiter: char,
    pub header: bool,
    /// For each CSV column, the table column it goes to, or `None` to skip.
    pub mapping: Vec<Option<String>>,
    pub empty_as_null: bool,
}

/// Import a CSV into a table, all or nothing.
#[tauri::command]
pub async fn db_import_csv(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    id: String,
    database: String,
    schema: Option<String>,
    table: String,
    spec: ImportSpec,
) -> Result<String, String> {
    let l = sql(&state, &id).await?;
    if l.config.read_only {
        return Err("This connection is read-only".into());
    }
    let text = std::fs::read_to_string(&spec.path).map_err(|e| format!("Could not read {}: {e}", spec.path))?;
    let mut rows = csv::parse(&text, spec.delimiter)?;
    if spec.header && !rows.is_empty() {
        rows.remove(0);
    }
    let targets: Vec<(usize, String)> = spec.mapping.iter().enumerate().filter_map(|(i, m)| m.clone().map(|c| (i, c))).collect();
    if targets.is_empty() {
        return Err("Map at least one column".into());
    }
    let e = l.engine();
    let t = dialect::qualified(e, owner_schema(e, &database, schema).as_deref(), &table);
    let cols = targets.iter().map(|(_, c)| dialect::ident(e, c)).collect::<Vec<_>>().join(", ");

    let job = uuid::Uuid::new_v4().to_string();
    let flag = state.db.lock().await.start_job(&job);
    let db = state.db.clone();
    let job_id = job.clone();
    tokio::spawn(async move {
        let result = async {
            let session = l.session(&database, "__import__").await?;
            let mut s = session.lock().await;
            let (begin, commit, rollback) = if e == Engine::Mssql {
                ("BEGIN TRANSACTION", "COMMIT TRANSACTION", "ROLLBACK TRANSACTION")
            } else {
                ("BEGIN", "COMMIT", "ROLLBACK")
            };
            s.run(begin, 0).await?;
            let total = rows.len();
            // SQL Server allows at most 1000 rows per VALUES list.
            for (n, chunk) in rows.chunks(200).enumerate() {
                if flag.load(Ordering::Relaxed) {
                    let _ = s.run(rollback, 0).await;
                    return Err("Stopped; nothing was imported".to_string());
                }
                let values = chunk
                    .iter()
                    .map(|r| {
                        let v = targets
                            .iter()
                            .map(|(i, _)| {
                                let cell = r.get(*i).map(String::as_str).unwrap_or("");
                                if spec.empty_as_null && cell.is_empty() { "NULL".to_string() } else { dialect::literal(e, Some(cell)) }
                            })
                            .collect::<Vec<_>>()
                            .join(", ");
                        format!("({v})")
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                if let Err(err) = s.run(&format!("INSERT INTO {t} ({cols}) VALUES {values}"), 0).await {
                    let _ = s.run(rollback, 0).await;
                    return Err(format!("Rows {}–{}: {err}. Nothing was imported.", n * 200 + 1, n * 200 + chunk.len()));
                }
                emit(&app, &job_id, JobEvent::Progress(Progress { step: "Importing".into(), done: n * 200 + chunk.len(), total, rows: (n * 200 + chunk.len()) as u64 }));
            }
            s.run(commit, 0).await?;
            Ok(Summary { tables: 1, rows: total as u64, statements: total.div_ceil(200), failed: vec![] })
        }
        .await;
        emit(&app, &job_id, match result {
            Ok(s) => JobEvent::Done(s),
            Err(message) => JobEvent::Failed { message },
        });
        db.lock().await.end_job(&job_id);
    });
    Ok(job)
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    Csv,
    Json,
    Sql,
}

/// Export a whole table (paged) or a query's full result to a file.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn db_export(
    state: State<'_, AppState>,
    id: String,
    database: String,
    schema: Option<String>,
    table: Option<String>,
    query: Option<String>,
    format: ExportFormat,
    path: String,
) -> Result<u64, String> {
    let l = sql(&state, &id).await?;
    let e = l.engine();
    let sch = owner_schema(e, &database, schema);
    let file = std::fs::File::create(&path).map_err(|err| format!("Could not write {path}: {err}"))?;
    let mut out = BufWriter::new(file);
    let io = |err: std::io::Error| err.to_string();
    let session = l.session(&database, "__export__").await?;
    let mut s = session.lock().await;

    let target = table.as_deref().map(|t| dialect::qualified(e, sch.as_deref(), t));
    let sql_name = target.clone().unwrap_or_else(|| dialect::ident(e, "export"));
    let mut written = 0u64;
    let mut first = true;
    let mut offset = 0u64;
    const PAGE: u64 = 5000;

    if matches!(format, ExportFormat::Json) {
        out.write_all(b"[\n").map_err(io)?;
    }
    loop {
        let rs = match (&target, &query) {
            (Some(t), _) => s.run(&dialect::page(e, &format!("SELECT * FROM {t}"), None, PAGE, offset), PAGE as usize).await?,
            (None, Some(q)) => {
                let check = guard::analyze(e, q);
                if check.len() != 1 || check[0].risk != Risk::Read {
                    return Err("Only a single SELECT can be exported".into());
                }
                s.run(q, 5_000_000).await?
            }
            (None, None) => return Err("Nothing to export".into()),
        };
        if first {
            if let ExportFormat::Csv = format {
                let header: Vec<Option<String>> = rs.columns.iter().map(|c| Some(c.name.clone())).collect();
                out.write_all(csv::line(&header, ',').as_bytes()).map_err(io)?;
            }
        }
        let cols: Vec<String> = rs.columns.iter().map(|c| c.name.clone()).collect();
        for row in &rs.rows {
            match format {
                ExportFormat::Csv => out.write_all(csv::line(row, ',').as_bytes()).map_err(io)?,
                ExportFormat::Json => {
                    let obj: serde_json::Map<String, serde_json::Value> = cols
                        .iter()
                        .zip(row)
                        .map(|(c, v)| (c.clone(), v.clone().map(serde_json::Value::String).unwrap_or(serde_json::Value::Null)))
                        .collect();
                    if written > 0 {
                        out.write_all(b",\n").map_err(io)?;
                    }
                    out.write_all(b"  ").map_err(io)?;
                    serde_json::to_writer(&mut out, &obj).map_err(|err| err.to_string())?;
                }
                ExportFormat::Sql => {
                    let names = cols.iter().map(|c| dialect::ident(e, c)).collect::<Vec<_>>().join(", ");
                    let vals = row.iter().map(|v| dialect::literal(e, v.as_deref())).collect::<Vec<_>>().join(", ");
                    writeln!(out, "INSERT INTO {sql_name} ({names}) VALUES ({vals});").map_err(io)?;
                }
            }
            written += 1;
        }
        first = false;
        offset += PAGE;
        if target.is_none() || (rs.rows.len() as u64) < PAGE {
            break;
        }
    }
    if matches!(format, ExportFormat::Json) {
        out.write_all(b"\n]\n").map_err(io)?;
    }
    out.flush().map_err(io)?;
    Ok(written)
}

// ---------------------------------------------------------------------------
// Finding databases on a terminal tab's machine

async fn probe_port(handle: Option<&SharedHandle>, host: &str, port: u16) -> Option<Vec<u8>> {
    use tokio::io::AsyncReadExt;
    let timeout = std::time::Duration::from_millis(1500);
    let mut greeting = vec![0u8; 256];
    match handle {
        Some(h) => {
            let channel = {
                let guard = h.lock().await;
                tokio::time::timeout(timeout, guard.channel_open_direct_tcpip(host, port as u32, "127.0.0.1", 0)).await.ok()?.ok()?
            };
            let mut stream = channel.into_stream();
            let n = tokio::time::timeout(std::time::Duration::from_millis(400), stream.read(&mut greeting)).await.ok().and_then(Result::ok).unwrap_or(0);
            greeting.truncate(n);
            Some(greeting)
        }
        None => {
            let mut tcp = tokio::time::timeout(std::time::Duration::from_millis(300), tokio::net::TcpStream::connect((host, port))).await.ok()?.ok()?;
            let n = tokio::time::timeout(std::time::Duration::from_millis(300), tcp.read(&mut greeting)).await.ok().and_then(Result::ok).unwrap_or(0);
            greeting.truncate(n);
            Some(greeting)
        }
    }
}

/// Databases on the machine behind a terminal tab (`connection_id`), or on
/// this device when there is none.
#[tauri::command]
pub async fn db_detect(state: State<'_, AppState>, connection_id: Option<String>) -> Result<Vec<Detected>, String> {
    devops::require(Tool::Databases)?;
    let handle = match &connection_id {
        Some(id) => Some(state.ssh_manager.lock().await.get_handle(id).map_err(|_| "That tab is not connected".to_string())?),
        None => None,
    };
    let mut found = match &handle {
        Some(h) => crate::ssh::client::exec_on_connection(h, detect::PROBE_SCRIPT)
            .await
            .map(|out| detect::parse_probe(&out))
            .unwrap_or_default(),
        None => Vec::new(),
    };
    // Try the usual ports for anything the listing could not see.
    for (port, engine) in detect::WELL_KNOWN {
        if found.iter().any(|d| d.port == port) {
            continue;
        }
        if let Some(greeting) = probe_port(handle.as_ref(), "127.0.0.1", port).await {
            let engine = if engine.is_mysql_family() { detect::refine_mysql(&greeting) } else { engine };
            found.push(Detected { engine, host: "127.0.0.1".into(), port, label: None });
        }
    }
    // MySQL seen by `ss` alone may really be MariaDB; its greeting says.
    for d in found.iter_mut().filter(|d| d.engine == Engine::Mysql && d.label.as_deref().is_none_or(|l| !l.contains("maria"))) {
        if let Some(g) = probe_port(handle.as_ref(), &d.host, d.port).await {
            d.engine = detect::refine_mysql(&g);
        }
    }
    Ok(found)
}

// ---------------------------------------------------------------------------
// Redis

async fn redis(state: &State<'_, AppState>, id: &str) -> Result<Arc<LiveRedis>, String> {
    devops::require(Tool::Databases)?;
    state.db.lock().await.redis(id)
}

#[tauri::command]
pub async fn db_redis_databases(state: State<'_, AppState>, id: String) -> Result<Vec<(i64, i64)>, String> {
    redis(&state, &id).await?.databases().await
}

#[tauri::command]
pub async fn db_redis_scan(
    state: State<'_, AppState>,
    id: String,
    db: i64,
    pattern: String,
    cursor: String,
    count: usize,
) -> Result<ScanPage, String> {
    redis(&state, &id).await?.scan(db, &pattern, &cursor, count.clamp(10, 2000)).await
}

#[tauri::command]
pub async fn db_redis_get(state: State<'_, AppState>, id: String, db: i64, key: String) -> Result<KeyValue, String> {
    redis(&state, &id).await?.get(db, &key).await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase", tag = "status")]
pub enum RedisOutcome {
    Confirm { reason: String },
    Done { reply: Reply },
}

/// Run a command given either as a console line or as ready arguments (the
/// value editors send arguments, so a value with spaces needs no quoting).
#[tauri::command]
pub async fn db_redis_command(
    state: State<'_, AppState>,
    id: String,
    db: i64,
    line: Option<String>,
    args: Option<Vec<String>>,
    confirmed: bool,
) -> Result<RedisOutcome, String> {
    let r = redis(&state, &id).await?;
    let args = match (args, line) {
        (Some(a), _) => a,
        (None, Some(l)) => rds::split_args(&l)?,
        (None, None) => return Err("Type a command".into()),
    };
    let risk = rds::classify(&args);
    if r.config.read_only && !matches!(risk, Risk::Read) {
        return Err("This connection is read-only".into());
    }
    if !confirmed {
        let hold = match &risk {
            Risk::Danger(reason) => Some(reason.clone()),
            Risk::Write if r.config.production => Some("This is a production connection".into()),
            _ => None,
        };
        if let Some(reason) = hold {
            return Ok(RedisOutcome::Confirm { reason });
        }
    }
    Ok(RedisOutcome::Done { reply: r.command(db, &args).await? })
}

#[tauri::command]
pub async fn db_redis_info(state: State<'_, AppState>, id: String) -> Result<Vec<(String, Vec<(String, String)>)>, String> {
    redis(&state, &id).await?.info().await
}

#[tauri::command]
pub async fn db_redis_clients(state: State<'_, AppState>, id: String) -> Result<Vec<std::collections::HashMap<String, String>>, String> {
    redis(&state, &id).await?.clients().await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::types::TlsMode;

    fn conn(read_only: bool, production: bool) -> DbConnection {
        DbConnection {
            id: "c".into(), name: "c".into(), engine: Engine::Postgres, host: String::new(), port: 0,
            username: String::new(), password: None, database: None, file_path: None, route: Route::Direct,
            tls: TlsMode::Prefer, color: None, read_only, production, last_used_at: 0,
        }
    }

    #[test]
    fn read_only_refuses_writes_but_allows_reads_and_transactions() {
        let st = guard::analyze(Engine::Postgres, "begin; select 1; update t set a = 1 where b = 2");
        assert!(gate(&conn(true, false), &st, true).is_err());
        let reads = guard::analyze(Engine::Postgres, "begin; select 1; commit");
        assert!(gate(&conn(true, false), &reads, false).unwrap().is_none());
    }

    #[test]
    fn danger_is_held_back_until_confirmed() {
        let st = guard::analyze(Engine::Postgres, "select 1; drop table t");
        let held = gate(&conn(false, false), &st, false).unwrap().unwrap();
        assert_eq!(held.len(), 1);
        assert!(gate(&conn(false, false), &st, true).unwrap().is_none());
    }

    #[test]
    fn production_holds_back_every_write() {
        let st = guard::analyze(Engine::Postgres, "update t set a = 1 where id = 2");
        assert!(gate(&conn(false, false), &st, false).unwrap().is_none());
        assert_eq!(gate(&conn(false, true), &st, false).unwrap().unwrap().len(), 1);
    }
}
