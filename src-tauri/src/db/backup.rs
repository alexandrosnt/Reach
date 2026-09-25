//! Backup to a SQL file, and running a SQL file back in.
//!
//! Written in Rust rather than by calling `pg_dump` or `mysqldump`, because
//! those do not exist on Android and are often missing or the wrong version
//! on a desktop. The file is plain SQL in the engine's own dialect, laid out
//! so it restores cleanly into an empty database: sequences, then tables
//! without their foreign keys, then the rows, then indexes, keys, triggers,
//! views and routines. Loading rows before the keys means no ordering between
//! tables is needed and no check has to be switched off.
//!
//! It covers what the designer covers. Extensions, custom types, grants and
//! the like are outside it; the file says so at the top.

use std::io::Write;

use serde::{Deserialize, Serialize};

use super::conn::Session;
use super::design::TableDesign;
use super::dialect::{ident, literal, page, qualified};
use super::guard;
use super::introspect;
use super::types::{Engine, ObjectKind};

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DumpOptions {
    /// Only these tables; `None` is all of them.
    pub tables: Option<Vec<String>>,
    pub data: bool,
    pub structure: bool,
    pub views: bool,
    pub routines: bool,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub step: String,
    pub done: usize,
    pub total: usize,
    pub rows: u64,
}

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub tables: usize,
    pub rows: u64,
    pub statements: usize,
    pub failed: Vec<String>,
}

const PAGE: u64 = 1000;
const ROWS_PER_INSERT: usize = 100;

fn end(engine: Engine) -> &'static str {
    if engine == Engine::Mssql {
        "\nGO\n"
    } else {
        ";\n"
    }
}

fn is_binary(engine: Engine, type_name: &str) -> bool {
    let t = type_name.to_lowercase();
    match engine {
        Engine::Mysql | Engine::Mariadb => t.contains("blob") || t.contains("binary"),
        Engine::Mssql => t.contains("binary") || t == "image" || t.starts_with("bigvarbin") || t.starts_with("bigbinary"),
        Engine::Sqlite => t == "blob",
        _ => false,
    }
}

/// A value for an INSERT. Binary values arrive as `0x…` hex; each engine
/// has its own way to write them back.
fn value_sql(engine: Engine, type_name: &str, v: Option<&str>) -> String {
    match v {
        Some(hex) if hex.starts_with("0x") && is_binary(engine, type_name) => match engine {
            Engine::Sqlite => format!("X'{}'", &hex[2..]),
            _ => hex.to_string(),
        },
        other => literal(engine, other),
    }
}

