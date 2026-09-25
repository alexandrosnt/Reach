//! The SQL Reach writes itself: names, values, row edits.
//!
//! Row edits are rendered as literal SQL rather than bound parameters, on
//! purpose. The user reviews the exact text before it runs, and a string
//! literal is the one form every engine converts to any column type — a
//! parameter bound as text is rejected by a PostgreSQL integer column.

use serde::Deserialize;

use super::types::Engine;

/// Quote an identifier.
pub fn ident(engine: Engine, name: &str) -> String {
    match engine {
        Engine::Mysql | Engine::Mariadb => format!("`{}`", name.replace('`', "``")),
        Engine::Mssql => format!("[{}]", name.replace(']', "]]")),
        Engine::Postgres | Engine::Sqlite | Engine::Redis => format!("\"{}\"", name.replace('"', "\"\"")),
    }
}

/// `schema.table`, or just `table` when there is no schema to name.
pub fn qualified(engine: Engine, schema: Option<&str>, table: &str) -> String {
    match schema.filter(|s| !s.is_empty()) {
        Some(s) => format!("{}.{}", ident(engine, s), ident(engine, table)),
        None => ident(engine, table),
    }
}

/// A value as a SQL literal. `None` is NULL.
pub fn literal(engine: Engine, value: Option<&str>) -> String {
    let Some(v) = value else { return "NULL".into() };
    match engine {
        // MySQL treats backslash as an escape inside strings by default.
        Engine::Mysql | Engine::Mariadb => {
            format!("'{}'", v.replace('\\', "\\\\").replace('\'', "''"))
        }
        // N'' keeps non-Latin text intact in nvarchar columns.
        Engine::Mssql => format!("N'{}'", v.replace('\'', "''")),
        Engine::Postgres | Engine::Sqlite | Engine::Redis => format!("'{}'", v.replace('\'', "''")),
    }
}

/// `LIMIT`/`OFFSET` for a page of a table.
pub fn page(engine: Engine, select: &str, order_by: Option<&str>, limit: u64, offset: u64) -> String {
    match engine {
        Engine::Mssql => {
            // OFFSET … FETCH needs an ORDER BY; (SELECT NULL) is the no-op one.
            let order = order_by.map(str::to_string).unwrap_or_else(|| "(SELECT NULL)".into());
            format!("{select} ORDER BY {order} OFFSET {offset} ROWS FETCH NEXT {limit} ROWS ONLY")
        }
        _ => {
            let order = order_by.map(|o| format!(" ORDER BY {o}")).unwrap_or_default();
            format!("{select}{order} LIMIT {limit} OFFSET {offset}")
        }
    }
}

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum RowEdit {
    /// Change `values` in the row identified by `key`.
    Update { key: Vec<Cell>, values: Vec<Cell> },
    Insert { values: Vec<Cell> },
    Delete { key: Vec<Cell> },
}

#[derive(Deserialize, Clone, Debug)]
pub struct Cell {
    pub column: String,
    pub value: Option<String>,
}

/// The statements for a set of pending edits, in the order they were made.
pub fn row_edits(engine: Engine, schema: Option<&str>, table: &str, edits: &[RowEdit]) -> Result<Vec<String>, String> {
    let t = qualified(engine, schema, table);
    edits
        .iter()
        .map(|edit| match edit {
            RowEdit::Update { key, values } => {
                if values.is_empty() {
                    return Err("An update needs at least one changed value".into());
                }
                let set = values
                    .iter()
                    .map(|c| format!("{} = {}", ident(engine, &c.column), literal(engine, c.value.as_deref())))
                    .collect::<Vec<_>>()
                    .join(", ");
                Ok(format!("UPDATE {t} SET {set} WHERE {}", key_filter(engine, key)?))
            }
            RowEdit::Insert { values } => {
                if values.is_empty() {
                    return Ok(match engine {
                        Engine::Mysql | Engine::Mariadb => format!("INSERT INTO {t} () VALUES ()"),
                        _ => format!("INSERT INTO {t} DEFAULT VALUES"),
                    });
                }
                let cols = values.iter().map(|c| ident(engine, &c.column)).collect::<Vec<_>>().join(", ");
                let vals = values.iter().map(|c| literal(engine, c.value.as_deref())).collect::<Vec<_>>().join(", ");
                Ok(format!("INSERT INTO {t} ({cols}) VALUES ({vals})"))
            }
            RowEdit::Delete { key } => Ok(format!("DELETE FROM {t} WHERE {}", key_filter(engine, key)?)),
        })
        .collect()
}

