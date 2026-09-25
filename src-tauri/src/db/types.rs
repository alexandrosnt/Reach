//! What the Databases workspace passes between Rust and the frontend.
//!
//! Values travel as text. Every engine can render any of its types as text,
//! and the grid shows text anyway; carrying the type name alongside lets the
//! editor and the SQL generator do the right thing without a typed value
//! model per engine.

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum Engine {
    Postgres,
    Mysql,
    Mariadb,
    Sqlite,
    Mssql,
    /// Keys, not tables: its own module and its own screens.
    Redis,
}

impl Engine {
    pub fn default_port(self) -> u16 {
        match self {
            Engine::Postgres => 5432,
            Engine::Mysql | Engine::Mariadb => 3306,
            Engine::Mssql => 1433,
            Engine::Redis => 6379,
            Engine::Sqlite => 0,
        }
    }

    pub fn is_mysql_family(self) -> bool {
        matches!(self, Engine::Mysql | Engine::Mariadb)
    }
}

/// How to reach the server.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "lowercase", rename_all_fields = "camelCase")]
pub enum Route {
    /// Straight to host:port.
    Direct,
    /// Through a saved SSH session, logged in on its own when needed.
    Session { session_id: String },
    /// Through a terminal tab's live SSH connection. Never saved: the
    /// connection ends with the tab.
    Live { connection_id: String },
}

impl Default for Route {
    fn default() -> Self {
        Route::Direct
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum TlsMode {
    /// Use TLS when the server offers it.
    #[default]
    Prefer,
    Require,
    Disable,
}

/// A saved connection, as stored in the vault. The password lives here too,
/// encrypted with everything else, the same way sessions keep theirs.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DbConnection {
    pub id: String,
    pub name: String,
    pub engine: Engine,
    #[serde(default)]
    pub host: String,
    #[serde(default)]
    pub port: u16,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub password: Option<String>,
    /// The database to open first. Empty means the server's default.
    #[serde(default)]
    pub database: Option<String>,
    /// SQLite only.
    #[serde(default)]
    pub file_path: Option<String>,
    #[serde(default)]
    pub route: Route,
    #[serde(default)]
    pub tls: TlsMode,
    /// A label colour; the UI tints the connection with it.
    #[serde(default)]
    pub color: Option<String>,
    /// Refuse anything but reads, in Reach and (where the engine allows) in
    /// the session itself.
    #[serde(default)]
    pub read_only: bool,
    /// Ask before every write, not only the dangerous ones.
    #[serde(default)]
    pub production: bool,
    #[serde(default)]
    pub last_used_at: u64,
}

/// A connection as the frontend lists it: everything but the password.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DbConnectionView {
    #[serde(flatten)]
    pub connection: DbConnection,
    pub has_password: bool,
}

impl From<&DbConnection> for DbConnectionView {
    fn from(c: &DbConnection) -> Self {
        let mut connection = c.clone();
        let has_password = connection.password.as_deref().is_some_and(|p| !p.is_empty());
        connection.password = None;
        Self { connection, has_password }
    }
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ColumnInfo {
    pub name: String,
    /// The engine's own name for the type, lower case.
    pub type_name: String,
}

/// One result set. `rows_affected` is set for statements that change data.
#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct ResultSet {
    pub statement: String,
    pub columns: Vec<ColumnInfo>,
    pub rows: Vec<Vec<Option<String>>>,
    /// More rows existed than were fetched.
    pub truncated: bool,
    pub rows_affected: Option<u64>,
    pub elapsed_ms: u64,
}

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ObjectKind {
    Table,
    View,
    MaterializedView,
    Function,
    Procedure,
    Sequence,
    Trigger,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DbObject {
    pub name: String,
    pub kind: ObjectKind,
    /// Rough row count where the catalog has one for free.
    pub rows: Option<i64>,
    pub comment: Option<String>,
}

/// A server session, for the monitor.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ServerSession {
    pub id: String,
    pub user: Option<String>,
    pub database: Option<String>,
    pub client: Option<String>,
    pub state: Option<String>,
    /// Seconds in the current state or statement.
    pub seconds: Option<i64>,
    pub query: Option<String>,
    /// Reach's own connection, which should not be killed from its own list.
    pub is_self: bool,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ServerInfo {
    pub version: String,
    pub current_database: Option<String>,
    pub current_user: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routes_speak_the_frontends_field_names() {
        let r: Route = serde_json::from_str(r#"{"kind":"session","sessionId":"s1"}"#).unwrap();
        assert_eq!(r, Route::Session { session_id: "s1".into() });
        assert_eq!(serde_json::to_string(&Route::Live { connection_id: "c".into() }).unwrap(), r#"{"kind":"live","connectionId":"c"}"#);
    }
}