pub async fn dump<W: Write>(
    engine: Engine,
    session: &mut Session,
    database: &str,
    schema: Option<&str>,
    opts: &DumpOptions,
    out: &mut W,
    progress: impl Fn(Progress),
) -> Result<Summary, String> {
    let io = |e: std::io::Error| e.to_string();
    let e = engine;
    let scope = schema.unwrap_or(database);
    let objects = introspect::objects(e, session, scope).await?;
    let wanted = |name: &str| opts.tables.as_ref().is_none_or(|t| t.iter().any(|x| x == name));
    let tables: Vec<String> = objects.iter().filter(|o| o.kind == ObjectKind::Table && wanted(&o.name)).map(|o| o.name.clone()).collect();

    writeln!(out, "-- Reach backup of {} {}", engine_name(e), if scope.is_empty() { database } else { scope }).map_err(io)?;
    writeln!(out, "-- Tables, rows, indexes, keys, triggers, views and routines. Not included:").map_err(io)?;
    writeln!(out, "-- extensions, custom types, grants, and server settings.").map_err(io)?;
    writeln!(out).map_err(io)?;
    match e {
        Engine::Postgres => write!(out, "SET client_encoding = 'UTF8'{}", end(e)).map_err(io)?,
        Engine::Mysql | Engine::Mariadb => write!(out, "SET NAMES utf8mb4{}", end(e)).map_err(io)?,
        // SQLite's foreign keys live in CREATE TABLE, so they exist before
        // the rows arrive; hold the check until every table is loaded.
        Engine::Sqlite => write!(out, "PRAGMA foreign_keys = OFF{}", end(e)).map_err(io)?,
        _ => {}
    }

    let mut summary = Summary::default();
    let mut designs: Vec<TableDesign> = Vec::new();
    for (i, t) in tables.iter().enumerate() {
        progress(Progress { step: format!("Reading {t}"), done: i, total: tables.len(), rows: 0 });
        designs.push(introspect::table(e, session, schema.or((!database.is_empty() && e.is_mysql_family()).then_some(database)), t).await?);
    }

    // Sequences first: serial columns default to them.
    if opts.structure && e == Engine::Postgres {
        for s in objects.iter().filter(|o| o.kind == ObjectKind::Sequence) {
            write!(out, "CREATE SEQUENCE IF NOT EXISTS {}{}", qualified(e, schema, &s.name), end(e)).map_err(io)?;
            summary.statements += 1;
        }
    }

    if opts.structure {
        writeln!(out, "\n-- Tables").map_err(io)?;
        for d in &designs {
            // SQLite can only declare foreign keys inside CREATE TABLE; it
            // does not enforce them unless asked, so loading rows after is fine.
            let foreign_keys = if e == Engine::Sqlite { d.foreign_keys.clone() } else { vec![] };
            let bare = TableDesign { foreign_keys, indexes: vec![], triggers: vec![], original_name: None, ..d.clone() };
            let plan = super::design::create_table(e, &bare)?;
            for s in plan.statements {
                write!(out, "{s}{}", end(e)).map_err(io)?;
                summary.statements += 1;
            }
        }
    }

    if opts.data {
        writeln!(out, "\n-- Rows").map_err(io)?;
        for (i, d) in designs.iter().enumerate() {
            let rows = dump_rows(e, session, d, out, |rows| {
                progress(Progress { step: format!("Copying {}", d.name), done: i, total: designs.len(), rows })
            })
            .await?;
            summary.rows += rows;
            summary.tables += 1;
        }
    } else {
        summary.tables = designs.len();
    }

    if opts.structure {
        writeln!(out, "\n-- Indexes, keys and triggers").map_err(io)?;
        for d in &designs {
            for ix in &d.indexes {
                write!(out, "{}{}", super::design::index_sql(e, d, ix), end(e)).map_err(io)?;
                summary.statements += 1;
            }
            if e != Engine::Sqlite {
                let table = qualified(e, d.schema.as_deref(), &d.name);
                for fk in &d.foreign_keys {
                    let cols = fk.columns.iter().map(|c| ident(e, c)).collect::<Vec<_>>().join(", ");
                    let refs = fk.ref_columns.iter().map(|c| ident(e, c)).collect::<Vec<_>>().join(", ");
                    let mut s = format!(
                        "ALTER TABLE {table} ADD CONSTRAINT {} FOREIGN KEY ({cols}) REFERENCES {} ({refs})",
                        ident(e, &fk.name),
                        qualified(e, fk.ref_schema.as_deref().or(d.schema.as_deref()), &fk.ref_table)
                    );
                    if let Some(a) = fk.on_delete.as_deref().filter(|a| !a.eq_ignore_ascii_case("no action")) {
                        s.push_str(&format!(" ON DELETE {a}"));
                    }
                    if let Some(a) = fk.on_update.as_deref().filter(|a| !a.eq_ignore_ascii_case("no action")) {
                        s.push_str(&format!(" ON UPDATE {a}"));
                    }
                    write!(out, "{s}{}", end(e)).map_err(io)?;
                    summary.statements += 1;
                }
            }
            for tr in &d.triggers {
                write_compound(e, out, &tr.definition)?;
                summary.statements += 1;
            }
            if e == Engine::Postgres {
                for c in d.columns.iter().filter(|c| c.auto_increment) {
                    let t = qualified(e, d.schema.as_deref(), &d.name);
                    write!(
                        out,
                        "SELECT setval(pg_get_serial_sequence({}, {}), COALESCE((SELECT MAX({}) FROM {t}), 1)){}",
                        literal(e, Some(&t)),
                        literal(e, Some(&c.name)),
                        ident(e, &c.name),
                        end(e)
                    )
                    .map_err(io)?;
                }
            }
        }
    }

    if opts.views || opts.routines {
        writeln!(out, "\n-- Views and routines").map_err(io)?;
        for o in &objects {
            let kind = match o.kind {
                ObjectKind::View if opts.views => "view",
                ObjectKind::MaterializedView if opts.views => "materializedView",
                ObjectKind::Function if opts.routines => "function",
                ObjectKind::Procedure if opts.routines => "procedure",
                _ => continue,
            };
            match introspect::definition(e, session, schema.or((e.is_mysql_family()).then_some(database)), &o.name, kind).await {
                Ok(def) if !def.trim().is_empty() => {
                    write_compound(e, out, &def)?;
                    summary.statements += 1;
                }
                _ => summary.failed.push(o.name.clone()),
            }
        }
    }
    if e == Engine::Sqlite {
        write!(out, "PRAGMA foreign_keys = ON{}", end(e)).map_err(io)?;
    }
    out.flush().map_err(io)?;
    progress(Progress { step: "Done".into(), done: designs.len(), total: designs.len(), rows: summary.rows });
    Ok(summary)
}

/// A routine or trigger body. MySQL's needs a DELIMITER around it so its
/// inner semicolons survive both our splitter and the mysql client.
fn write_compound<W: Write>(engine: Engine, out: &mut W, def: &str) -> Result<(), String> {
    let io = |e: std::io::Error| e.to_string();
    let def = def.trim().trim_end_matches(';');
    if engine.is_mysql_family() {
        write!(out, "DELIMITER ;;\n{def};;\nDELIMITER ;\n").map_err(io)
    } else {
        write!(out, "{def}{}", end(engine)).map_err(io)
    }
}

