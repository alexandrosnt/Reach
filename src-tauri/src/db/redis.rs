//! Redis: keys and values rather than tables, so it has its own model.
//!
//! The browser walks keys with SCAN — never KEYS, which blocks the server
//! for as long as it takes — and asks TYPE and PTTL for each page in one
//! pipeline. Values are read with a cap so a million-member set cannot freeze
//! the window. Edits and the console both go through [`LiveRedis::command`],
//! with the same guard as the SQL editor: reads run, writes on a read-only
//! connection are refused, and the few commands that wipe or stop a server
//! ask first.

use std::collections::HashMap;

use redis::aio::MultiplexedConnection;
use redis::{ConnectionAddr, IntoConnectionInfo, RedisConnectionInfo, Value};
use serde::Serialize;
use tokio::sync::Mutex;

use super::conn::Forward;
use super::guard::Risk;
use super::types::{DbConnection, TlsMode};

const MAX_ITEMS: isize = 1000;

pub struct LiveRedis {
    pub config: DbConnection,
    host: String,
    port: u16,
    _forward: Option<Forward>,
    /// SELECT changes the database for the whole connection, so each
    /// database gets its own.
    by_db: Mutex<HashMap<i64, MultiplexedConnection>>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct KeyInfo {
    pub key: String,
    pub kind: String,
    /// Milliseconds left, or `None` for a key that does not expire.
    pub ttl_ms: Option<i64>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ScanPage {
    /// "0" when the walk is complete.
    pub cursor: String,
    pub keys: Vec<KeyInfo>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase", tag = "kind")]
pub enum RedisValue {
    String { value: String, binary: bool },
    Hash { entries: Vec<(String, String)> },
    List { items: Vec<String> },
    Set { members: Vec<String> },
    Zset { members: Vec<(String, f64)> },
    Stream { entries: Vec<(String, Vec<(String, String)>)> },
    None,
    Other { type_name: String },
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct KeyValue {
    pub key: String,
    pub ttl_ms: Option<i64>,
    /// How many items the key holds in total (bytes, for a string).
    pub length: i64,
    /// Fewer items were read than it holds.
    pub truncated: bool,
    pub value: RedisValue,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Reply {
    /// As redis-cli prints it.
    pub text: String,
    pub is_error: bool,
}

impl LiveRedis {
    pub async fn open(config: DbConnection, forward: Option<Forward>) -> Result<Self, String> {
        let (host, port) = match &forward {
            Some(f) => ("127.0.0.1".to_string(), f.port),
            None => (config.host.clone(), if config.port == 0 { 6379 } else { config.port }),
        };
        let live = LiveRedis { config, host, port, _forward: forward, by_db: Mutex::new(HashMap::new()) };
        let db = live.default_db();
        live.conn(db).await?;
        Ok(live)
    }

    pub fn default_db(&self) -> i64 {
        self.config.database.as_deref().and_then(|d| d.parse().ok()).unwrap_or(0)
    }

    async fn conn(&self, db: i64) -> Result<MultiplexedConnection, String> {
        let mut map = self.by_db.lock().await;
        if let Some(c) = map.get(&db) {
            return Ok(c.clone());
        }
        let addr = match self.config.tls {
            TlsMode::Disable => ConnectionAddr::Tcp(self.host.clone(), self.port),
            // A tunnel ends on 127.0.0.1, which will never match the server's
            // certificate name; the SSH hop already protects that leg.
            _ => ConnectionAddr::TcpTls {
                host: self.host.clone(),
                port: self.port,
                insecure: self._forward.is_some(),
                tls_params: None,
            },
        };
        let mut settings = RedisConnectionInfo::default().set_db(db).set_lib_name("reach", env!("CARGO_PKG_VERSION"));
        if !self.config.username.is_empty() {
            settings = settings.set_username(&self.config.username);
        }
        if let Some(p) = self.config.password.as_deref().filter(|p| !p.is_empty()) {
            settings = settings.set_password(p);
        }
        let info = addr.into_connection_info().map_err(|e| e.to_string())?.set_redis_settings(settings);
        let client = redis::Client::open(info).map_err(|e| e.to_string())?;
        let conn = tokio::time::timeout(std::time::Duration::from_secs(15), client.get_multiplexed_async_connection())
            .await
            .map_err(|_| format!("Timed out reaching {}:{}", self.host, self.port))?
            .map_err(|e| describe(&e))?;
        map.insert(db, conn.clone());
        Ok(conn)
    }

    /// Databases that hold keys, from INFO keyspace, plus the default one.
    pub async fn databases(&self) -> Result<Vec<(i64, i64)>, String> {
        let mut c = self.conn(self.default_db()).await?;
        let info: String = redis::cmd("INFO").arg("keyspace").query_async(&mut c).await.map_err(|e| describe(&e))?;
        let mut dbs: Vec<(i64, i64)> = info
            .lines()
            .filter_map(|l| {
                let (name, rest) = l.split_once(':')?;
                let n = name.strip_prefix("db")?.parse().ok()?;
                let keys = rest.split(',').find_map(|kv| kv.strip_prefix("keys="))?.parse().ok()?;
                Some((n, keys))
            })
            .collect();
        if !dbs.iter().any(|(n, _)| *n == self.default_db()) {
            dbs.push((self.default_db(), 0));
        }
        dbs.sort();
        Ok(dbs)
    }

    pub async fn scan(&self, db: i64, pattern: &str, cursor: &str, count: usize) -> Result<ScanPage, String> {
        let mut c = self.conn(db).await?;
        let (next, keys): (String, Vec<Vec<u8>>) = redis::cmd("SCAN")
            .arg(cursor)
            .arg("MATCH")
            .arg(if pattern.is_empty() { "*" } else { pattern })
            .arg("COUNT")
            .arg(count)
            .query_async(&mut c)
            .await
            .map_err(|e| describe(&e))?;
        let mut pipe = redis::pipe();
        for k in &keys {
            pipe.cmd("TYPE").arg(k).cmd("PTTL").arg(k);
        }
        let meta: Vec<Value> = if keys.is_empty() { vec![] } else { pipe.query_async(&mut c).await.map_err(|e| describe(&e))? };
        let keys = keys
            .into_iter()
            .enumerate()
            .map(|(i, k)| KeyInfo {
                key: String::from_utf8_lossy(&k).into_owned(),
                kind: meta.get(i * 2).map(value_text).unwrap_or_default(),
                ttl_ms: match meta.get(i * 2 + 1) {
                    Some(Value::Int(n)) if *n >= 0 => Some(*n),
                    _ => None,
                },
            })
            .collect();
        Ok(ScanPage { cursor: next, keys })
    }

    pub async fn get(&self, db: i64, key: &str) -> Result<KeyValue, String> {
        let mut c = self.conn(db).await?;
        let (kind, ttl): (String, i64) = redis::pipe()
            .cmd("TYPE")
            .arg(key)
            .cmd("PTTL")
            .arg(key)
            .query_async(&mut c)
            .await
            .map_err(|e| describe(&e))?;
        let q = |cmd: &str| redis::cmd(cmd).arg(key).clone();
        let (value, length) = match kind.as_str() {
            "string" => {
                let raw: Vec<u8> = q("GET").query_async(&mut c).await.map_err(|e| describe(&e))?;
                let len = raw.len() as i64;
                let value = match String::from_utf8(raw) {
                    Ok(s) => RedisValue::String { value: s, binary: false },
                    Err(e) => RedisValue::String { value: hex(e.as_bytes()), binary: true },
                };
                (value, len)
            }
            "hash" => {
                let len: i64 = q("HLEN").query_async(&mut c).await.map_err(|e| describe(&e))?;
                let (_, flat): (String, Vec<Vec<u8>>) = q("HSCAN").arg(0).arg("COUNT").arg(MAX_ITEMS).query_async(&mut c).await.map_err(|e| describe(&e))?;
                let entries = flat.chunks(2).map(|p| (lossy(&p[0]), p.get(1).map(|v| lossy(v)).unwrap_or_default())).collect();
                (RedisValue::Hash { entries }, len)
            }
            "list" => {
                let len: i64 = q("LLEN").query_async(&mut c).await.map_err(|e| describe(&e))?;
                let items: Vec<Vec<u8>> = q("LRANGE").arg(0).arg(MAX_ITEMS - 1).query_async(&mut c).await.map_err(|e| describe(&e))?;
                (RedisValue::List { items: items.iter().map(|v| lossy(v)).collect() }, len)
            }
            "set" => {
                let len: i64 = q("SCARD").query_async(&mut c).await.map_err(|e| describe(&e))?;
                let (_, members): (String, Vec<Vec<u8>>) = q("SSCAN").arg(0).arg("COUNT").arg(MAX_ITEMS).query_async(&mut c).await.map_err(|e| describe(&e))?;
                (RedisValue::Set { members: members.iter().map(|v| lossy(v)).collect() }, len)
            }
            "zset" => {
                let len: i64 = q("ZCARD").query_async(&mut c).await.map_err(|e| describe(&e))?;
                let flat: Vec<Vec<u8>> = q("ZRANGE").arg(0).arg(MAX_ITEMS - 1).arg("WITHSCORES").query_async(&mut c).await.map_err(|e| describe(&e))?;
                let members = flat.chunks(2).map(|p| (lossy(&p[0]), p.get(1).and_then(|s| lossy(s).parse().ok()).unwrap_or(0.0))).collect();
                (RedisValue::Zset { members }, len)
            }
            "stream" => {
                let len: i64 = q("XLEN").query_async(&mut c).await.map_err(|e| describe(&e))?;
                let raw: Value = q("XREVRANGE").arg("+").arg("-").arg("COUNT").arg(500).query_async(&mut c).await.map_err(|e| describe(&e))?;
                (RedisValue::Stream { entries: stream_entries(raw) }, len)
            }
            "none" => (RedisValue::None, 0),
            other => (RedisValue::Other { type_name: other.to_string() }, 0),
        };
        let shown = match &value {
            RedisValue::Hash { entries } => entries.len() as i64,
            RedisValue::List { items } => items.len() as i64,
            RedisValue::Set { members } => members.len() as i64,
            RedisValue::Zset { members } => members.len() as i64,
            RedisValue::Stream { entries } => entries.len() as i64,
            _ => length,
        };
        Ok(KeyValue { key: key.to_string(), ttl_ms: (ttl >= 0).then_some(ttl), length, truncated: shown < length, value })
    }

    /// Run one command, already split into arguments.
    pub async fn command(&self, db: i64, args: &[String]) -> Result<Reply, String> {
        let Some(name) = args.first() else { return Err("Type a command".into()) };
        if name.eq_ignore_ascii_case("select") {
            return Err("Pick the database in the list instead of SELECT; each database has its own connection".into());
        }
        let mut c = self.conn(db).await?;
        let mut cmd = redis::cmd(name);
        for a in &args[1..] {
            cmd.arg(a);
        }
        match cmd.query_async::<Value>(&mut c).await {
            Ok(v) => Ok(Reply { text: format_reply(&v, 0), is_error: false }),
            Err(e) => Ok(Reply { text: format!("(error) {}", describe(&e)), is_error: true }),
        }
    }

    pub async fn info(&self) -> Result<Vec<(String, Vec<(String, String)>)>, String> {
        let mut c = self.conn(self.default_db()).await?;
        let text: String = redis::cmd("INFO").query_async(&mut c).await.map_err(|e| describe(&e))?;
        Ok(parse_info(&text))
    }

    /// CLIENT LIST, one map per client.
    pub async fn clients(&self) -> Result<Vec<HashMap<String, String>>, String> {
        let mut c = self.conn(self.default_db()).await?;
        let text: String = redis::cmd("CLIENT").arg("LIST").query_async(&mut c).await.map_err(|e| describe(&e))?;
        Ok(text
            .lines()
            .map(|l| l.split(' ').filter_map(|kv| kv.split_once('=')).map(|(k, v)| (k.to_string(), v.to_string())).collect())
            .collect())
    }
}

fn describe(e: &redis::RedisError) -> String {
    let s = e.to_string();
    if s.contains("NOAUTH") || s.contains("WRONGPASS") {
        "The server wants a password, or the one given was refused".into()
    } else {
        s
    }
}

fn lossy(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

fn hex(b: &[u8]) -> String {
    let mut s = String::from("0x");
    for x in b {
        s.push_str(&format!("{x:02x}"));
    }
    s
}

fn value_text(v: &Value) -> String {
    match v {
        Value::SimpleString(s) => s.clone(),
        Value::BulkString(b) => lossy(b),
        Value::Okay => "OK".into(),
        Value::Int(i) => i.to_string(),
        other => format!("{other:?}"),
    }
}

fn stream_entries(v: Value) -> Vec<(String, Vec<(String, String)>)> {
    let Value::Array(items) = v else { return vec![] };
    items
        .into_iter()
        .filter_map(|e| {
            let Value::Array(mut pair) = e else { return None };
            if pair.len() != 2 {
                return None;
            }
            let fields = pair.pop()?;
            let id = value_text(&pair.pop()?);
            let Value::Array(flat) = fields else { return Some((id, vec![])) };
            let kv = flat.chunks(2).map(|p| (value_text(&p[0]), p.get(1).map(value_text).unwrap_or_default())).collect();
            Some((id, kv))
        })
        .collect()
}

/// A reply the way redis-cli shows it.
pub fn format_reply(v: &Value, indent: usize) -> String {
    match v {
        Value::Nil => "(nil)".into(),
        Value::Int(i) => format!("(integer) {i}"),
        Value::Double(d) => format!("(double) {d}"),
        Value::Boolean(b) => format!("({b})"),
        Value::Okay => "OK".into(),
        Value::SimpleString(s) => s.clone(),
        Value::BulkString(b) => match std::str::from_utf8(b) {
            Ok(s) => format!("{s:?}"),
            Err(_) => hex(b),
        },
        Value::Array(items) | Value::Set(items) => {
            if items.is_empty() {
                return "(empty array)".into();
            }
            let width = items.len().to_string().len();
            items
                .iter()
                .enumerate()
                .map(|(i, item)| {
                    let pad = if i == 0 { String::new() } else { " ".repeat(indent) };
                    format!("{pad}{:>width$}) {}", i + 1, format_reply(item, indent + width + 2))
                })
                .collect::<Vec<_>>()
                .join("\n")
        }
        Value::Map(pairs) => pairs
            .iter()
            .enumerate()
            .map(|(i, (k, val))| {
                let pad = if i == 0 { String::new() } else { " ".repeat(indent) };
                format!("{pad}{}) {} => {}", i + 1, format_reply(k, 0), format_reply(val, indent + 4))
            })
            .collect::<Vec<_>>()
            .join("\n"),
        Value::VerbatimString { text, .. } => text.clone(),
        Value::ServerError(e) => format!("(error) {e:?}"),
        other => format!("{other:?}"),
    }
}

fn parse_info(text: &str) -> Vec<(String, Vec<(String, String)>)> {
    let mut out: Vec<(String, Vec<(String, String)>)> = Vec::new();
    for line in text.lines().map(str::trim) {
        if let Some(section) = line.strip_prefix("# ") {
            out.push((section.to_string(), Vec::new()));
        } else if let Some((k, v)) = line.split_once(':') {
            if out.is_empty() {
                out.push(("Server".into(), Vec::new()));
            }
            out.last_mut().unwrap().1.push((k.to_string(), v.to_string()));
        }
    }
    out
}

/// Split a console line into arguments the way redis-cli does: whitespace
/// separates, double quotes allow escapes, single quotes are literal.
pub fn split_args(line: &str) -> Result<Vec<String>, String> {
    let mut args = Vec::new();
    let mut cur = String::new();
    let mut chars = line.chars().peekable();
    let mut in_arg = false;
    while let Some(c) = chars.next() {
        match c {
            '"' => {
                in_arg = true;
                loop {
                    match chars.next() {
                        None => return Err("Unclosed double quote".into()),
                        Some('"') => break,
                        Some('\\') => match chars.next() {
                            Some('n') => cur.push('\n'),
                            Some('t') => cur.push('\t'),
                            Some('r') => cur.push('\r'),
                            Some('x') => {
                                let h: String = chars.by_ref().take(2).collect();
                                let b = u8::from_str_radix(&h, 16).map_err(|_| format!("Bad escape \\x{h}"))?;
                                cur.push(b as char);
                            }
                            Some(o) => cur.push(o),
                            None => return Err("Unclosed double quote".into()),
                        },
                        Some(o) => cur.push(o),
                    }
                }
            }
            '\'' => {
                in_arg = true;
                loop {
                    match chars.next() {
                        None => return Err("Unclosed single quote".into()),
                        Some('\'') => break,
                        Some(o) => cur.push(o),
                    }
                }
            }
            c if c.is_whitespace() => {
                if in_arg {
                    args.push(std::mem::take(&mut cur));
                    in_arg = false;
                }
            }
            c => {
                in_arg = true;
                cur.push(c);
            }
        }
    }
    if in_arg {
        args.push(cur);
    }
    Ok(args)
}

/// Commands that only read. Anything not listed counts as a write.
const READS: &[&str] = &[
    "get", "mget", "strlen", "getrange", "exists", "type", "ttl", "pttl", "expiretime", "keys", "scan", "hget", "hmget",
    "hgetall", "hkeys", "hvals", "hlen", "hexists", "hscan", "hstrlen", "lrange", "llen", "lindex", "lpos", "smembers",
    "scard", "sismember", "smismember", "sscan", "srandmember", "sinter", "sunion", "sdiff", "zrange", "zrangebyscore",
    "zrevrange", "zrevrangebyscore", "zscore", "zmscore", "zcard", "zcount", "zrank", "zrevrank", "zscan", "xrange",
    "xrevrange", "xlen", "xinfo", "xread", "info", "dbsize", "ping", "echo", "time", "memory", "object", "randomkey",
    "client", "slowlog", "latency", "lastsave", "role", "command", "dump", "bitcount", "bitpos", "getbit", "pfcount",
    "geopos", "geodist", "georadius_ro", "geosearch", "json.get", "ft.search", "ft.info",
];

/// Risk of a Redis command. KEYS is a read but blocks the server while it
/// walks every key, so it asks first like a destructive command would.
pub fn classify(args: &[String]) -> Risk {
    let Some(name) = args.first().map(|a| a.to_lowercase()) else { return Risk::Read };
    match name.as_str() {
        "flushall" | "flushdb" => Risk::Danger(format!("{} deletes every key", name.to_uppercase())),
        "shutdown" => Risk::Danger("SHUTDOWN stops the server".into()),
        "debug" => Risk::Danger("DEBUG can crash or stall the server".into()),
        "keys" => Risk::Danger("KEYS blocks the server while it scans every key; the key list uses SCAN instead".into()),
        "config" if args.get(1).is_some_and(|a| !a.eq_ignore_ascii_case("get")) => {
            Risk::Danger("CONFIG changes the running server".into())
        }
        "config" => Risk::Read,
        "client" if args.get(1).is_some_and(|a| a.eq_ignore_ascii_case("kill")) => Risk::Write,
        "script" if args.get(1).is_some_and(|a| a.eq_ignore_ascii_case("flush")) => Risk::Danger("SCRIPT FLUSH removes every cached script".into()),
        "del" | "unlink" if args.len() > 11 => Risk::Danger(format!("{} deletes {} keys at once", name.to_uppercase(), args.len() - 1)),
        "multi" | "exec" | "discard" | "watch" | "unwatch" => Risk::Session,
        n if READS.contains(&n) => Risk::Read,
        _ => Risk::Write,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn console_lines_split_like_redis_cli() {
        assert_eq!(split_args(r#"SET "a key" 'it''s' plain"#).unwrap(), vec!["SET", "a key", "its", "plain"]);
        assert_eq!(split_args(r#"SET k "line\nbreak \x41""#).unwrap(), vec!["SET", "k", "line\nbreak A"]);
        assert_eq!(split_args("  GET   k  ").unwrap(), vec!["GET", "k"]);
        assert_eq!(split_args(r#"SET k """#).unwrap(), vec!["SET", "k", ""]);
        assert!(split_args(r#"GET "k"#).is_err());
    }

    #[test]
    fn replies_render_like_redis_cli() {
        let v = Value::Array(vec![Value::BulkString(b"a".to_vec()), Value::Int(2), Value::Nil]);
        assert_eq!(format_reply(&v, 0), "1) \"a\"\n2) (integer) 2\n3) (nil)");
        assert_eq!(format_reply(&Value::Array(vec![]), 0), "(empty array)");
        assert_eq!(format_reply(&Value::Okay, 0), "OK");
    }

    #[test]
    fn wipes_and_blocking_scans_ask_first() {
        let a = |s: &str| split_args(s).unwrap();
        assert!(matches!(classify(&a("FLUSHALL")), Risk::Danger(_)));
        assert!(matches!(classify(&a("keys *")), Risk::Danger(_)));
        assert!(matches!(classify(&a("config set maxmemory 1")), Risk::Danger(_)));
        assert_eq!(classify(&a("config get maxmemory")), Risk::Read);
        assert_eq!(classify(&a("HGETALL h")), Risk::Read);
        assert_eq!(classify(&a("HSET h f v")), Risk::Write);
    }

    #[test]
    fn info_parses_into_sections() {
        let s = parse_info("# Server\r\nredis_version:7.2.4\r\n\r\n# Keyspace\r\ndb0:keys=3,expires=0\r\n");
        assert_eq!(s[0].0, "Server");
        assert_eq!(s[0].1[0], ("redis_version".into(), "7.2.4".into()));
        assert_eq!(s[1].1[0].0, "db0");
    }
}
