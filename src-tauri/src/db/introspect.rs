//! Reading structure back out of each engine's catalog: the object tree, a
//! table as the designer edits it, definitions, server info and sessions.
//!
//! Every query here runs through the same text-only [`Session::run`] the
//! editor uses, and names are embedded as literals rendered by
//! [`super::dialect::literal`], never spliced in raw.

use std::collections::HashMap;

use super::conn::Session;
use super::design::{CheckDef, ColumnDef, ForeignKeyDef, IndexColumn, IndexDef, TableDesign, TableOptions, TriggerDef};
use super::dialect::{ident, literal, qualified};
use super::types::{DbObject, Engine, ObjectKind, ResultSet, ServerInfo, ServerSession};

/// Rows keyed by lower-cased column name.
struct Rows(Vec<HashMap<String, Option<String>>>);

impl Rows {
    fn from(rs: ResultSet) -> Self {
        let names: Vec<String> = rs.columns.iter().map(|c| c.name.to_lowercase()).collect();
        Rows(
            rs.rows
                .into_iter()
                .map(|r| names.iter().cloned().zip(r).collect())
                .collect(),
        )
    }
}

fn s(row: &HashMap<String, Option<String>>, col: &str) -> Option<String> {
    row.get(col).cloned().flatten()
}

fn st(row: &HashMap<String, Option<String>>, col: &str) -> String {
    s(row, col).unwrap_or_default()
}

fn truthy(row: &HashMap<String, Option<String>>, col: &str) -> bool {
    matches!(s(row, col).as_deref().map(str::to_lowercase).as_deref(), Some("t" | "true" | "1" | "yes" | "y"))
}

fn num<T: std::str::FromStr>(row: &HashMap<String, Option<String>>, col: &str) -> Option<T> {
    s(row, col).and_then(|v| v.split('.').next().unwrap_or("").parse().ok())
}

async fn q(session: &mut Session, sql: &str) -> Result<Rows, String> {
    Ok(Rows::from(session.run(sql, usize::MAX).await?))
}

fn lit(e: Engine, v: &str) -> String {
    // N'' is right for SQL Server catalog names too.
    literal(e, Some(v))
}

// ---------------------------------------------------------------------------
// Tree

pub async fn databases(engine: Engine, session: &mut Session) -> Result<Vec<String>, String> {
    let sql = match engine {
        Engine::Postgres => "SELECT datname AS name FROM pg_database WHERE NOT datistemplate AND datallowconn ORDER BY 1",
        Engine::Mysql | Engine::Mariadb => "SELECT SCHEMA_NAME AS name FROM information_schema.SCHEMATA ORDER BY 1",
        Engine::Mssql => "SELECT name FROM sys.databases WHERE state = 0 AND HAS_DBACCESS(name) = 1 ORDER BY name",
        Engine::Sqlite => return Ok(vec!["main".into()]),
        Engine::Redis => return Ok(vec![]),
    };
    Ok(q(session, sql).await?.0.iter().map(|r| st(r, "name")).collect())
}

/// Schemas inside a database. MySQL and SQLite have none below the database.
pub async fn schemas(engine: Engine, session: &mut Session) -> Result<Vec<String>, String> {
    let sql = match engine {
        Engine::Postgres => {
            "SELECT nspname AS name FROM pg_namespace WHERE nspname NOT LIKE 'pg\\_%' AND nspname <> 'information_schema' ORDER BY nspname = 'public' DESC, 1"
        }
        Engine::Mssql => {
            "SELECT name FROM sys.schemas WHERE name NOT IN ('sys', 'INFORMATION_SCHEMA', 'guest') AND name NOT LIKE 'db[_]%' ORDER BY CASE WHEN name = 'dbo' THEN 0 ELSE 1 END, name"
        }
        _ => return Ok(vec![]),
    };
    Ok(q(session, sql).await?.0.iter().map(|r| st(r, "name")).collect())
}

/// Tables, views, routines and sequences in one schema (the database, for
/// MySQL; everything, for SQLite).
pub async fn objects(engine: Engine, session: &mut Session, schema: &str) -> Result<Vec<DbObject>, String> {
    let mut out = Vec::new();
    match engine {
        Engine::Postgres => {
            let rows = q(session, &format!(
                "SELECT c.relname AS name, c.relkind AS kind, c.reltuples::bigint AS rows, obj_description(c.oid, 'pg_class') AS comment \
                 FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace \
                 WHERE n.nspname = {} AND c.relkind IN ('r','p','v','m','S') ORDER BY 1",
                lit(engine, schema)
            )).await?;
            for r in rows.0 {
                let kind = match st(&r, "kind").as_str() {
                    "v" => ObjectKind::View,
                    "m" => ObjectKind::MaterializedView,
                    "S" => ObjectKind::Sequence,
                    _ => ObjectKind::Table,
                };
                let rows = num::<i64>(&r, "rows").filter(|n| *n >= 0 && kind == ObjectKind::Table);
                out.push(DbObject { name: st(&r, "name"), kind, rows, comment: s(&r, "comment") });
            }
            let routines = q(session, &format!(
                "SELECT p.proname || '(' || pg_get_function_identity_arguments(p.oid) || ')' AS name, p.prokind AS kind \
                 FROM pg_proc p JOIN pg_namespace n ON n.oid = p.pronamespace \
                 WHERE n.nspname = {} AND p.prokind IN ('f','p') ORDER BY 1",
                lit(engine, schema)
            )).await?;
            for r in routines.0 {
                let kind = if st(&r, "kind") == "p" { ObjectKind::Procedure } else { ObjectKind::Function };
                out.push(DbObject { name: st(&r, "name"), kind, rows: None, comment: None });
            }
        }
        Engine::Mysql | Engine::Mariadb => {
            let rows = q(session, &format!(
                "SELECT TABLE_NAME AS name, TABLE_TYPE AS kind, TABLE_ROWS AS `rows`, TABLE_COMMENT AS comment \
                 FROM information_schema.TABLES WHERE TABLE_SCHEMA = {} ORDER BY 1",
                lit(engine, schema)
            )).await?;
            for r in rows.0 {
                let kind = if st(&r, "kind").contains("VIEW") { ObjectKind::View } else { ObjectKind::Table };
                let rows = if kind == ObjectKind::Table { num(&r, "rows") } else { None };
                out.push(DbObject { name: st(&r, "name"), kind, rows, comment: s(&r, "comment").filter(|c| !c.is_empty()) });
            }
            let routines = q(session, &format!(
                "SELECT ROUTINE_NAME AS name, ROUTINE_TYPE AS kind FROM information_schema.ROUTINES WHERE ROUTINE_SCHEMA = {} ORDER BY 1",
                lit(engine, schema)
            )).await?;
            for r in routines.0 {
                let kind = if st(&r, "kind") == "PROCEDURE" { ObjectKind::Procedure } else { ObjectKind::Function };
                out.push(DbObject { name: st(&r, "name"), kind, rows: None, comment: None });
            }
        }
        Engine::Mssql => {
            let rows = q(session, &format!(
                "SELECT o.name, o.type AS kind, \
                   (SELECT SUM(p.rows) FROM sys.partitions p WHERE p.object_id = o.object_id AND p.index_id IN (0, 1)) AS rows, \
                   CAST(ep.value AS nvarchar(4000)) AS comment \
                 FROM sys.objects o \
                 LEFT JOIN sys.extended_properties ep ON ep.major_id = o.object_id AND ep.minor_id = 0 AND ep.name = 'MS_Description' \
                 WHERE o.schema_id = SCHEMA_ID({}) AND o.type IN ('U','V','P','FN','IF','TF','SO') AND o.is_ms_shipped = 0 \
                 ORDER BY o.name",
                lit(engine, schema)
            )).await?;
            for r in rows.0 {
                let kind = match st(&r, "kind").trim() {
                    "V" => ObjectKind::View,
                    "P" => ObjectKind::Procedure,
                    "FN" | "IF" | "TF" => ObjectKind::Function,
                    "SO" => ObjectKind::Sequence,
                    _ => ObjectKind::Table,
                };
                let rows = if kind == ObjectKind::Table { num(&r, "rows") } else { None };
                out.push(DbObject { name: st(&r, "name"), kind, rows, comment: s(&r, "comment") });
            }
        }
        Engine::Sqlite => {
            let rows = q(session, "SELECT name, type AS kind FROM sqlite_master WHERE type IN ('table','view') AND name NOT LIKE 'sqlite\\_%' ESCAPE '\\' ORDER BY name").await?;
            for r in rows.0 {
                let kind = if st(&r, "kind") == "view" { ObjectKind::View } else { ObjectKind::Table };
                out.push(DbObject { name: st(&r, "name"), kind, rows: None, comment: None });
            }
        }
        Engine::Redis => {}
    }
    Ok(out)
}

