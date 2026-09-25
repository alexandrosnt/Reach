//! The table designer's model, and the DDL that turns one version of a table
//! into another.
//!
//! The frontend edits a copy of what [`super::introspect`] loaded. Every part
//! carries the name it had when loaded (`original`), so a rename is told apart
//! from a drop plus an add — the difference between keeping a column's data
//! and losing it. Nothing here runs anything; the designer shows the output as
//! its SQL preview and the same text is what Apply executes.

use serde::{Deserialize, Serialize};

use super::dialect::{ident, literal, qualified};
use super::types::Engine;

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct TableDesign {
    pub schema: Option<String>,
    /// The name when loaded; `None` for a table being created.
    pub original_name: Option<String>,
    pub name: String,
    pub columns: Vec<ColumnDef>,
    pub primary_key: Vec<String>,
    /// PostgreSQL and SQL Server name the constraint; needed to drop it.
    pub primary_key_name: Option<String>,
    pub indexes: Vec<IndexDef>,
    pub foreign_keys: Vec<ForeignKeyDef>,
    pub checks: Vec<CheckDef>,
    pub triggers: Vec<TriggerDef>,
    pub comment: Option<String>,
    pub options: TableOptions,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct ColumnDef {
    pub original: Option<String>,
    pub name: String,
    /// Base type without length, e.g. `varchar`, `numeric`, `int`.
    pub data_type: String,
    pub length: Option<u32>,
    pub scale: Option<u32>,
    pub not_null: bool,
    /// A SQL expression, written as the engine expects it: `'text'`, `0`, `now()`.
    pub default: Option<String>,
    pub auto_increment: bool,
    pub comment: Option<String>,
    pub collation: Option<String>,
    /// MySQL.
    pub unsigned: bool,
    /// MySQL: `ON UPDATE CURRENT_TIMESTAMP`.
    pub on_update: Option<String>,
    /// Generated column expression.
    pub generated: Option<String>,
    pub generated_stored: bool,
    /// SQL Server keeps a default as a named constraint.
    pub default_constraint: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct IndexColumn {
    pub name: String,
    pub descending: bool,
    /// MySQL prefix length.
    pub length: Option<u32>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct IndexDef {
    pub original: Option<String>,
    pub name: String,
    pub columns: Vec<IndexColumn>,
    pub unique: bool,
    /// A UNIQUE constraint rather than a unique index (PostgreSQL, SQL Server).
    pub constraint: bool,
    /// `btree`, `hash`, `gin`, `gist`, `fulltext`, `spatial`…
    pub method: Option<String>,
    /// PostgreSQL / SQLite partial index.
    pub where_clause: Option<String>,
    pub comment: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct ForeignKeyDef {
    pub original: Option<String>,
    pub name: String,
    pub columns: Vec<String>,
    pub ref_schema: Option<String>,
    pub ref_table: String,
    pub ref_columns: Vec<String>,
    pub on_delete: Option<String>,
    pub on_update: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct CheckDef {
    pub original: Option<String>,
    pub name: String,
    pub expression: String,
}

/// Triggers differ too much between engines to model field by field; the
/// designer edits the full `CREATE TRIGGER` text instead.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct TriggerDef {
    pub original: Option<String>,
    pub name: String,
    pub definition: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct TableOptions {
    /// MySQL storage engine.
    pub engine: Option<String>,
    pub charset: Option<String>,
    pub collation: Option<String>,
    pub auto_increment: Option<u64>,
}

/// The statements Apply will run, plus whether they may share a transaction.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DdlPlan {
    pub statements: Vec<String>,
    /// PostgreSQL and SQL Server can roll DDL back; MySQL commits each
    /// statement on its own, and the SQLite rebuild manages its own
    /// transaction around a PRAGMA that cannot run inside one.
    pub transactional: bool,
}

// ---------------------------------------------------------------------------
// Column rendering

fn type_sql(c: &ColumnDef) -> String {
    let mut t = c.data_type.trim().to_string();
    match (c.length, c.scale) {
        (Some(l), Some(s)) => t.push_str(&format!("({l},{s})")),
        (Some(l), None) => t.push_str(&format!("({l})")),
        _ => {}
    }
    t
}

fn collate(engine: Engine, c: &ColumnDef) -> String {
    match c.collation.as_deref().filter(|s| !s.is_empty()) {
        Some(col) if engine == Engine::Postgres => format!(" COLLATE {}", ident(engine, col)),
        Some(col) => format!(" COLLATE {col}"),
        None => String::new(),
    }
}

fn default_name(table: &str, column: &str) -> String {
    format!("DF_{table}_{column}")
}

/// A full column definition, as in CREATE TABLE or ADD COLUMN.
/// `inline_pk` is SQLite's `INTEGER PRIMARY KEY AUTOINCREMENT`.
fn column_sql(engine: Engine, table: &str, c: &ColumnDef, inline_pk: bool) -> String {
    let name = ident(engine, &c.name);
    let generated = c.generated.as_deref().filter(|g| !g.trim().is_empty());

    if engine == Engine::Mssql {
        if let Some(expr) = generated {
            let persisted = if c.generated_stored { " PERSISTED" } else { "" };
            return format!("{name} AS ({expr}){persisted}");
        }
    }

    // SQLite accepts AUTOINCREMENT only on a column declared exactly INTEGER.
    let ty = if inline_pk { "INTEGER".to_string() } else { type_sql(c) };
    let mut s = format!("{name} {ty}");
    if engine.is_mysql_family() && c.unsigned {
        s.push_str(" UNSIGNED");
    }
    s.push_str(&collate(engine, c));

    if let Some(expr) = generated {
        let kind = match engine {
            Engine::Postgres => " STORED",
            _ if c.generated_stored => " STORED",
            _ => " VIRTUAL",
        };
        s.push_str(&format!(" GENERATED ALWAYS AS ({expr}){kind}"));
    }

    if inline_pk {
        s.push_str(" PRIMARY KEY AUTOINCREMENT");
    } else if c.auto_increment {
        match engine {
            Engine::Postgres => s.push_str(" GENERATED BY DEFAULT AS IDENTITY"),
            Engine::Mssql => s.push_str(" IDENTITY(1,1)"),
            _ => {}
        }
    }

    s.push_str(if c.not_null { " NOT NULL" } else if generated.is_some() { "" } else { " NULL" });

    if generated.is_none() {
        if let Some(d) = c.default.as_deref().filter(|d| !d.trim().is_empty()) {
            if engine == Engine::Mssql {
                let cname = ident(engine, &default_name(table, &c.name));
                s.push_str(&format!(" CONSTRAINT {cname} DEFAULT {d}"));
            } else {
                s.push_str(&format!(" DEFAULT {d}"));
            }
        }
    }

    if engine.is_mysql_family() {
        if c.auto_increment {
            s.push_str(" AUTO_INCREMENT");
        }
        if let Some(u) = c.on_update.as_deref().filter(|u| !u.is_empty()) {
            s.push_str(&format!(" ON UPDATE {u}"));
        }
        if let Some(cm) = c.comment.as_deref().filter(|c| !c.is_empty()) {
            s.push_str(&format!(" COMMENT {}", literal(engine, Some(cm))));
        }
    }
    s
}

fn fk_sql(engine: Engine, fk: &ForeignKeyDef) -> String {
    let cols = fk.columns.iter().map(|c| ident(engine, c)).collect::<Vec<_>>().join(", ");
    let refs = fk.ref_columns.iter().map(|c| ident(engine, c)).collect::<Vec<_>>().join(", ");
    let target = qualified(engine, fk.ref_schema.as_deref(), &fk.ref_table);
    let mut s = format!("CONSTRAINT {} FOREIGN KEY ({cols}) REFERENCES {target} ({refs})", ident(engine, &fk.name));
    for (verb, action) in [("DELETE", &fk.on_delete), ("UPDATE", &fk.on_update)] {
        if let Some(a) = action.as_deref().filter(|a| !a.is_empty() && !a.eq_ignore_ascii_case("no action")) {
            s.push_str(&format!(" ON {verb} {}", a.to_uppercase()));
        }
    }
    s
}

fn check_sql(engine: Engine, ck: &CheckDef) -> String {
    format!("CONSTRAINT {} CHECK ({})", ident(engine, &ck.name), ck.expression)
}

fn pk_sql(engine: Engine, t: &TableDesign) -> String {
    let cols = t.primary_key.iter().map(|c| ident(engine, c)).collect::<Vec<_>>().join(", ");
    match (engine, t.primary_key_name.as_deref().filter(|n| !n.is_empty())) {
        (Engine::Postgres | Engine::Mssql, Some(n)) => format!("CONSTRAINT {} PRIMARY KEY ({cols})", ident(engine, n)),
        _ => format!("PRIMARY KEY ({cols})"),
    }
}

pub(crate) fn index_sql(engine: Engine, t: &TableDesign, ix: &IndexDef) -> String {
    let table = qualified(engine, t.schema.as_deref(), &t.name);
    let cols = ix
        .columns
        .iter()
        .map(|c| {
            let mut s = ident(engine, &c.name);
            if let (true, Some(l)) = (engine.is_mysql_family(), c.length) {
                s.push_str(&format!("({l})"));
            }
            if c.descending {
                s.push_str(" DESC");
            }
            s
        })
        .collect::<Vec<_>>()
        .join(", ");
    let name = ident(engine, &ix.name);
    let method = ix.method.as_deref().map(str::to_lowercase);

    if ix.constraint && matches!(engine, Engine::Postgres | Engine::Mssql) {
        return format!("ALTER TABLE {table} ADD CONSTRAINT {name} UNIQUE ({cols})");
    }

    let kind = match (engine.is_mysql_family(), method.as_deref()) {
        (true, Some("fulltext")) => "FULLTEXT ",
        (true, Some("spatial")) => "SPATIAL ",
        _ if ix.unique => "UNIQUE ",
        _ => "",
    };
    let clustered = match (engine, method.as_deref()) {
        (Engine::Mssql, Some("clustered")) => "CLUSTERED ",
        (Engine::Mssql, Some("nonclustered")) => "NONCLUSTERED ",
        _ => "",
    };
    let mut s = format!("CREATE {kind}{clustered}INDEX {name} ON {table}");
    match (engine, method.as_deref()) {
        (Engine::Postgres, Some(m)) if m != "btree" => s.push_str(&format!(" USING {m}")),
        _ => {}
    }
    s.push_str(&format!(" ({cols})"));
    if let (true, Some(m)) = (engine.is_mysql_family(), method.as_deref()) {
        if m == "hash" || m == "btree" {
            s.push_str(&format!(" USING {}", m.to_uppercase()));
        }
    }
    if let Some(w) = ix.where_clause.as_deref().filter(|w| !w.trim().is_empty()) {
        if matches!(engine, Engine::Postgres | Engine::Sqlite | Engine::Mssql) {
            s.push_str(&format!(" WHERE {w}"));
        }
    }
    if let (true, Some(cm)) = (engine.is_mysql_family(), ix.comment.as_deref().filter(|c| !c.is_empty())) {
        s.push_str(&format!(" COMMENT {}", literal(engine, Some(cm))));
    }
    s
}

fn drop_index_sql(engine: Engine, t: &TableDesign, ix: &IndexDef, name: &str) -> String {
    let table = qualified(engine, t.schema.as_deref(), &t.name);
    let n = ident(engine, name);
    match engine {
        _ if ix.constraint && matches!(engine, Engine::Postgres | Engine::Mssql) => {
            format!("ALTER TABLE {table} DROP CONSTRAINT {n}")
        }
        Engine::Postgres => format!("DROP INDEX {}", qualified(engine, t.schema.as_deref(), name)),
        Engine::Mysql | Engine::Mariadb | Engine::Mssql => format!("DROP INDEX {n} ON {table}"),
        Engine::Sqlite | Engine::Redis => format!("DROP INDEX {n}"),
    }
}

fn drop_trigger_sql(engine: Engine, t: &TableDesign, name: &str) -> String {
    match engine {
        Engine::Postgres => format!(
            "DROP TRIGGER {} ON {}",
            ident(engine, name),
            qualified(engine, t.schema.as_deref(), &t.name)
        ),
        Engine::Mssql | Engine::Mysql | Engine::Mariadb => {
            format!("DROP TRIGGER {}", qualified(engine, t.schema.as_deref(), name))
        }
        Engine::Sqlite | Engine::Redis => format!("DROP TRIGGER {}", ident(engine, name)),
    }
}

/// Comment statements for engines that keep comments outside the definition.
fn comment_sql(engine: Engine, t: &TableDesign, column: Option<&str>, old: Option<&str>, new: Option<&str>) -> Option<String> {
    let new = new.filter(|c| !c.is_empty());
    let old = old.filter(|c| !c.is_empty());
    if old == new {
        return None;
    }
    let table = qualified(engine, t.schema.as_deref(), &t.name);
    match engine {
        Engine::Postgres => Some(match column {
            Some(c) => format!("COMMENT ON COLUMN {table}.{} IS {}", ident(engine, c), literal(engine, new)),
            None => format!("COMMENT ON TABLE {table} IS {}", literal(engine, new)),
        }),
        Engine::Mssql => {
            let schema = t.schema.clone().unwrap_or_else(|| "dbo".into());
            let proc = match (old, new) {
                (None, Some(_)) => "sp_addextendedproperty",
                (Some(_), Some(_)) => "sp_updateextendedproperty",
                (Some(_), None) => "sp_dropextendedproperty",
                (None, None) => return None,
            };
            let value = match new {
                Some(v) => format!(", @value = {}", literal(engine, Some(v))),
                None => String::new(),
            };
            let col = column
                .map(|c| format!(", @level2type = N'COLUMN', @level2name = {}", literal(engine, Some(c))))
                .unwrap_or_default();
            Some(format!(
                "EXEC sys.{proc} @name = N'MS_Description'{value}, @level0type = N'SCHEMA', @level0name = {}, @level1type = N'TABLE', @level1name = {}{col}",
                literal(engine, Some(&schema)),
                literal(engine, Some(&t.name)),
            ))
        }
        // MySQL writes comments inline; SQLite has none.
        _ => None,
    }
}

fn mysql_table_options(engine: Engine, t: &TableDesign) -> String {
    let o = &t.options;
    let mut parts = Vec::new();
    if let Some(e) = o.engine.as_deref().filter(|e| !e.is_empty()) {
        parts.push(format!("ENGINE={e}"));
    }
    if let Some(c) = o.charset.as_deref().filter(|c| !c.is_empty()) {
        parts.push(format!("DEFAULT CHARSET={c}"));
    }
    if let Some(c) = o.collation.as_deref().filter(|c| !c.is_empty()) {
        parts.push(format!("COLLATE={c}"));
    }
    if let Some(a) = o.auto_increment {
        parts.push(format!("AUTO_INCREMENT={a}"));
    }
    if let Some(c) = t.comment.as_deref().filter(|c| !c.is_empty()) {
        parts.push(format!("COMMENT={}", literal(engine, Some(c))));
    }
    parts.join(" ")
}

fn sqlite_inline_pk(engine: Engine, t: &TableDesign) -> Option<String> {
    if engine != Engine::Sqlite || t.primary_key.len() != 1 {
        return None;
    }
    let pk = &t.primary_key[0];
    t.columns
        .iter()
        .find(|c| &c.name == pk && c.auto_increment)
        .map(|c| c.name.clone())
}

// ---------------------------------------------------------------------------
// CREATE

pub fn create_table(engine: Engine, t: &TableDesign) -> Result<DdlPlan, String> {
    validate(t)?;
    let table = qualified(engine, t.schema.as_deref(), &t.name);
    let inline_pk = sqlite_inline_pk(engine, t);

    let mut body: Vec<String> = t
        .columns
        .iter()
        .map(|c| column_sql(engine, &t.name, c, inline_pk.as_deref() == Some(c.name.as_str())))
        .collect();
    if !t.primary_key.is_empty() && inline_pk.is_none() {
        body.push(pk_sql(engine, t));
    }
    body.extend(t.foreign_keys.iter().map(|f| fk_sql(engine, f)));
    body.extend(t.checks.iter().map(|c| check_sql(engine, c)));

    let mut create = format!("CREATE TABLE {table} (\n  {}\n)", body.join(",\n  "));
    if engine.is_mysql_family() {
        let opts = mysql_table_options(engine, t);
        if !opts.is_empty() {
            create.push(' ');
            create.push_str(&opts);
        }
    }

    let mut statements = vec![create];
    statements.extend(t.indexes.iter().map(|ix| index_sql(engine, t, ix)));
    if let Some(s) = comment_sql(engine, t, None, None, t.comment.as_deref()) {
        statements.push(s);
    }
    for c in &t.columns {
        if let Some(s) = comment_sql(engine, t, Some(&c.name), None, c.comment.as_deref()) {
            statements.push(s);
        }
    }
    statements.extend(t.triggers.iter().filter(|tr| !tr.definition.trim().is_empty()).map(|tr| tr.definition.trim().to_string()));

    Ok(DdlPlan { statements, transactional: matches!(engine, Engine::Postgres | Engine::Mssql | Engine::Sqlite) })
}

fn validate(t: &TableDesign) -> Result<(), String> {
    if t.name.trim().is_empty() {
        return Err("The table needs a name".into());
    }
    if t.columns.is_empty() {
        return Err("A table needs at least one field".into());
    }
    let mut seen = std::collections::HashSet::new();
    for c in &t.columns {
        if c.name.trim().is_empty() {
            return Err("Every field needs a name".into());
        }
        if c.data_type.trim().is_empty() && c.generated.is_none() {
            return Err(format!("Field \"{}\" needs a type", c.name));
        }
        if !seen.insert(c.name.to_lowercase()) {
            return Err(format!("There are two fields named \"{}\"", c.name));
        }
    }
    for pk in &t.primary_key {
        if !t.columns.iter().any(|c| &c.name == pk) {
            return Err(format!("The primary key names \"{pk}\", which is not a field"));
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// ALTER

/// The statements that turn `old` into `new`. Empty when nothing changed.
pub fn alter_table(engine: Engine, old: &TableDesign, new: &TableDesign) -> Result<DdlPlan, String> {
    validate(new)?;
    if engine == Engine::Sqlite {
        return alter_sqlite(old, new);
    }

    let mut out: Vec<String> = Vec::new();
    // Work against the table under its new name once renamed.
    let mut cur = new.clone();
    let old_name = new.original_name.clone().unwrap_or_else(|| old.name.clone());
    cur.name = old_name.clone();

    if old_name != new.name {
        out.push(match engine {
            Engine::Mysql | Engine::Mariadb => format!(
                "RENAME TABLE {} TO {}",
                qualified(engine, new.schema.as_deref(), &old_name),
                qualified(engine, new.schema.as_deref(), &new.name)
            ),
            Engine::Mssql => format!(
                "EXEC sp_rename {}, {}",
                literal(engine, Some(&format!("{}.{}", new.schema.clone().unwrap_or_else(|| "dbo".into()), old_name))),
                literal(engine, Some(&new.name))
            ),
            _ => format!(
                "ALTER TABLE {} RENAME TO {}",
                qualified(engine, new.schema.as_deref(), &old_name),
                ident(engine, &new.name)
            ),
        });
        cur.name = new.name.clone();
    }
    let table = qualified(engine, cur.schema.as_deref(), &cur.name);

    // Things that depend on columns go first, so dropping a column never
    // trips over its own index or key.
    let removed_fks: Vec<&ForeignKeyDef> = old.foreign_keys.iter().filter(|o| !kept_same(&new.foreign_keys, o, |f| &f.original, |a, b| fk_eq(a, b))).collect();
    for fk in &removed_fks {
        out.push(match engine {
            Engine::Mysql | Engine::Mariadb => format!("ALTER TABLE {table} DROP FOREIGN KEY {}", ident(engine, &fk.name)),
            _ => format!("ALTER TABLE {table} DROP CONSTRAINT {}", ident(engine, &fk.name)),
        });
    }
    for ix in old.indexes.iter().filter(|o| !kept_same(&new.indexes, o, |i| &i.original, |a, b| ix_eq(a, b))) {
        out.push(drop_index_sql(engine, &cur, ix, &ix.name));
    }
    for ck in old.checks.iter().filter(|o| !kept_same(&new.checks, o, |c| &c.original, |a, b| a.expression == b.expression && a.name == b.name)) {
        out.push(match engine {
            Engine::Mysql => format!("ALTER TABLE {table} DROP CHECK {}", ident(engine, &ck.name)),
            _ => format!("ALTER TABLE {table} DROP CONSTRAINT {}", ident(engine, &ck.name)),
        });
    }
    for tr in old.triggers.iter().filter(|o| !kept_same(&new.triggers, o, |t| &t.original, |a, b| a.definition.trim() == b.definition.trim())) {
        out.push(drop_trigger_sql(engine, &cur, &tr.name));
    }
    let pk_changed = old.primary_key != new.primary_key;
    if pk_changed && !old.primary_key.is_empty() {
        out.push(match (engine, old.primary_key_name.as_deref()) {
            (Engine::Mysql | Engine::Mariadb, _) => format!("ALTER TABLE {table} DROP PRIMARY KEY"),
            (_, Some(n)) => format!("ALTER TABLE {table} DROP CONSTRAINT {}", ident(engine, n)),
            (_, None) => return Err("The current primary key has no name to drop it by; reload the table and try again".into()),
        });
    }

    // Columns.
    for oc in &old.columns {
        if !new.columns.iter().any(|c| c.original.as_deref() == Some(oc.name.as_str())) {
            if engine == Engine::Mssql {
                if let Some(df) = &oc.default_constraint {
                    out.push(format!("ALTER TABLE {table} DROP CONSTRAINT {}", ident(engine, df)));
                }
            }
            out.push(format!("ALTER TABLE {table} DROP COLUMN {}", ident(engine, &oc.name)));
        }
    }
    for (pos, nc) in new.columns.iter().enumerate() {
        let position = mysql_position(engine, old, new, pos);
        match nc.original.as_deref().and_then(|o| old.columns.iter().find(|c| c.name == o)) {
            None => {
                let mut s = format!("ALTER TABLE {table} ADD {}{}", if engine == Engine::Mssql { "" } else { "COLUMN " }, column_sql(engine, &cur.name, nc, false));
                s.push_str(&position);
                out.push(s);
                if let Some(c) = comment_sql(engine, &cur, Some(&nc.name), None, nc.comment.as_deref()) {
                    out.push(c);
                }
            }
            Some(oc) => out.extend(alter_column(engine, &cur, oc, nc, &position)?),
        }
    }

    if pk_changed && !new.primary_key.is_empty() {
        out.push(format!("ALTER TABLE {table} ADD {}", pk_sql(engine, new)));
    }
    for ck in &new.checks {
        let unchanged = old.checks.iter().any(|o| Some(&o.name) == ck.original.as_ref() && o.expression == ck.expression && o.name == ck.name);
        if !unchanged {
            out.push(format!("ALTER TABLE {table} ADD {}", check_sql(engine, ck)));
        }
    }
    for ix in &new.indexes {
        let unchanged = old.indexes.iter().any(|o| Some(&o.name) == ix.original.as_ref() && ix_eq(o, ix));
        if !unchanged {
            out.push(index_sql(engine, &cur, ix));
        }
    }
    for fk in &new.foreign_keys {
        let unchanged = old.foreign_keys.iter().any(|o| Some(&o.name) == fk.original.as_ref() && fk_eq(o, fk));
        if !unchanged {
            out.push(format!("ALTER TABLE {table} ADD {}", fk_sql(engine, fk)));
        }
    }
    for tr in &new.triggers {
        let unchanged = old.triggers.iter().any(|o| Some(&o.name) == tr.original.as_ref() && o.definition.trim() == tr.definition.trim());
        if !unchanged && !tr.definition.trim().is_empty() {
            out.push(tr.definition.trim().to_string());
        }
    }

    if engine.is_mysql_family() {
        let mut opts = Vec::new();
        let (o, n) = (&old.options, &new.options);
        if o.engine != n.engine {
            if let Some(e) = n.engine.as_deref().filter(|e| !e.is_empty()) {
                opts.push(format!("ENGINE={e}"));
            }
        }
        if o.charset != n.charset || o.collation != n.collation {
            if let Some(c) = n.charset.as_deref().filter(|c| !c.is_empty()) {
                opts.push(format!("DEFAULT CHARSET={c}"));
            }
            if let Some(c) = n.collation.as_deref().filter(|c| !c.is_empty()) {
                opts.push(format!("COLLATE={c}"));
            }
        }
        if o.auto_increment != n.auto_increment {
            if let Some(a) = n.auto_increment {
                opts.push(format!("AUTO_INCREMENT={a}"));
            }
        }
        if old.comment != new.comment {
            opts.push(format!("COMMENT={}", literal(engine, Some(new.comment.as_deref().unwrap_or("")))));
        }
        if !opts.is_empty() {
            out.push(format!("ALTER TABLE {table} {}", opts.join(" ")));
        }
    } else if let Some(s) = comment_sql(engine, &cur, None, old.comment.as_deref(), new.comment.as_deref()) {
        out.push(s);
    }

    Ok(DdlPlan { statements: out, transactional: matches!(engine, Engine::Postgres | Engine::Mssql) })
}

/// Was `item` (from one side) kept unchanged on the `other` side?
fn kept_same<T>(other: &[T], item: &T, original: impl Fn(&T) -> &Option<String>, same: impl Fn(&T, &T) -> bool) -> bool
where
    T: HasName,
{
    // `item` comes from the old side; the new side refers to it by `original`.
    other.iter().any(|n| original(n).as_deref() == Some(item.name()) && same(item, n))
}

trait HasName {
    fn name(&self) -> &str;
}
impl HasName for IndexDef {
    fn name(&self) -> &str {
        &self.name
    }
}
impl HasName for ForeignKeyDef {
    fn name(&self) -> &str {
        &self.name
    }
}
impl HasName for CheckDef {
    fn name(&self) -> &str {
        &self.name
    }
}
impl HasName for TriggerDef {
    fn name(&self) -> &str {
        &self.name
    }
}

fn ix_eq(a: &IndexDef, b: &IndexDef) -> bool {
    a.name == b.name
        && a.columns == b.columns
        && a.unique == b.unique
        && a.constraint == b.constraint
        && a.method.as_deref().map(str::to_lowercase) == b.method.as_deref().map(str::to_lowercase)
        && a.where_clause == b.where_clause
        && a.comment == b.comment
}

fn fk_eq(a: &ForeignKeyDef, b: &ForeignKeyDef) -> bool {
    let norm = |s: &Option<String>| s.as_deref().map(|x| x.to_uppercase()).filter(|x| x != "NO ACTION");
    a.name == b.name
        && a.columns == b.columns
        && a.ref_schema == b.ref_schema
        && a.ref_table == b.ref_table
        && a.ref_columns == b.ref_columns
        && norm(&a.on_delete) == norm(&b.on_delete)
        && norm(&a.on_update) == norm(&b.on_update)
}

/// ` FIRST` / ` AFTER x` when a MySQL column's place in the order changed.
fn mysql_position(engine: Engine, old: &TableDesign, new: &TableDesign, pos: usize) -> String {
    if !engine.is_mysql_family() {
        return String::new();
    }
    let prev_new = if pos == 0 { None } else { Some(&new.columns[pos - 1]) };
    let this = &new.columns[pos];
    // Compare against the old neighbour, by original names.
    let old_prev = this.original.as_deref().and_then(|o| {
        let i = old.columns.iter().position(|c| c.name == o)?;
        Some(if i == 0 { None } else { Some(old.columns[i - 1].name.clone()) })
    });
    let new_prev_orig = prev_new.map(|p| p.original.clone());
    let moved = match (&old_prev, &new_prev_orig) {
        (Some(op), Some(np)) => op != np,
        (Some(op), None) => op.is_some(),
        (None, _) => false, // a new column: only place it when it is not last
    };
    let is_new_not_last = this.original.is_none() && pos + 1 < new.columns.len();
    if !(moved || is_new_not_last) {
        return String::new();
    }
    match prev_new {
        None => " FIRST".into(),
        Some(p) => format!(" AFTER {}", ident(engine, &p.name)),
    }
}

fn same_definition(a: &ColumnDef, b: &ColumnDef) -> bool {
    type_sql(a).eq_ignore_ascii_case(&type_sql(b))
        && a.not_null == b.not_null
        && a.default == b.default
        && a.auto_increment == b.auto_increment
        && a.collation == b.collation
        && a.unsigned == b.unsigned
        && a.on_update == b.on_update
        && a.generated == b.generated
        && a.generated_stored == b.generated_stored
}

fn alter_column(engine: Engine, t: &TableDesign, old: &ColumnDef, new: &ColumnDef, position: &str) -> Result<Vec<String>, String> {
    let table = qualified(engine, t.schema.as_deref(), &t.name);
    let renamed = old.name != new.name;
    let redefined = !same_definition(old, new);
    let mut out = Vec::new();

    match engine {
        Engine::Mysql | Engine::Mariadb => {
            // MySQL restates the whole column, comment included.
            if renamed || redefined || old.comment != new.comment || !position.is_empty() {
                let verb = if renamed {
                    format!("CHANGE COLUMN {} ", ident(engine, &old.name))
                } else {
                    "MODIFY COLUMN ".into()
                };
                out.push(format!("ALTER TABLE {table} {verb}{}{position}", column_sql(engine, &t.name, new, false)));
            }
        }
        Engine::Postgres => {
            let col = ident(engine, &new.name);
            if renamed {
                out.push(format!("ALTER TABLE {table} RENAME COLUMN {} TO {col}", ident(engine, &old.name)));
            }
            if old.generated != new.generated || old.generated_stored != new.generated_stored {
                return Err(format!("PostgreSQL cannot change how \"{}\" is generated in place. Remove the field and add it again.", new.name));
            }
            if !type_sql(old).eq_ignore_ascii_case(&type_sql(new)) || old.collation != new.collation {
                let ty = type_sql(new);
                out.push(format!("ALTER TABLE {table} ALTER COLUMN {col} TYPE {ty}{} USING {col}::{ty}", collate(engine, new)));
            }
            if old.not_null != new.not_null {
                out.push(format!("ALTER TABLE {table} ALTER COLUMN {col} {} NOT NULL", if new.not_null { "SET" } else { "DROP" }));
            }
            if old.default != new.default {
                match new.default.as_deref().filter(|d| !d.trim().is_empty()) {
                    Some(d) => out.push(format!("ALTER TABLE {table} ALTER COLUMN {col} SET DEFAULT {d}")),
                    None => out.push(format!("ALTER TABLE {table} ALTER COLUMN {col} DROP DEFAULT")),
                }
            }
            if old.auto_increment != new.auto_increment {
                out.push(if new.auto_increment {
                    format!("ALTER TABLE {table} ALTER COLUMN {col} ADD GENERATED BY DEFAULT AS IDENTITY")
                } else {
                    format!("ALTER TABLE {table} ALTER COLUMN {col} DROP IDENTITY IF EXISTS")
                });
            }
            if let Some(c) = comment_sql(engine, t, Some(&new.name), old.comment.as_deref(), new.comment.as_deref()) {
                out.push(c);
            }
        }
        Engine::Mssql => {
            let col = ident(engine, &new.name);
            if renamed {
                let schema = t.schema.clone().unwrap_or_else(|| "dbo".into());
                out.push(format!(
                    "EXEC sp_rename {}, {}, 'COLUMN'",
                    literal(engine, Some(&format!("{schema}.{}.{}", t.name, old.name))),
                    literal(engine, Some(&new.name))
                ));
            }
            if old.auto_increment != new.auto_increment || old.generated != new.generated {
                return Err(format!("SQL Server cannot change identity or computed settings of \"{}\" in place. Remove the field and add it again.", new.name));
            }
            if !type_sql(old).eq_ignore_ascii_case(&type_sql(new)) || old.not_null != new.not_null || old.collation != new.collation {
                // ALTER COLUMN resets nullability unless it is restated.
                out.push(format!(
                    "ALTER TABLE {table} ALTER COLUMN {col} {}{}{}",
                    type_sql(new),
                    collate(engine, new),
                    if new.not_null { " NOT NULL" } else { " NULL" }
                ));
            }
            if old.default != new.default {
                if let Some(df) = &old.default_constraint {
                    out.push(format!("ALTER TABLE {table} DROP CONSTRAINT {}", ident(engine, df)));
                }
                if let Some(d) = new.default.as_deref().filter(|d| !d.trim().is_empty()) {
                    out.push(format!(
                        "ALTER TABLE {table} ADD CONSTRAINT {} DEFAULT {d} FOR {col}",
                        ident(engine, &default_name(&t.name, &new.name))
                    ));
                }
            }
            if let Some(c) = comment_sql(engine, t, Some(&new.name), old.comment.as_deref(), new.comment.as_deref()) {
                out.push(c);
            }
        }
        Engine::Sqlite | Engine::Redis => unreachable!("SQLite alters by rebuilding"),
    }
    Ok(out)
}

/// SQLite can add, rename and drop a column in place. Anything else means
/// the rebuild its documentation describes: create the new shape, copy the
/// rows, drop the old table, rename the new one, restore indexes and
/// triggers. Foreign-key enforcement is off for the duration so the drop does
/// not cascade into other tables.
fn alter_sqlite(old: &TableDesign, new: &TableDesign) -> Result<DdlPlan, String> {
    let e = Engine::Sqlite;
    let old_name = new.original_name.clone().unwrap_or_else(|| old.name.clone());

    let simple_columns = new.columns.iter().all(|nc| match nc.original.as_deref() {
        None => nc.generated.is_none() || !nc.generated_stored,
        Some(o) => old.columns.iter().find(|c| c.name == o).is_some_and(|oc| same_definition(oc, nc)),
    });
    let order_kept = {
        let kept: Vec<&str> = new.columns.iter().filter_map(|c| c.original.as_deref()).collect();
        let old_order: Vec<&str> = old.columns.iter().map(|c| c.name.as_str()).filter(|n| kept.contains(n)).collect();
        // Added columns can only go at the end.
        let first_new = new.columns.iter().position(|c| c.original.is_none()).unwrap_or(new.columns.len());
        kept == old_order && new.columns[first_new..].iter().all(|c| c.original.is_none())
    };
    let no_structure_change = old.primary_key == new.primary_key
        && old.foreign_keys.len() == new.foreign_keys.len()
        && old.foreign_keys.iter().all(|o| new.foreign_keys.iter().any(|n| fk_eq(o, n)))
        && old.checks.len() == new.checks.len()
        && old.checks.iter().all(|o| new.checks.iter().any(|n| n.expression == o.expression && n.name == o.name));

    let mut out = Vec::new();
    if simple_columns && order_kept && no_structure_change {
        let mut table = ident(e, &old_name);
        if old_name != new.name {
            out.push(format!("ALTER TABLE {table} RENAME TO {}", ident(e, &new.name)));
            table = ident(e, &new.name);
        }
        for oc in &old.columns {
            if !new.columns.iter().any(|c| c.original.as_deref() == Some(oc.name.as_str())) {
                out.push(format!("ALTER TABLE {table} DROP COLUMN {}", ident(e, &oc.name)));
            }
        }
        for nc in &new.columns {
            match nc.original.as_deref() {
                None => out.push(format!("ALTER TABLE {table} ADD COLUMN {}", column_sql(e, &new.name, nc, false))),
                Some(o) if o != nc.name => {
                    out.push(format!("ALTER TABLE {table} RENAME COLUMN {} TO {}", ident(e, o), ident(e, &nc.name)))
                }
                _ => {}
            }
        }
        let cur = TableDesign { name: new.name.clone(), ..new.clone() };
        for ix in old.indexes.iter().filter(|o| !new.indexes.iter().any(|n| n.original.as_deref() == Some(o.name.as_str()) && ix_eq(o, n))) {
            out.push(drop_index_sql(e, &cur, ix, &ix.name));
        }
        for ix in new.indexes.iter().filter(|n| !old.indexes.iter().any(|o| n.original.as_deref() == Some(o.name.as_str()) && ix_eq(o, n))) {
            out.push(index_sql(e, &cur, ix));
        }
        for tr in old.triggers.iter().filter(|o| !new.triggers.iter().any(|n| n.original.as_deref() == Some(o.name.as_str()) && n.definition.trim() == o.definition.trim())) {
            out.push(drop_trigger_sql(e, &cur, &tr.name));
        }
        for tr in new.triggers.iter().filter(|n| !old.triggers.iter().any(|o| n.original.as_deref() == Some(o.name.as_str()) && n.definition.trim() == o.definition.trim())) {
            out.push(tr.definition.trim().to_string());
        }
        return Ok(DdlPlan { statements: out, transactional: true });
    }

    // Rebuild.
    let temp = format!("_reach_new_{}", new.name);
    let shape = TableDesign { name: temp.clone(), original_name: None, indexes: vec![], triggers: vec![], ..new.clone() };
    let create = create_table(e, &shape)?.statements.remove(0);
    let copied: Vec<(&str, &str)> = new
        .columns
        .iter()
        .filter(|c| c.generated.is_none())
        .filter_map(|c| {
            let o = c.original.as_deref()?;
            old.columns.iter().find(|oc| oc.name == o && oc.generated.is_none()).map(|_| (o, c.name.as_str()))
        })
        .collect();
    let into = copied.iter().map(|(_, n)| ident(e, n)).collect::<Vec<_>>().join(", ");
    let from = copied.iter().map(|(o, _)| ident(e, o)).collect::<Vec<_>>().join(", ");

    out.push("PRAGMA foreign_keys = OFF".into());
    out.push("BEGIN".into());
    out.push(create);
    if !copied.is_empty() {
        out.push(format!("INSERT INTO {} ({into}) SELECT {from} FROM {}", ident(e, &temp), ident(e, &old_name)));
    }
    out.push(format!("DROP TABLE {}", ident(e, &old_name)));
    out.push(format!("ALTER TABLE {} RENAME TO {}", ident(e, &temp), ident(e, &new.name)));
    out.extend(new.indexes.iter().map(|ix| index_sql(e, new, ix)));
    out.extend(new.triggers.iter().filter(|t| !t.definition.trim().is_empty()).map(|t| t.definition.trim().to_string()));
    out.push("PRAGMA foreign_key_check".into());
    out.push("COMMIT".into());
    out.push("PRAGMA foreign_keys = ON".into());
    Ok(DdlPlan { statements: out, transactional: false })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn col(name: &str, ty: &str) -> ColumnDef {
        ColumnDef { original: Some(name.into()), name: name.into(), data_type: ty.into(), ..Default::default() }
    }

    fn users() -> TableDesign {
        TableDesign {
            schema: Some("public".into()),
            original_name: Some("users".into()),
            name: "users".into(),
            columns: vec![
                ColumnDef { not_null: true, auto_increment: true, ..col("id", "integer") },
                ColumnDef { length: Some(100), ..col("name", "varchar") },
            ],
            primary_key: vec!["id".into()],
            primary_key_name: Some("users_pkey".into()),
            ..Default::default()
        }
    }

    #[test]
    fn creates_a_postgres_table_with_identity_and_comment() {
        let mut t = users();
        t.original_name = None;
        t.comment = Some("People".into());
        let plan = create_table(Engine::Postgres, &t).unwrap();
        assert_eq!(
            plan.statements[0],
            "CREATE TABLE \"public\".\"users\" (\n  \"id\" integer GENERATED BY DEFAULT AS IDENTITY NOT NULL,\n  \"name\" varchar(100) NULL,\n  CONSTRAINT \"users_pkey\" PRIMARY KEY (\"id\")\n)"
        );
        assert_eq!(plan.statements[1], "COMMENT ON TABLE \"public\".\"users\" IS 'People'");
    }

    #[test]
    fn mysql_create_writes_options_and_inline_comments() {
        let mut t = users();
        t.schema = None;
        t.columns[1].comment = Some("Full name".into());
        t.options.engine = Some("InnoDB".into());
        let plan = create_table(Engine::Mysql, &t).unwrap();
        assert!(plan.statements[0].contains("`id` integer NOT NULL AUTO_INCREMENT"), "{}", plan.statements[0]);
        assert!(plan.statements[0].contains("COMMENT 'Full name'"));
        assert!(plan.statements[0].ends_with(") ENGINE=InnoDB"));
        assert!(!plan.transactional);
    }

    #[test]
    fn sqlite_single_autoincrement_key_goes_inline() {
        let mut t = users();
        t.schema = None;
        let s = &create_table(Engine::Sqlite, &t).unwrap().statements[0];
        assert!(s.contains("\"id\" INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL"), "{s}");
        assert!(!s.contains("PRIMARY KEY (\"id\")"));
    }

    #[test]
    fn unchanged_design_alters_nothing() {
        for e in [Engine::Postgres, Engine::Mysql, Engine::Mssql, Engine::Sqlite] {
            let plan = alter_table(e, &users(), &users()).unwrap();
            assert!(plan.statements.is_empty(), "{e:?}: {:?}", plan.statements);
        }
    }

    #[test]
    fn a_rename_keeps_the_column_rather_than_dropping_it() {
        let mut new = users();
        new.columns[1].name = "full_name".into();
        let pg = alter_table(Engine::Postgres, &users(), &new).unwrap().statements;
        assert_eq!(pg, vec!["ALTER TABLE \"public\".\"users\" RENAME COLUMN \"name\" TO \"full_name\""]);

        let my = alter_table(Engine::Mysql, &users(), &new).unwrap().statements;
        assert_eq!(my, vec!["ALTER TABLE `public`.`users` CHANGE COLUMN `name` `full_name` varchar(100) NULL"]);

        let sq = alter_table(Engine::Sqlite, &users(), &new).unwrap().statements;
        assert_eq!(sq, vec!["ALTER TABLE \"users\" RENAME COLUMN \"name\" TO \"full_name\""]);
    }

    #[test]
    fn postgres_type_null_and_default_changes_each_get_a_clause() {
        let mut new = users();
        new.columns[1].length = Some(200);
        new.columns[1].not_null = true;
        new.columns[1].default = Some("''".into());
        let s = alter_table(Engine::Postgres, &users(), &new).unwrap().statements;
        assert_eq!(
            s,
            vec![
                "ALTER TABLE \"public\".\"users\" ALTER COLUMN \"name\" TYPE varchar(200) USING \"name\"::varchar(200)",
                "ALTER TABLE \"public\".\"users\" ALTER COLUMN \"name\" SET NOT NULL",
                "ALTER TABLE \"public\".\"users\" ALTER COLUMN \"name\" SET DEFAULT ''",
            ]
        );
    }

    #[test]
    fn sql_server_restates_nullability_and_swaps_default_constraints() {
        let mut old = users();
        old.schema = Some("dbo".into());
        old.columns[1].default = Some("('x')".into());
        old.columns[1].default_constraint = Some("DF__users__name__1234".into());
        let mut new = old.clone();
        new.columns[1].length = Some(150);
        new.columns[1].default = Some("('y')".into());
        let s = alter_table(Engine::Mssql, &old, &new).unwrap().statements;
        assert_eq!(
            s,
            vec![
                "ALTER TABLE [dbo].[users] ALTER COLUMN [name] varchar(150) NULL",
                "ALTER TABLE [dbo].[users] DROP CONSTRAINT [DF__users__name__1234]",
                "ALTER TABLE [dbo].[users] ADD CONSTRAINT [DF_users_name] DEFAULT ('y') FOR [name]",
            ]
        );
    }

    #[test]
    fn dropping_a_column_drops_its_index_first() {
        let mut old = users();
        old.indexes.push(IndexDef {
            original: Some("ix_name".into()),
            name: "ix_name".into(),
            columns: vec![IndexColumn { name: "name".into(), ..Default::default() }],
            ..Default::default()
        });
        let mut new = users();
        new.columns.pop();
        let s = alter_table(Engine::Postgres, &old, &new).unwrap().statements;
        assert_eq!(
            s,
            vec![
                "DROP INDEX \"public\".\"ix_name\"",
                "ALTER TABLE \"public\".\"users\" DROP COLUMN \"name\"",
            ]
        );
    }

    #[test]
    fn mysql_moving_a_column_says_where() {
        let mut new = users();
        new.columns.swap(0, 1);
        let s = alter_table(Engine::Mysql, &users(), &new).unwrap().statements;
        assert!(s.iter().any(|x| x.ends_with(" FIRST")), "{s:?}");
    }

    #[test]
    fn sqlite_type_change_rebuilds_and_copies_rows() {
        let mut old = users();
        old.schema = None;
        let mut new = old.clone();
        new.columns[1].data_type = "text".into();
        new.columns[1].length = None;
        let plan = alter_table(Engine::Sqlite, &old, &new).unwrap();
        assert!(!plan.transactional);
        let s = plan.statements;
        assert_eq!(s[0], "PRAGMA foreign_keys = OFF");
        assert!(s.contains(&"INSERT INTO \"_reach_new_users\" (\"id\", \"name\") SELECT \"id\", \"name\" FROM \"users\"".to_string()), "{s:?}");
        assert!(s.contains(&"ALTER TABLE \"_reach_new_users\" RENAME TO \"users\"".to_string()));
        assert_eq!(s.last().unwrap(), "PRAGMA foreign_keys = ON");
    }

    #[test]
    fn primary_key_change_drops_by_name_then_adds() {
        let mut new = users();
        new.primary_key = vec!["id".into(), "name".into()];
        let s = alter_table(Engine::Postgres, &users(), &new).unwrap().statements;
        assert_eq!(
            s,
            vec![
                "ALTER TABLE \"public\".\"users\" DROP CONSTRAINT \"users_pkey\"",
                "ALTER TABLE \"public\".\"users\" ADD CONSTRAINT \"users_pkey\" PRIMARY KEY (\"id\", \"name\")",
            ]
        );
    }

    #[test]
    fn sql_server_indexes_can_be_clustered() {
        let mut t = users();
        t.schema = Some("dbo".into());
        let ix = IndexDef { name: "ix".into(), unique: true, method: Some("clustered".into()), columns: vec![IndexColumn { name: "name".into(), ..Default::default() }], ..Default::default() };
        assert_eq!(index_sql(Engine::Mssql, &t, &ix), "CREATE UNIQUE CLUSTERED INDEX [ix] ON [dbo].[users] ([name])");
    }

    #[test]
    fn duplicate_field_names_are_refused() {
        let mut t = users();
        t.columns[1].name = "ID".into();
        assert!(create_table(Engine::Postgres, &t).is_err());
    }
}