async fn dump_rows<W: Write>(
    e: Engine,
    session: &mut Session,
    d: &TableDesign,
    out: &mut W,
    progress: impl Fn(u64),
) -> Result<u64, String> {
    let io = |e: std::io::Error| e.to_string();
    let table = qualified(e, d.schema.as_deref(), &d.name);
    let cols: Vec<&str> = d.columns.iter().filter(|c| c.generated.is_none()).map(|c| c.name.as_str()).collect();
    if cols.is_empty() {
        return Ok(0);
    }
    let col_list = cols.iter().map(|c| ident(e, c)).collect::<Vec<_>>().join(", ");
    let order = if d.primary_key.is_empty() {
        None
    } else {
        Some(d.primary_key.iter().map(|c| ident(e, c)).collect::<Vec<_>>().join(", "))
    };
    let identity = e == Engine::Mssql && d.columns.iter().any(|c| c.auto_increment);
    if identity {
        write!(out, "SET IDENTITY_INSERT {table} ON{}", end(e)).map_err(io)?;
    }

    let mut total = 0u64;
    let mut offset = 0u64;
    loop {
        let sql = page(e, &format!("SELECT {col_list} FROM {table}"), order.as_deref(), PAGE, offset);
        let rs = session.run(&sql, PAGE as usize).await?;
        if rs.rows.is_empty() {
            break;
        }
        let types: Vec<String> = rs.columns.iter().map(|c| c.type_name.clone()).collect();
        for chunk in rs.rows.chunks(ROWS_PER_INSERT) {
            let values = chunk
                .iter()
                .map(|row| {
                    let vals = row
                        .iter()
                        .enumerate()
                        .map(|(i, v)| value_sql(e, types.get(i).map(String::as_str).unwrap_or(""), v.as_deref()))
                        .collect::<Vec<_>>()
                        .join(", ");
                    format!("({vals})")
                })
                .collect::<Vec<_>>()
                .join(",\n  ");
            write!(out, "INSERT INTO {table} ({col_list}) VALUES\n  {values}{}", end(e)).map_err(io)?;
        }
        total += rs.rows.len() as u64;
        offset += PAGE;
        progress(total);
        if (rs.rows.len() as u64) < PAGE {
            break;
        }
    }
    if identity {
        write!(out, "SET IDENTITY_INSERT {table} OFF{}", end(e)).map_err(io)?;
    }
    Ok(total)
}

fn engine_name(e: Engine) -> &'static str {
    match e {
        Engine::Postgres => "PostgreSQL",
        Engine::Mysql => "MySQL",
        Engine::Mariadb => "MariaDB",
        Engine::Sqlite => "SQLite",
        Engine::Mssql => "SQL Server",
        Engine::Redis => "Redis",
    }
}

/// Run a script statement by statement. Stops at the first failure unless
/// `keep_going`, in which case failures are collected and reported.
pub async fn restore(
    engine: Engine,
    session: &mut Session,
    script: &str,
    keep_going: bool,
    cancelled: impl Fn() -> bool,
    progress: impl Fn(Progress),
) -> Result<Summary, String> {
    let statements = guard::split(engine, script);
    let total = statements.len();
    let mut summary = Summary::default();
    for (i, s) in statements.iter().enumerate() {
        if cancelled() {
            return Err(format!("Stopped after {i} of {total} statements"));
        }
        if i % 25 == 0 {
            progress(Progress { step: "Running".into(), done: i, total, rows: summary.rows });
        }
        match session.run(s, 0).await {
            Ok(rs) => {
                summary.statements += 1;
                summary.rows += rs.rows_affected.unwrap_or(0);
            }
            Err(err) => {
                let head: String = s.chars().take(120).collect();
                let msg = format!("Statement {} ({head}…): {err}", i + 1);
                if !keep_going {
                    return Err(msg);
                }
                summary.failed.push(msg);
            }
        }
    }
    progress(Progress { step: "Done".into(), done: total, total, rows: summary.rows });
    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binary_values_are_written_in_each_engines_form() {
        assert_eq!(value_sql(Engine::Sqlite, "blob", Some("0xdead")), "X'dead'");
        assert_eq!(value_sql(Engine::Mysql, "blob", Some("0xdead")), "0xdead");
        assert_eq!(value_sql(Engine::Mysql, "varchar", Some("0xdead")), "'0xdead'");
        assert_eq!(value_sql(Engine::Postgres, "bytea", Some("\\xdead")), "'\\xdead'");
    }

    #[test]
    fn a_mysql_routine_is_wrapped_so_it_splits_back_whole() {
        let mut out = Vec::new();
        write_compound(Engine::Mysql, &mut out, "CREATE PROCEDURE p() BEGIN SELECT 1; SELECT 2; END").unwrap();
        let text = String::from_utf8(out).unwrap();
        let parts = guard::split(Engine::Mysql, &text);
        assert_eq!(parts, vec!["CREATE PROCEDURE p() BEGIN SELECT 1; SELECT 2; END"]);
    }
}