/// Every column of every table in a schema, for autocomplete.
pub async fn completion_columns(engine: Engine, session: &mut Session, schema: &str) -> Result<HashMap<String, Vec<String>>, String> {
    let mut out: HashMap<String, Vec<String>> = HashMap::new();
    match engine {
        Engine::Sqlite => {
            let tables = q(session, "SELECT name FROM sqlite_master WHERE type IN ('table','view') AND name NOT LIKE 'sqlite\\_%' ESCAPE '\\'").await?;
            for t in tables.0 {
                let name = st(&t, "name");
                let cols = q(session, &format!("SELECT name FROM pragma_table_info({})", lit(engine, &name))).await?;
                out.insert(name, cols.0.iter().map(|c| st(c, "name")).collect());
            }
        }
        Engine::Redis => {}
        _ => {
            let rows = q(session, &format!(
                "SELECT TABLE_NAME AS t, COLUMN_NAME AS c FROM INFORMATION_SCHEMA.COLUMNS WHERE TABLE_SCHEMA = {} ORDER BY TABLE_NAME, ORDINAL_POSITION",
                lit(engine, schema)
            )).await?;
            for r in rows.0 {
                out.entry(st(&r, "t")).or_default().push(st(&r, "c"));
            }
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Table design

pub async fn table(engine: Engine, session: &mut Session, schema: Option<&str>, name: &str) -> Result<TableDesign, String> {
    let mut t = match engine {
        Engine::Postgres => pg_table(session, schema.unwrap_or("public"), name).await?,
        Engine::Mysql | Engine::Mariadb => mysql_table(engine, session, schema.unwrap_or(""), name).await?,
        Engine::Mssql => mssql_table(session, schema.unwrap_or("dbo"), name).await?,
        Engine::Sqlite => sqlite_table(session, name).await?,
        Engine::Redis => return Err("Redis has no tables".into()),
    };
    if t.columns.is_empty() {
        return Err(format!("Table {name} was not found"));
    }
    t.schema = schema.map(str::to_string).filter(|s| !s.is_empty());
    t.original_name = Some(name.to_string());
    t.name = name.to_string();
    for c in &mut t.columns {
        c.original = Some(c.name.clone());
    }
    for i in &mut t.indexes {
        i.original = Some(i.name.clone());
    }
    for f in &mut t.foreign_keys {
        f.original = Some(f.name.clone());
    }
    for c in &mut t.checks {
        c.original = Some(c.name.clone());
    }
    for tr in &mut t.triggers {
        tr.original = Some(tr.name.clone());
    }
    Ok(t)
}

fn pg_type_name(udt: &str) -> String {
    let (base, array) = match udt.strip_prefix('_') {
        Some(b) => (b, "[]"),
        None => (udt, ""),
    };
    let nice = match base {
        "int2" => "smallint",
        "int4" => "integer",
        "int8" => "bigint",
        "float4" => "real",
        "float8" => "double precision",
        "bool" => "boolean",
        "bpchar" => "char",
        other => other,
    };
    format!("{nice}{array}")
}

fn pg_action(code: &str) -> Option<String> {
    Some(
        match code {
            "r" => "RESTRICT",
            "c" => "CASCADE",
            "n" => "SET NULL",
            "d" => "SET DEFAULT",
            _ => return None,
        }
        .into(),
    )
}

async fn pg_table(session: &mut Session, schema: &str, name: &str) -> Result<TableDesign, String> {
    let e = Engine::Postgres;
    let rel = format!("{}::regclass", lit(e, &qualified(e, Some(schema), name)));
    let mut t = TableDesign::default();

    let cols = q(session, &format!(
        "SELECT c.column_name AS name, c.udt_name AS udt, c.data_type, c.character_maximum_length AS len, \
           c.numeric_precision AS prec, c.numeric_scale AS scale, c.is_nullable, c.column_default AS dflt, \
           c.is_identity, c.is_generated, c.generation_expression AS gen, c.collation_name AS coll, \
           col_description({rel}, c.ordinal_position::int) AS comment \
         FROM information_schema.columns c WHERE c.table_schema = {} AND c.table_name = {} ORDER BY c.ordinal_position",
        lit(e, schema), lit(e, name)
    )).await?;
    for r in cols.0 {
        let udt = st(&r, "udt");
        let data_type = pg_type_name(&udt);
        let (length, scale) = match udt.as_str() {
            "varchar" | "bpchar" | "bit" | "varbit" => (num(&r, "len"), None),
            "numeric" => (num(&r, "prec"), num::<u32>(&r, "prec").and(num(&r, "scale"))),
            _ => (None, None),
        };
        let generated = (st(&r, "is_generated") == "ALWAYS").then(|| st(&r, "gen"));
        t.columns.push(ColumnDef {
            name: st(&r, "name"),
            data_type,
            length,
            scale,
            not_null: st(&r, "is_nullable") == "NO",
            default: s(&r, "dflt"),
            auto_increment: st(&r, "is_identity") == "YES",
            comment: s(&r, "comment"),
            collation: s(&r, "coll"),
            generated_stored: generated.is_some(),
            generated,
            ..Default::default()
        });
    }

    let cons = q(session, &format!(
        "SELECT con.conname AS name, con.contype AS kind, a.attname AS col, k.ord \
         FROM pg_constraint con CROSS JOIN LATERAL unnest(con.conkey) WITH ORDINALITY k(attnum, ord) \
         JOIN pg_attribute a ON a.attrelid = con.conrelid AND a.attnum = k.attnum \
         WHERE con.conrelid = {rel} AND con.contype IN ('p','u') ORDER BY con.conname, k.ord"
    )).await?;
    let mut uniques: Vec<IndexDef> = Vec::new();
    for r in cons.0 {
        let cname = st(&r, "name");
        if st(&r, "kind") == "p" {
            t.primary_key_name = Some(cname);
            t.primary_key.push(st(&r, "col"));
        } else {
            match uniques.iter_mut().find(|u| u.name == cname) {
                Some(u) => u.columns.push(IndexColumn { name: st(&r, "col"), ..Default::default() }),
                None => uniques.push(IndexDef {
                    name: cname,
                    unique: true,
                    constraint: true,
                    columns: vec![IndexColumn { name: st(&r, "col"), ..Default::default() }],
                    ..Default::default()
                }),
            }
        }
    }

    let idx = q(session, &format!(
        "SELECT ic.relname AS name, i.indisunique AS uniq, am.amname AS method, pg_get_expr(i.indpred, i.indrelid) AS pred, \
           k.ord, pg_get_indexdef(i.indexrelid, k.ord::int, true) AS col, (i.indoption[k.ord - 1] & 1) = 1 AS descending \
         FROM pg_index i JOIN pg_class ic ON ic.oid = i.indexrelid JOIN pg_am am ON am.oid = ic.relam \
         CROSS JOIN LATERAL generate_series(1, i.indnkeyatts) AS k(ord) \
         WHERE i.indrelid = {rel} AND NOT EXISTS (SELECT 1 FROM pg_constraint c WHERE c.conindid = i.indexrelid AND c.contype IN ('p','u','x')) \
         ORDER BY ic.relname, k.ord"
    )).await?;
    for r in idx.0 {
        let iname = st(&r, "name");
        let col = IndexColumn { name: st(&r, "col").trim_matches('"').to_string(), descending: truthy(&r, "descending"), length: None };
        match t.indexes.iter_mut().find(|i| i.name == iname) {
            Some(i) => i.columns.push(col),
            None => t.indexes.push(IndexDef {
                name: iname,
                unique: truthy(&r, "uniq"),
                method: s(&r, "method"),
                where_clause: s(&r, "pred"),
                columns: vec![col],
                ..Default::default()
            }),
        }
    }
    t.indexes.extend(uniques);

    let fks = q(session, &format!(
        "SELECT con.conname AS name, a.attname AS col, fa.attname AS ref_col, fn.nspname AS ref_schema, fc.relname AS ref_table, \
           con.confupdtype AS upd, con.confdeltype AS del \
         FROM pg_constraint con CROSS JOIN LATERAL unnest(con.conkey, con.confkey) WITH ORDINALITY k(attnum, fattnum, ord) \
         JOIN pg_attribute a ON a.attrelid = con.conrelid AND a.attnum = k.attnum \
         JOIN pg_attribute fa ON fa.attrelid = con.confrelid AND fa.attnum = k.fattnum \
         JOIN pg_class fc ON fc.oid = con.confrelid JOIN pg_namespace fn ON fn.oid = fc.relnamespace \
         WHERE con.conrelid = {rel} AND con.contype = 'f' ORDER BY con.conname, k.ord"
    )).await?;
    for r in fks.0 {
        let fname = st(&r, "name");
        match t.foreign_keys.iter_mut().find(|f| f.name == fname) {
            Some(f) => {
                f.columns.push(st(&r, "col"));
                f.ref_columns.push(st(&r, "ref_col"));
            }
            None => t.foreign_keys.push(ForeignKeyDef {
                name: fname,
                columns: vec![st(&r, "col")],
                ref_schema: s(&r, "ref_schema"),
                ref_table: st(&r, "ref_table"),
                ref_columns: vec![st(&r, "ref_col")],
                on_update: pg_action(&st(&r, "upd")),
                on_delete: pg_action(&st(&r, "del")),
                ..Default::default()
            }),
        }
    }

    let checks = q(session, &format!(
        "SELECT conname AS name, pg_get_constraintdef(oid, true) AS def FROM pg_constraint WHERE conrelid = {rel} AND contype = 'c' ORDER BY 1"
    )).await?;
    for r in checks.0 {
        t.checks.push(CheckDef { name: st(&r, "name"), expression: strip_check(&st(&r, "def")), ..Default::default() });
    }

    let trig = q(session, &format!(
        "SELECT tgname AS name, pg_get_triggerdef(oid, true) AS def FROM pg_trigger WHERE tgrelid = {rel} AND NOT tgisinternal ORDER BY 1"
    )).await?;
    for r in trig.0 {
        t.triggers.push(TriggerDef { name: st(&r, "name"), definition: st(&r, "def"), ..Default::default() });
    }

    let comment = q(session, &format!("SELECT obj_description({rel}, 'pg_class') AS c")).await?;
    t.comment = comment.0.first().and_then(|r| s(r, "c"));
    Ok(t)
}

/// `CHECK ((price > 0)) NOT VALID` → `(price > 0)`.
fn strip_check(def: &str) -> String {
    let d = def.trim();
    let d = d.strip_prefix("CHECK").unwrap_or(d).trim();
    let d = d.strip_suffix("NOT VALID").unwrap_or(d).trim();
    match d.strip_prefix('(').and_then(|x| x.strip_suffix(')')) {
        Some(inner) => inner.trim().to_string(),
        None => d.to_string(),
    }
}

/// `int(10) unsigned` → (`int`, 10, None, unsigned). `enum(…)`/`set(…)` keep
/// their value list in the type.
fn parse_mysql_type(column_type: &str) -> (String, Option<u32>, Option<u32>, bool) {
    let lower = column_type.to_lowercase();
    let unsigned = lower.contains(" unsigned");
    let base_end = lower.find(['(', ' ']).unwrap_or(lower.len());
    let base = lower[..base_end].to_string();
    if base == "enum" || base == "set" {
        let full = column_type.split(" unsigned").next().unwrap_or(column_type).trim().to_string();
        return (full, None, None, false);
    }
    let (mut len, mut scale) = (None, None);
    if let (Some(a), Some(b)) = (lower.find('('), lower.find(')')) {
        let mut parts = lower[a + 1..b].split(',');
        len = parts.next().and_then(|p| p.trim().parse().ok());
        scale = parts.next().and_then(|p| p.trim().parse().ok());
    }
    (base, len, scale, unsigned)
}

/// MySQL 8 reports a string default bare (`abc`); MariaDB quotes it (`'abc'`)
/// and spells a missing default `NULL`. Both become SQL expressions here.
fn mysql_default(engine: Engine, raw: Option<String>, extra: &str, data_type: &str) -> Option<String> {
    let v = raw?;
    if engine == Engine::Mariadb {
        return if v == "NULL" { None } else { Some(v) };
    }
    let expression = extra.to_uppercase().contains("DEFAULT_GENERATED");
    let numeric = matches!(
        data_type,
        "tinyint" | "smallint" | "mediumint" | "int" | "integer" | "bigint" | "decimal" | "numeric" | "float" | "double" | "bit"
    );
    if expression {
        // MySQL 8 omits the parentheses an expression default needs.
        let upper = v.to_uppercase();
        return Some(if upper.starts_with("CURRENT_TIMESTAMP") || upper.starts_with("NOW(") { v } else { format!("({v})") });
    }
    Some(if numeric { v } else { literal(engine, Some(&v)) })
}

async fn mysql_table(engine: Engine, session: &mut Session, schema: &str, name: &str) -> Result<TableDesign, String> {
    let (sch, tbl) = (lit(engine, schema), lit(engine, name));
    let mut t = TableDesign::default();

    let cols = q(session, &format!(
        "SELECT COLUMN_NAME AS name, DATA_TYPE AS dt, COLUMN_TYPE AS ct, IS_NULLABLE AS nullable, COLUMN_DEFAULT AS dflt, \
           EXTRA AS extra, COLUMN_COMMENT AS comment, COLLATION_NAME AS coll, GENERATION_EXPRESSION AS gen \
         FROM information_schema.COLUMNS WHERE TABLE_SCHEMA = {sch} AND TABLE_NAME = {tbl} ORDER BY ORDINAL_POSITION"
    )).await?;
    let table_coll = q(session, &format!(
        "SELECT ENGINE AS eng, TABLE_COLLATION AS coll, AUTO_INCREMENT AS ai, TABLE_COMMENT AS comment \
         FROM information_schema.TABLES WHERE TABLE_SCHEMA = {sch} AND TABLE_NAME = {tbl}"
    )).await?;
    let default_coll = table_coll.0.first().and_then(|r| s(r, "coll"));
    for r in cols.0 {
        let (data_type, length, scale, unsigned) = parse_mysql_type(&st(&r, "ct"));
        let extra = st(&r, "extra");
        let ex = extra.to_lowercase();
        let generated = (ex.contains("generated") && !ex.contains("default_generated")).then(|| st(&r, "gen")).filter(|g| !g.is_empty());
        let on_update = ex.find("on update ").map(|i| extra[i + "on update ".len()..].trim().to_string()).filter(|u| !u.is_empty());
        let coll = s(&r, "coll").filter(|c| Some(c) != default_coll.as_ref());
        t.columns.push(ColumnDef {
            name: st(&r, "name"),
            default: if generated.is_some() { None } else { mysql_default(engine, s(&r, "dflt"), &extra, &st(&r, "dt")) },
            data_type,
            length,
            scale,
            unsigned,
            not_null: st(&r, "nullable") == "NO",
            auto_increment: ex.contains("auto_increment"),
            comment: s(&r, "comment").filter(|c| !c.is_empty()),
            collation: coll,
            on_update,
            generated_stored: ex.contains("stored"),
            generated,
            ..Default::default()
        });
    }
    if let Some(r) = table_coll.0.first() {
        let coll = s(r, "coll");
        t.options = TableOptions {
            engine: s(r, "eng"),
            charset: coll.as_deref().and_then(|c| c.split('_').next()).map(str::to_string),
            collation: coll,
            auto_increment: num(r, "ai"),
        };
        t.comment = s(r, "comment").filter(|c| !c.is_empty());
    }

    let stats = q(session, &format!(
        "SELECT INDEX_NAME AS name, NON_UNIQUE AS nonuniq, COLUMN_NAME AS col, COLLATION AS coll, SUB_PART AS part, \
           INDEX_TYPE AS method, INDEX_COMMENT AS comment \
         FROM information_schema.STATISTICS WHERE TABLE_SCHEMA = {sch} AND TABLE_NAME = {tbl} ORDER BY INDEX_NAME, SEQ_IN_INDEX"
    )).await?;
    for r in stats.0 {
        let iname = st(&r, "name");
        if iname == "PRIMARY" {
            t.primary_key.push(st(&r, "col"));
            continue;
        }
        let col = IndexColumn { name: st(&r, "col"), descending: st(&r, "coll") == "D", length: num(&r, "part") };
        match t.indexes.iter_mut().find(|i| i.name == iname) {
            Some(i) => i.columns.push(col),
            None => t.indexes.push(IndexDef {
                name: iname,
                unique: st(&r, "nonuniq") == "0",
                method: s(&r, "method").map(|m| m.to_lowercase()),
                comment: s(&r, "comment").filter(|c| !c.is_empty()),
                columns: vec![col],
                ..Default::default()
            }),
        }
    }

    let fks = q(session, &format!(
        "SELECT k.CONSTRAINT_NAME AS name, k.COLUMN_NAME AS col, k.REFERENCED_TABLE_SCHEMA AS ref_schema, \
           k.REFERENCED_TABLE_NAME AS ref_table, k.REFERENCED_COLUMN_NAME AS ref_col, r.UPDATE_RULE AS upd, r.DELETE_RULE AS del \
         FROM information_schema.KEY_COLUMN_USAGE k JOIN information_schema.REFERENTIAL_CONSTRAINTS r \
           ON r.CONSTRAINT_SCHEMA = k.CONSTRAINT_SCHEMA AND r.CONSTRAINT_NAME = k.CONSTRAINT_NAME AND r.TABLE_NAME = k.TABLE_NAME \
         WHERE k.TABLE_SCHEMA = {sch} AND k.TABLE_NAME = {tbl} AND k.REFERENCED_TABLE_NAME IS NOT NULL \
         ORDER BY k.CONSTRAINT_NAME, k.ORDINAL_POSITION"
    )).await?;
    for r in fks.0 {
        let fname = st(&r, "name");
        match t.foreign_keys.iter_mut().find(|f| f.name == fname) {
            Some(f) => {
                f.columns.push(st(&r, "col"));
                f.ref_columns.push(st(&r, "ref_col"));
            }
            None => {
                let ref_schema = s(&r, "ref_schema").filter(|rs| rs != schema);
                t.foreign_keys.push(ForeignKeyDef {
                    name: fname,
                    columns: vec![st(&r, "col")],
                    ref_schema,
                    ref_table: st(&r, "ref_table"),
                    ref_columns: vec![st(&r, "ref_col")],
                    on_update: s(&r, "upd"),
                    on_delete: s(&r, "del"),
                    ..Default::default()
                })
            }
        }
    }

    // CHECK constraints: MySQL 8.0.16+ and MariaDB 10.2+. Older servers lack
    // the view entirely, which just means there are none to show.
    let checks_sql = if engine == Engine::Mariadb {
        format!("SELECT CONSTRAINT_NAME AS name, CHECK_CLAUSE AS def FROM information_schema.CHECK_CONSTRAINTS WHERE CONSTRAINT_SCHEMA = {sch} AND TABLE_NAME = {tbl}")
    } else {
        format!(
            "SELECT c.CONSTRAINT_NAME AS name, c.CHECK_CLAUSE AS def FROM information_schema.CHECK_CONSTRAINTS c \
             JOIN information_schema.TABLE_CONSTRAINTS t ON t.CONSTRAINT_SCHEMA = c.CONSTRAINT_SCHEMA AND t.CONSTRAINT_NAME = c.CONSTRAINT_NAME \
             WHERE t.TABLE_SCHEMA = {sch} AND t.TABLE_NAME = {tbl} AND t.CONSTRAINT_TYPE = 'CHECK'"
        )
    };
    if let Ok(checks) = q(session, &checks_sql).await {
        for r in checks.0 {
            t.checks.push(CheckDef { name: st(&r, "name"), expression: st(&r, "def"), ..Default::default() });
        }
    }

    let trig = q(session, &format!(
        "SELECT TRIGGER_NAME AS name, ACTION_TIMING AS timing, EVENT_MANIPULATION AS event, ACTION_STATEMENT AS body \
         FROM information_schema.TRIGGERS WHERE EVENT_OBJECT_SCHEMA = {sch} AND EVENT_OBJECT_TABLE = {tbl} ORDER BY ACTION_ORDER"
    )).await?;
    for r in trig.0 {
        let n = st(&r, "name");
        t.triggers.push(TriggerDef {
            definition: format!(
                "CREATE TRIGGER {} {} {} ON {} FOR EACH ROW {}",
                ident(engine, &n),
                st(&r, "timing"),
                st(&r, "event"),
                qualified(engine, Some(schema), name),
                st(&r, "body")
            ),
            name: n,
            ..Default::default()
        });
    }
    Ok(t)
}

async fn mssql_table(session: &mut Session, schema: &str, name: &str) -> Result<TableDesign, String> {
    let e = Engine::Mssql;
    let obj = format!("OBJECT_ID({})", lit(e, &qualified(e, Some(schema), name)));
    let mut t = TableDesign::default();

    let cols = q(session, &format!(
        "SELECT c.name, TYPE_NAME(c.user_type_id) AS type, c.max_length AS len, c.precision AS prec, c.scale, \
           c.is_nullable AS nullable, c.is_identity AS ident, c.collation_name AS coll, \
           dc.name AS df_name, dc.definition AS df, cc.definition AS gen, cc.is_persisted AS persisted, \
           CAST(ep.value AS nvarchar(4000)) AS comment, \
           CAST(DATABASEPROPERTYEX(DB_NAME(), 'Collation') AS nvarchar(128)) AS db_coll \
         FROM sys.columns c \
         LEFT JOIN sys.default_constraints dc ON dc.parent_object_id = c.object_id AND dc.parent_column_id = c.column_id \
         LEFT JOIN sys.computed_columns cc ON cc.object_id = c.object_id AND cc.column_id = c.column_id \
         LEFT JOIN sys.extended_properties ep ON ep.major_id = c.object_id AND ep.minor_id = c.column_id AND ep.name = 'MS_Description' \
         WHERE c.object_id = {obj} ORDER BY c.column_id"
    )).await?;
    for r in cols.0 {
        let ty = st(&r, "type").to_lowercase();
        let len: Option<i64> = num(&r, "len");
        let (data_type, length, scale) = match ty.as_str() {
            "varchar" | "char" | "varbinary" | "binary" => match len {
                Some(-1) => (format!("{ty}(max)"), None, None),
                l => (ty.clone(), l.map(|v| v as u32), None),
            },
            "nvarchar" | "nchar" => match len {
                Some(-1) => (format!("{ty}(max)"), None, None),
                l => (ty.clone(), l.map(|v| (v / 2) as u32), None),
            },
            "decimal" | "numeric" => (ty.clone(), num(&r, "prec"), num(&r, "scale")),
            _ => (ty.clone(), None, None),
        };
        let generated = s(&r, "gen");
        t.columns.push(ColumnDef {
            name: st(&r, "name"),
            data_type,
            length,
            scale,
            not_null: !truthy(&r, "nullable"),
            default: s(&r, "df"),
            default_constraint: s(&r, "df_name"),
            auto_increment: truthy(&r, "ident"),
            collation: s(&r, "coll").filter(|c| Some(c) != s(&r, "db_coll").as_ref()),
            comment: s(&r, "comment"),
            generated_stored: truthy(&r, "persisted"),
            generated,
            ..Default::default()
        });
    }

    let idx = q(session, &format!(
        "SELECT i.name, i.is_primary_key AS pk, i.is_unique AS uniq, i.is_unique_constraint AS ucon, i.type_desc AS method, \
           i.filter_definition AS pred, c.name AS col, ic.is_descending_key AS descending \
         FROM sys.indexes i JOIN sys.index_columns ic ON ic.object_id = i.object_id AND ic.index_id = i.index_id \
         JOIN sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id \
         WHERE i.object_id = {obj} AND i.type > 0 AND ic.is_included_column = 0 ORDER BY i.name, ic.key_ordinal"
    )).await?;
    for r in idx.0 {
        let iname = st(&r, "name");
        if truthy(&r, "pk") {
            t.primary_key_name = Some(iname);
            t.primary_key.push(st(&r, "col"));
            continue;
        }
        let col = IndexColumn { name: st(&r, "col"), descending: truthy(&r, "descending"), length: None };
        match t.indexes.iter_mut().find(|i| i.name == iname) {
            Some(i) => i.columns.push(col),
            None => t.indexes.push(IndexDef {
                name: iname,
                unique: truthy(&r, "uniq"),
                constraint: truthy(&r, "ucon"),
                method: s(&r, "method").map(|m| m.to_lowercase()),
                where_clause: s(&r, "pred"),
                columns: vec![col],
                ..Default::default()
            }),
        }
    }

    let fks = q(session, &format!(
        "SELECT fk.name, pc.name AS col, rc.name AS ref_col, OBJECT_SCHEMA_NAME(fk.referenced_object_id) AS ref_schema, \
           OBJECT_NAME(fk.referenced_object_id) AS ref_table, fk.update_referential_action_desc AS upd, fk.delete_referential_action_desc AS del \
         FROM sys.foreign_keys fk JOIN sys.foreign_key_columns fkc ON fkc.constraint_object_id = fk.object_id \
         JOIN sys.columns pc ON pc.object_id = fkc.parent_object_id AND pc.column_id = fkc.parent_column_id \
         JOIN sys.columns rc ON rc.object_id = fkc.referenced_object_id AND rc.column_id = fkc.referenced_column_id \
         WHERE fk.parent_object_id = {obj} ORDER BY fk.name, fkc.constraint_column_id"
    )).await?;
    for r in fks.0 {
        let fname = st(&r, "name");
        let action = |v: Option<String>| v.map(|a| a.replace('_', " ")).filter(|a| a != "NO ACTION");
        match t.foreign_keys.iter_mut().find(|f| f.name == fname) {
            Some(f) => {
                f.columns.push(st(&r, "col"));
                f.ref_columns.push(st(&r, "ref_col"));
            }
            None => t.foreign_keys.push(ForeignKeyDef {
                name: fname,
                columns: vec![st(&r, "col")],
                ref_schema: s(&r, "ref_schema"),
                ref_table: st(&r, "ref_table"),
                ref_columns: vec![st(&r, "ref_col")],
                on_update: action(s(&r, "upd")),
                on_delete: action(s(&r, "del")),
                ..Default::default()
            }),
        }
    }

    let checks = q(session, &format!("SELECT name, definition AS def FROM sys.check_constraints WHERE parent_object_id = {obj} ORDER BY name")).await?;
    for r in checks.0 {
        let def = st(&r, "def");
        let expr = def.strip_prefix('(').and_then(|d| d.strip_suffix(')')).unwrap_or(&def).to_string();
        t.checks.push(CheckDef { name: st(&r, "name"), expression: expr, ..Default::default() });
    }

    let trig = q(session, &format!("SELECT name, OBJECT_DEFINITION(object_id) AS def FROM sys.triggers WHERE parent_id = {obj} ORDER BY name")).await?;
    for r in trig.0 {
        t.triggers.push(TriggerDef { name: st(&r, "name"), definition: st(&r, "def"), ..Default::default() });
    }

    let comment = q(session, &format!(
        "SELECT CAST(value AS nvarchar(4000)) AS c FROM sys.extended_properties WHERE major_id = {obj} AND minor_id = 0 AND name = 'MS_Description'"
    )).await?;
    t.comment = comment.0.first().and_then(|r| s(r, "c"));
    Ok(t)
}

/// `VARCHAR(100)` → (`varchar`, 100, None).
fn parse_sqlite_type(t: &str) -> (String, Option<u32>, Option<u32>) {
    let lower = t.trim().to_lowercase();
    match (lower.find('('), lower.rfind(')')) {
        (Some(a), Some(b)) if b > a => {
            let mut parts = lower[a + 1..b].split(',');
            let l = parts.next().and_then(|p| p.trim().parse().ok());
            let s = parts.next().and_then(|p| p.trim().parse().ok());
            (lower[..a].trim().to_string(), l, s)
        }
        _ => (lower, None, None),
    }
}

/// The text inside the parenthesis that opens at `open` (a byte index).
fn paren_body(sql: &str, open: usize) -> Option<&str> {
    let mut depth = 0;
    let mut in_str: Option<char> = None;
    for (i, ch) in sql[open..].char_indices() {
        match in_str {
            Some(q) if ch == q => in_str = None,
            Some(_) => {}
            None => match ch {
                '\'' | '"' | '`' => in_str = Some(ch),
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(&sql[open + 1..open + i]);
                    }
                }
                _ => {}
            },
        }
    }
    None
}

/// CHECK constraints from a SQLite CREATE TABLE statement, which is the only
/// place SQLite keeps them. A rebuild must carry them over, so they have to
/// be found even when they were written inline on a column.
fn sqlite_checks(create: &str) -> Vec<CheckDef> {
    let upper = create.to_uppercase();
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(pos) = upper[from..].find("CHECK") {
        let at = from + pos;
        from = at + 5;
        let before_ok = at == 0 || !upper.as_bytes()[at - 1].is_ascii_alphanumeric();
        let after = upper[at + 5..].trim_start();
        if !before_ok || !after.starts_with('(') {
            continue;
        }
        let open = at + 5 + (upper[at + 5..].len() - after.len());
        let Some(body) = paren_body(create, open) else { continue };
        // A preceding `CONSTRAINT name` names it.
        let head = create[..at].trim_end();
        let name = head
            .rsplit_once(char::is_whitespace)
            .filter(|(rest, _)| rest.trim_end().to_uppercase().ends_with("CONSTRAINT"))
            .map(|(_, n)| n.trim_matches(['"', '`', '[', ']']).to_string())
            .unwrap_or_else(|| format!("check_{}", out.len() + 1));
        out.push(CheckDef { name, expression: body.trim().to_string(), ..Default::default() });
    }
    out
}

/// A generated column's expression, from the CREATE TABLE text.
fn sqlite_generated(create: &str, column: &str) -> Option<String> {
    let upper = create.to_uppercase();
    let mut from = 0;
    while let Some(pos) = upper[from..].find("GENERATED ALWAYS AS") {
        let at = from + pos;
        from = at + 1;
        let open = at + upper[at..].find('(')?;
        // The column this belongs to is the nearest definition start before it.
        let start = create[..at].rfind([',', '(']).map(|i| i + 1).unwrap_or(0);
        let head = create[start..at].trim();
        let name = head.split_whitespace().next().unwrap_or("").trim_matches(['"', '`', '[', ']']);
        if name.eq_ignore_ascii_case(column) {
            return paren_body(create, open).map(|b| b.trim().to_string());
        }
    }
    None
}

async fn sqlite_table(session: &mut Session, name: &str) -> Result<TableDesign, String> {
    let e = Engine::Sqlite;
    let n = lit(e, name);
    let mut t = TableDesign::default();

    let create = q(session, &format!("SELECT sql FROM sqlite_master WHERE type = 'table' AND name = {n}")).await?;
    let create_sql = create.0.first().map(|r| st(r, "sql")).unwrap_or_default();
    let autoinc = create_sql.to_uppercase().contains("AUTOINCREMENT");

    let cols = q(session, &format!("SELECT * FROM pragma_table_xinfo({n})")).await?;
    let mut pk: Vec<(u32, String)> = Vec::new();
    for r in cols.0 {
        let hidden: i64 = num(&r, "hidden").unwrap_or(0);
        if hidden == 1 {
            continue; // virtual-table hidden column
        }
        let cname = st(&r, "name");
        let (data_type, length, scale) = parse_sqlite_type(&st(&r, "type"));
        let order: u32 = num(&r, "pk").unwrap_or(0);
        if order > 0 {
            pk.push((order, cname.clone()));
        }
        let generated = if hidden >= 2 { sqlite_generated(&create_sql, &cname) } else { None };
        t.columns.push(ColumnDef {
            data_type,
            length,
            scale,
            not_null: st(&r, "notnull") == "1",
            default: s(&r, "dflt_value"),
            auto_increment: order > 0 && autoinc,
            generated_stored: hidden == 3,
            generated,
            name: cname,
            ..Default::default()
        });
    }
    pk.sort();
    t.primary_key = pk.into_iter().map(|(_, c)| c).collect();

    let idx = q(session, &format!("SELECT name, \"unique\" AS uniq, origin, partial FROM pragma_index_list({n})")).await?;
    let index_sql = q(session, &format!("SELECT name, sql FROM sqlite_master WHERE type = 'index' AND tbl_name = {n}")).await?;
    for r in idx.0 {
        if st(&r, "origin") == "pk" {
            continue;
        }
        let iname = st(&r, "name");
        let cols = q(session, &format!("SELECT name, \"desc\" AS descending FROM pragma_index_xinfo({}) WHERE key = 1 ORDER BY seqno", lit(e, &iname))).await?;
        let where_clause = index_sql
            .0
            .iter()
            .find(|x| st(x, "name") == iname)
            .and_then(|x| s(x, "sql"))
            .and_then(|sql| sql.to_uppercase().rfind(" WHERE ").map(|i| sql[i + 7..].trim().to_string()));
        t.indexes.push(IndexDef {
            // Indexes SQLite made for a UNIQUE constraint cannot be dropped on
            // their own; the rebuild recreates them as ordinary unique indexes.
            constraint: st(&r, "origin") == "u",
            unique: st(&r, "uniq") == "1",
            where_clause,
            columns: cols.0.iter().map(|c| IndexColumn { name: st(c, "name"), descending: st(c, "descending") == "1", length: None }).collect(),
            name: iname,
            ..Default::default()
        });
    }

    let fks = q(session, &format!("SELECT * FROM pragma_foreign_key_list({n}) ORDER BY id, seq")).await?;
    for r in fks.0 {
        let id = st(&r, "id");
        let fname = format!("fk_{name}_{id}");
        match t.foreign_keys.iter_mut().find(|f| f.name == fname) {
            Some(f) => {
                f.columns.push(st(&r, "from"));
                f.ref_columns.push(st(&r, "to"));
            }
            None => t.foreign_keys.push(ForeignKeyDef {
                name: fname,
                columns: vec![st(&r, "from")],
                ref_table: st(&r, "table"),
                ref_columns: vec![st(&r, "to")],
                on_update: s(&r, "on_update").filter(|a| a != "NO ACTION"),
                on_delete: s(&r, "on_delete").filter(|a| a != "NO ACTION"),
                ..Default::default()
            }),
        }
    }

    t.checks = sqlite_checks(&create_sql);

    let trig = q(session, &format!("SELECT name, sql FROM sqlite_master WHERE type = 'trigger' AND tbl_name = {n} ORDER BY name")).await?;
    for r in trig.0 {
        t.triggers.push(TriggerDef { name: st(&r, "name"), definition: st(&r, "sql"), ..Default::default() });
    }
    Ok(t)
}

// ---------------------------------------------------------------------------
// Definitions, server info, sessions

/// The CREATE statement for a view or routine, for opening it in an editor.
pub async fn definition(engine: Engine, session: &mut Session, schema: Option<&str>, name: &str, kind: &str) -> Result<String, String> {
    let rows = match engine {
        Engine::Postgres => {
            let sch = schema.unwrap_or("public");
            match kind {
                "view" => q(session, &format!(
                    "SELECT 'CREATE OR REPLACE VIEW ' || {} || ' AS' || chr(10) || pg_get_viewdef({}::regclass, true) AS def",
                    lit(engine, &qualified(engine, Some(sch), name)),
                    lit(engine, &qualified(engine, Some(sch), name))
                )).await?,
                "materializedView" => q(session, &format!(
                    "SELECT 'CREATE MATERIALIZED VIEW ' || {} || ' AS' || chr(10) || pg_get_viewdef({}::regclass, true) AS def",
                    lit(engine, &qualified(engine, Some(sch), name)),
                    lit(engine, &qualified(engine, Some(sch), name))
                )).await?,
                // Routines are listed with their argument list, which is what
                // regprocedure needs to pick the right overload.
                _ => q(session, &format!(
                    "SELECT pg_get_functiondef({}::regprocedure) AS def",
                    lit(engine, &format!("{}.{}", ident(engine, sch), name))
                )).await?,
            }
        }
        Engine::Mysql | Engine::Mariadb => {
            let what = match kind {
                "view" => "VIEW",
                "procedure" => "PROCEDURE",
                _ => "FUNCTION",
            };
            let rs = session.run(&format!("SHOW CREATE {what} {}", qualified(engine, schema, name)), 1).await?;
            // The statement is in the column whose name starts "Create ".
            let idx = rs.columns.iter().position(|c| c.name.to_lowercase().starts_with("create ")).unwrap_or(1);
            return Ok(rs.rows.first().and_then(|r| r.get(idx).cloned().flatten()).unwrap_or_default());
        }
        Engine::Mssql => q(session, &format!(
            "SELECT OBJECT_DEFINITION(OBJECT_ID({})) AS def",
            lit(engine, &qualified(engine, schema.or(Some("dbo")), name))
        )).await?,
        Engine::Sqlite => q(session, &format!("SELECT sql AS def FROM sqlite_master WHERE name = {}", lit(engine, name))).await?,
        Engine::Redis => return Err("Redis has no definitions".into()),
    };
    Ok(rows.0.first().map(|r| st(r, "def")).unwrap_or_default())
}

pub async fn server_info(engine: Engine, session: &mut Session) -> Result<ServerInfo, String> {
    let sql = match engine {
        Engine::Postgres => "SELECT version() AS v, current_database() AS db, current_user AS u",
        Engine::Mysql | Engine::Mariadb => "SELECT VERSION() AS v, DATABASE() AS db, CURRENT_USER() AS u",
        Engine::Mssql => "SELECT @@VERSION AS v, DB_NAME() AS db, SUSER_SNAME() AS u",
        Engine::Sqlite => "SELECT 'SQLite ' || sqlite_version() AS v, 'main' AS db, NULL AS u",
        Engine::Redis => return Err("Not a SQL connection".into()),
    };
    let r = q(session, sql).await?;
    let row = r.0.first().ok_or("The server did not answer")?;
    Ok(ServerInfo {
        version: st(row, "v").lines().next().unwrap_or("").to_string(),
        current_database: s(row, "db"),
        current_user: s(row, "u"),
    })
}

pub async fn sessions(engine: Engine, session: &mut Session, own: &[i64]) -> Result<Vec<ServerSession>, String> {
    let sql = match engine {
        Engine::Postgres => {
            "SELECT pid AS id, usename AS usr, datname AS db, client_addr::text AS client, state, \
               EXTRACT(EPOCH FROM (now() - COALESCE(query_start, backend_start)))::bigint AS secs, query \
             FROM pg_stat_activity WHERE backend_type = 'client backend' ORDER BY secs DESC NULLS LAST"
        }
        Engine::Mysql | Engine::Mariadb => {
            "SELECT ID AS id, USER AS usr, DB AS db, HOST AS client, COALESCE(NULLIF(STATE, ''), COMMAND) AS state, TIME AS secs, INFO AS query \
             FROM information_schema.PROCESSLIST ORDER BY TIME DESC"
        }
        Engine::Mssql => {
            "SELECT s.session_id AS id, s.login_name AS usr, DB_NAME(s.database_id) AS db, s.host_name AS client, \
               COALESCE(r.status, s.status) AS state, \
               DATEDIFF(SECOND, COALESCE(r.start_time, s.last_request_start_time), SYSDATETIME()) AS secs, t.text AS query \
             FROM sys.dm_exec_sessions s LEFT JOIN sys.dm_exec_requests r ON r.session_id = s.session_id \
             OUTER APPLY sys.dm_exec_sql_text(r.sql_handle) t WHERE s.is_user_process = 1 ORDER BY secs DESC"
        }
        _ => return Ok(vec![]),
    };
    let rows = q(session, sql).await?;
    Ok(rows
        .0
        .iter()
        .map(|r| {
            let id = st(r, "id");
            ServerSession {
                is_self: id.parse::<i64>().map(|n| own.contains(&n)).unwrap_or(false),
                id,
                user: s(r, "usr"),
                database: s(r, "db"),
                client: s(r, "client"),
                state: s(r, "state"),
                seconds: num(r, "secs"),
                query: s(r, "query"),
            }
        })
        .collect())
}

/// End a server session, or only its running statement.
pub fn kill_sql(engine: Engine, id: i64, only_query: bool) -> Result<String, String> {
    Ok(match (engine, only_query) {
        (Engine::Postgres, true) => format!("SELECT pg_cancel_backend({id})"),
        (Engine::Postgres, false) => format!("SELECT pg_terminate_backend({id})"),
        (Engine::Mysql | Engine::Mariadb, true) => format!("KILL QUERY {id}"),
        (Engine::Mysql | Engine::Mariadb, false) => format!("KILL {id}"),
        (Engine::Mssql, _) => format!("KILL {id}"),
        _ => return Err("This database has no server sessions".into()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mysql_column_types_split_into_parts() {
        assert_eq!(parse_mysql_type("int(10) unsigned"), ("int".into(), Some(10), None, true));
        assert_eq!(parse_mysql_type("decimal(12,2)"), ("decimal".into(), Some(12), Some(2), false));
        assert_eq!(parse_mysql_type("enum('a','b')"), ("enum('a','b')".into(), None, None, false));
        assert_eq!(parse_mysql_type("datetime"), ("datetime".into(), None, None, false));
    }

    #[test]
    fn mysql_defaults_become_expressions() {
        assert_eq!(mysql_default(Engine::Mysql, Some("abc".into()), "", "varchar"), Some("'abc'".into()));
        assert_eq!(mysql_default(Engine::Mysql, Some("0".into()), "", "int"), Some("0".into()));
        assert_eq!(
            mysql_default(Engine::Mysql, Some("CURRENT_TIMESTAMP".into()), "DEFAULT_GENERATED", "timestamp"),
            Some("CURRENT_TIMESTAMP".into())
        );
        assert_eq!(mysql_default(Engine::Mysql, Some("uuid()".into()), "DEFAULT_GENERATED", "char"), Some("(uuid())".into()));
        assert_eq!(mysql_default(Engine::Mariadb, Some("NULL".into()), "", "int"), None);
        assert_eq!(mysql_default(Engine::Mariadb, Some("'x'".into()), "", "varchar"), Some("'x'".into()));
    }

    #[test]
    fn postgres_check_definitions_lose_their_wrapper() {
        assert_eq!(strip_check("CHECK ((price > 0))"), "(price > 0)");
        assert_eq!(strip_check("CHECK (a <> b) NOT VALID"), "a <> b");
    }

    #[test]
    fn postgres_internal_type_names_read_naturally() {
        assert_eq!(pg_type_name("int4"), "integer");
        assert_eq!(pg_type_name("_text"), "text[]");
        assert_eq!(pg_type_name("timestamptz"), "timestamptz");
    }

    #[test]
    fn sqlite_checks_are_found_inline_and_named() {
        let sql = "CREATE TABLE t (a int CHECK (a > 0), b text, CONSTRAINT b_len CHECK (length(b) < 10), checked int)";
        let c = sqlite_checks(sql);
        assert_eq!(c.len(), 2, "{c:?}");
        assert_eq!(c[0].expression, "a > 0");
        assert_eq!(c[1].name, "b_len");
        assert_eq!(c[1].expression, "length(b) < 10");
    }

    #[test]
    fn sqlite_generated_expression_is_found_for_its_column() {
        let sql = "CREATE TABLE t (a int, b int GENERATED ALWAYS AS (a * 2) STORED, c text)";
        assert_eq!(sqlite_generated(sql, "b").as_deref(), Some("a * 2"));
        assert_eq!(sqlite_generated(sql, "a"), None);
    }
}