/// `a = 1 AND b IS NULL`. Refuses an empty key: an edit that cannot name its
/// row must never become an unfiltered UPDATE or DELETE.
fn key_filter(engine: Engine, key: &[Cell]) -> Result<String, String> {
    if key.is_empty() {
        return Err("This table has no primary key, so a row cannot be picked out safely. Edit it with SQL instead.".into());
    }
    Ok(key
        .iter()
        .map(|c| match &c.value {
            Some(v) => format!("{} = {}", ident(engine, &c.column), literal(engine, Some(v))),
            None => format!("{} IS NULL", ident(engine, &c.column)),
        })
        .collect::<Vec<_>>()
        .join(" AND "))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cell(c: &str, v: Option<&str>) -> Cell {
        Cell { column: c.into(), value: v.map(str::to_string) }
    }

    #[test]
    fn identifiers_escape_their_own_quote() {
        assert_eq!(ident(Engine::Postgres, r#"we"ird"#), r#""we""ird""#);
        assert_eq!(ident(Engine::Mysql, "a`b"), "`a``b`");
        assert_eq!(ident(Engine::Mssql, "a]b"), "[a]]b]");
    }

    #[test]
    fn literals_cannot_break_out_of_their_quotes() {
        assert_eq!(literal(Engine::Postgres, Some("it's")), "'it''s'");
        assert_eq!(literal(Engine::Mysql, Some(r"a\'; drop")), r"'a\\''; drop'");
        assert_eq!(literal(Engine::Mssql, Some("Ω")), "N'Ω'");
        assert_eq!(literal(Engine::Sqlite, None), "NULL");
    }

    #[test]
    fn edits_render_in_order_with_null_keys_as_is_null() {
        let sql = row_edits(
            Engine::Postgres,
            Some("public"),
            "users",
            &[
                RowEdit::Update { key: vec![cell("id", Some("7"))], values: vec![cell("name", Some("Ann")), cell("note", None)] },
                RowEdit::Insert { values: vec![cell("name", Some("Bo"))] },
                RowEdit::Delete { key: vec![cell("a", Some("1")), cell("b", None)] },
            ],
        )
        .unwrap();
        assert_eq!(
            sql,
            vec![
                r#"UPDATE "public"."users" SET "name" = 'Ann', "note" = NULL WHERE "id" = '7'"#,
                r#"INSERT INTO "public"."users" ("name") VALUES ('Bo')"#,
                r#"DELETE FROM "public"."users" WHERE "a" = '1' AND "b" IS NULL"#,
            ]
        );
    }

    #[test]
    fn an_edit_without_a_key_is_refused() {
        assert!(row_edits(Engine::Mysql, None, "t", &[RowEdit::Delete { key: vec![] }]).is_err());
    }

    #[test]
    fn sql_server_pages_need_an_order() {
        assert_eq!(
            page(Engine::Mssql, "SELECT * FROM [t]", None, 100, 200),
            "SELECT * FROM [t] ORDER BY (SELECT NULL) OFFSET 200 ROWS FETCH NEXT 100 ROWS ONLY"
        );
        assert_eq!(page(Engine::Sqlite, "SELECT * FROM \"t\"", None, 10, 0), "SELECT * FROM \"t\" LIMIT 10 OFFSET 0");
    }
}
