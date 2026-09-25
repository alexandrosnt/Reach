//! Splitting a script into statements, and deciding which ones deserve a
//! second look before they run.
//!
//! This is a lexer, not a parser. It knows enough — strings, quoted names,
//! comments, PostgreSQL dollar quotes, SQL Server `GO` lines — to find where
//! one statement ends and to read the words that matter at the top level of
//! each. Anything it cannot place is treated as a write, never as a read, so a
//! mistake here costs a confirmation, not a dropped table.

use serde::Serialize;

use super::types::Engine;

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase", tag = "kind", content = "reason")]
pub enum Risk {
    /// Reads only.
    Read,
    /// Session plumbing: transactions, `SET`, `USE`.
    Session,
    /// Changes data or structure in the ordinary way.
    Write,
    /// Destroys or rewrites more than it probably means to.
    Danger(String),
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Statement {
    pub sql: String,
    pub risk: Risk,
    /// Where it sits in the script, in UTF-16 code units — what the editor
    /// counts in — so "run the statement under the cursor" can find it.
    pub from: usize,
    pub to: usize,
}

/// Split and classify. Empty statements (a stray `;`) are dropped.
pub fn analyze(engine: Engine, script: &str) -> Vec<Statement> {
    split_spans(engine, script)
        .into_iter()
        .map(|(start, end)| {
            let sql = script[start..end].to_string();
            let risk = classify_batch(engine, &sql);
            Statement { from: utf16_len(&script[..start]), to: utf16_len(&script[..end]), sql, risk }
        })
        .collect()
}

fn utf16_len(s: &str) -> usize {
    s.chars().map(char::len_utf16).sum()
}

/// The statements of a script, trimmed.
pub fn split(engine: Engine, script: &str) -> Vec<String> {
    split_spans(engine, script).into_iter().map(|(a, b)| script[a..b].to_string()).collect()
}

/// Byte spans of each statement, trimmed of surrounding whitespace.
///
/// SQL Server scripts split on `GO` lines only, as SQL Server Management
/// Studio does: a batch is what the server runs, and procedure bodies are full
/// of semicolons. Elsewhere a top-level `;` ends a statement, except inside
/// the `BEGIN … END` body of a MySQL or SQLite trigger, procedure, function or
/// event, and MySQL also honours the client's `DELIMITER` command.
fn split_spans(engine: Engine, script: &str) -> Vec<(usize, usize)> {
    let toks: Vec<(usize, Tok)> = Lexer::new(engine, script).collect();
    let compound_bodies = matches!(engine, Engine::Mysql | Engine::Mariadb | Engine::Sqlite);
    let mut out = Vec::new();
    let mut start = 0;
    let mut first_word: Option<String> = None;
    let mut compound = false;
    let mut depth = 0i32;

    let end_statement = |out: &mut Vec<(usize, usize)>, from: usize, to: usize| {
        let raw = &script[from..to];
        let lead = raw.len() - raw.trim_start().len();
        let trimmed = raw.trim();
        if !trimmed.is_empty() && Lexer::new(engine, trimmed).any(|(_, t)| matches!(t, Tok::Word(_))) {
            out.push((from + lead, from + lead + trimmed.len()));
        }
    };

    for (i, (pos, tok)) in toks.iter().enumerate() {
        match tok {
            Tok::Word(w) => {
                if first_word.is_none() {
                    first_word = Some(w.clone());
                }
                if compound_bodies
                    && first_word.as_deref() == Some("create")
                    && depth == 0
                    && matches!(w.as_str(), "trigger" | "procedure" | "function" | "event")
                {
                    compound = true;
                }
                if compound {
                    match w.as_str() {
                        "begin" | "case" => depth += 1,
                        "end" => {
                            let next = toks[i + 1..].iter().find_map(|(_, t)| match t {
                                Tok::Word(n) => Some(n.as_str()),
                                Tok::Other | Tok::Open | Tok::Close => Some(""),
                                _ => None,
                            });
                            if !matches!(next, Some("if" | "loop" | "while" | "repeat")) {
                                depth -= 1;
                            }
                        }
                        _ => {}
                    }
                }
            }
            Tok::End(len) if engine != Engine::Mssql => {
                if depth > 0 {
                    continue;
                }
                end_statement(&mut out, start, *pos);
                start = pos + len;
                first_word = None;
                compound = false;
                depth = 0;
            }
            Tok::Go(after) | Tok::Delimiter(after) => {
                end_statement(&mut out, start, *pos);
                start = *after;
                first_word = None;
                compound = false;
                depth = 0;
            }
            _ => {}
        }
    }
    end_statement(&mut out, start, script.len());
    out
}

/// A SQL Server batch can hold several statements; it is as risky as the
/// riskiest of them. Other engines already split to single statements.
pub fn classify_batch(engine: Engine, sql: &str) -> Risk {
    if engine != Engine::Mssql {
        return classify(engine, sql);
    }
    let rank = |r: &Risk| match r {
        Risk::Read => 0,
        Risk::Session => 1,
        Risk::Write => 2,
        Risk::Danger(_) => 3,
    };
    let mut worst = Risk::Read;
    for (a, b) in split_spans(Engine::Postgres, sql) {
        let r = classify(engine, &sql[a..b]);
        if rank(&r) > rank(&worst) {
            worst = r;
        }
    }
    worst
}

/// Words at parenthesis depth zero, lower-cased, in order.
fn top_words(engine: Engine, sql: &str) -> Vec<String> {
    let mut depth = 0i32;
    let mut words = Vec::new();
    for (_, tok) in Lexer::new(engine, sql) {
        match tok {
            Tok::Open => depth += 1,
            Tok::Close => depth -= 1,
            Tok::Word(w) if depth <= 0 => words.push(w),
            _ => {}
        }
    }
    words
}

/// Every word, at any depth — for spotting DML inside a `WITH`.
fn all_words(engine: Engine, sql: &str) -> Vec<String> {
    Lexer::new(engine, sql)
        .filter_map(|(_, t)| if let Tok::Word(w) = t { Some(w) } else { None })
        .collect()
}

pub fn classify(engine: Engine, sql: &str) -> Risk {
    let words = top_words(engine, sql);
    let Some(first) = words.first().map(String::as_str) else {
        return Risk::Read;
    };
    let has = |w: &str| words.iter().any(|x| x == w);

    match first {
        "select" | "show" | "describe" | "desc" | "values" | "table" | "help" => {
            // SELECT … INTO creates a table in PostgreSQL and SQL Server.
            if first == "select" && has("into") {
                Risk::Write
            } else {
                Risk::Read
            }
        }
        "explain" => {
            // EXPLAIN ANALYZE runs the statement.
            if has("analyze") || has("analyse") {
                classify_rest(engine, sql, &words)
            } else {
                Risk::Read
            }
        }
        "pragma" => {
            if sql.contains('=') {
                Risk::Write
            } else {
                Risk::Read
            }
        }
        "with" => {
            let all = all_words(engine, sql);
            let writes = ["insert", "update", "delete", "merge"];
            if all.iter().any(|w| writes.contains(&w.as_str())) {
                Risk::Write
            } else {
                Risk::Read
            }
        }
        "begin" | "start" | "commit" | "rollback" | "savepoint" | "release" | "set" | "use"
        | "reset" | "discard" => Risk::Session,
        "drop" => Risk::Danger(format!("DROP {}", words.get(1).map(|w| w.to_uppercase()).unwrap_or_default())),
        "truncate" => Risk::Danger("TRUNCATE empties the whole table".into()),
        "delete" | "update" => {
            if has("where") {
                Risk::Write
            } else {
                Risk::Danger(format!("{} without WHERE touches every row", first.to_uppercase()))
            }
        }
        "alter" => {
            if has("drop") {
                Risk::Danger("ALTER … DROP removes a column, constraint or index".into())
            } else {
                Risk::Write
            }
        }
        _ => Risk::Write,
    }
}

fn classify_rest(engine: Engine, sql: &str, words: &[String]) -> Risk {
    // Classify whatever follows EXPLAIN's options as if it stood alone.
    let verbs = ["select", "insert", "update", "delete", "with", "values", "merge", "create"];
    match words.iter().position(|w| verbs.contains(&w.as_str())) {
        Some(i) => {
            let lower = sql.to_lowercase();
            // Find the verb in the text to classify from there.
            match lower.find(&words[i]) {
                Some(at) => classify(engine, &sql[at..]),
                None => Risk::Write,
            }
        }
        None => Risk::Write,
    }
}

#[derive(Debug, PartialEq)]
enum Tok {
    Word(String),
    Open,
    Close,
    /// The end of a statement: `;`, or MySQL's current `DELIMITER`. Carries
    /// its length in bytes.
    End(usize),
    /// A SQL Server `GO` line; carries the byte offset just past it.
    Go(usize),
    /// A MySQL `DELIMITER x` line; carries the byte offset just past it.
    Delimiter(usize),
    Other,
}

struct Lexer<'a> {
    engine: Engine,
    src: &'a str,
    bytes: &'a [u8],
    i: usize,
    line_start: bool,
    /// MySQL's statement terminator, changed by `DELIMITER`.
    delimiter: String,
}

impl<'a> Lexer<'a> {
    fn new(engine: Engine, src: &'a str) -> Self {
        Self { engine, src, bytes: src.as_bytes(), i: 0, line_start: true, delimiter: ";".into() }
    }

    fn skip_quoted(&mut self, quote: u8, backslash: bool) {
        self.i += 1;
        while self.i < self.bytes.len() {
            let c = self.bytes[self.i];
            if backslash && c == b'\\' {
                self.i += 2;
                continue;
            }
            if c == quote {
                // A doubled quote is an escaped one.
                if self.bytes.get(self.i + 1) == Some(&quote) {
                    self.i += 2;
                    continue;
                }
                self.i += 1;
                return;
            }
            self.i += 1;
        }
    }

    /// PostgreSQL `$tag$ … $tag$`. Returns false if this `$` does not open one.
    fn skip_dollar(&mut self) -> bool {
        let rest = &self.src[self.i..];
        let Some(end) = rest[1..].find('$') else { return false };
        let tag = &rest[..end + 2];
        if !tag[1..tag.len() - 1].chars().all(|c| c.is_alphanumeric() || c == '_') {
            return false;
        }
        match rest[tag.len()..].find(tag) {
            Some(close) => self.i += tag.len() + close + tag.len(),
            None => self.i = self.bytes.len(),
        }
        true
    }
}

impl Iterator for Lexer<'_> {
    type Item = (usize, Tok);

    fn next(&mut self) -> Option<(usize, Tok)> {
        loop {
            let start = self.i;
            let c = *self.bytes.get(self.i)?;
            let at_line_start = self.line_start;
            if c == b'\n' {
                self.line_start = true;
                self.i += 1;
                continue;
            }
            if c.is_ascii_whitespace() {
                self.i += 1;
                continue;
            }
            self.line_start = false;
            let next = self.bytes.get(self.i + 1).copied();
            let mysql = self.engine.is_mysql_family();

            if mysql && self.delimiter != ";" && self.src[self.i..].starts_with(self.delimiter.as_str()) {
                self.i += self.delimiter.len();
                return Some((start, Tok::End(self.delimiter.len())));
            }
            if c == b'-' && next == Some(b'-') || (mysql && c == b'#') {
                while self.i < self.bytes.len() && self.bytes[self.i] != b'\n' {
                    self.i += 1;
                }
                continue;
            }
            if c == b'/' && next == Some(b'*') {
                match self.src[self.i + 2..].find("*/") {
                    Some(end) => self.i += 2 + end + 2,
                    None => self.i = self.bytes.len(),
                }
                continue;
            }
            match c {
                b'\'' => {
                    self.skip_quoted(b'\'', mysql);
                    return Some((start, Tok::Other));
                }
                b'"' => {
                    self.skip_quoted(b'"', mysql);
                    return Some((start, Tok::Other));
                }
                b'`' => {
                    self.skip_quoted(b'`', false);
                    return Some((start, Tok::Other));
                }
                b'[' if self.engine == Engine::Mssql => {
                    self.skip_quoted(b']', false);
                    return Some((start, Tok::Other));
                }
                b'$' if self.engine == Engine::Postgres && self.skip_dollar() => {
                    return Some((start, Tok::Other));
                }
                b'(' => {
                    self.i += 1;
                    return Some((start, Tok::Open));
                }
                b')' => {
                    self.i += 1;
                    return Some((start, Tok::Close));
                }
                b';' if !mysql || self.delimiter == ";" => {
                    self.i += 1;
                    return Some((start, Tok::End(1)));
                }
                _ => {}
            }
            if c.is_ascii_alphabetic() || c == b'_' {
                while self.i < self.bytes.len()
                    && (self.bytes[self.i].is_ascii_alphanumeric() || matches!(self.bytes[self.i], b'_' | b'$'))
                {
                    self.i += 1;
                }
                let word = self.src[start..self.i].to_ascii_lowercase();
                if mysql && at_line_start && word == "delimiter" {
                    let line_end = self.src[self.i..].find('\n').map(|n| self.i + n).unwrap_or(self.bytes.len());
                    let d = self.src[self.i..line_end].trim();
                    if !d.is_empty() {
                        self.delimiter = d.to_string();
                        self.i = line_end;
                        return Some((start, Tok::Delimiter(line_end)));
                    }
                }
                if self.engine == Engine::Mssql && at_line_start && word == "go" {
                    // GO must stand alone on its line (a count may follow).
                    let line_end = self.src[self.i..].find('\n').map(|n| self.i + n).unwrap_or(self.bytes.len());
                    if self.src[self.i..line_end].trim().chars().all(|ch| ch.is_ascii_digit()) {
                        self.i = line_end;
                        return Some((start, Tok::Go(line_end)));
                    }
                }
                return Some((start, Tok::Word(word)));
            }
            // Multi-byte UTF-8 or punctuation: step one whole char.
            let len = self.src[self.i..].chars().next().map(char::len_utf8).unwrap_or(1);
            self.i += len;
            return Some((start, Tok::Other));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn risks(engine: Engine, sql: &str) -> Vec<Risk> {
        analyze(engine, sql).into_iter().map(|s| s.risk).collect()
    }

    #[test]
    fn splits_on_top_level_semicolons_only() {
        let parts = split(
            Engine::Postgres,
            "select ';' as a; -- ; comment\nselect \"x;y\" from t /* ; */;\n\nselect 1",
        );
        assert_eq!(parts.len(), 3, "{parts:?}");
        assert_eq!(parts[0], "select ';' as a");
    }

    #[test]
    fn keeps_a_postgres_function_body_whole() {
        let sql = "create function f() returns int as $body$ begin; return 1; end; $body$ language plpgsql; select 1";
        let parts = split(Engine::Postgres, sql);
        assert_eq!(parts.len(), 2, "{parts:?}");
        assert!(parts[0].ends_with("plpgsql"));
    }

    #[test]
    fn mysql_backslash_escapes_do_not_end_a_string() {
        let parts = split(Engine::Mysql, r"insert into t values ('it\'s; fine'); select 1");
        assert_eq!(parts.len(), 2, "{parts:?}");
    }

    #[test]
    fn sql_server_go_lines_split_batches() {
        let parts = split(Engine::Mssql, "select 1\nGO\nselect [go;] from t; select 2\ngo 2\nselect 3");
        assert_eq!(parts, vec!["select 1", "select [go;] from t; select 2", "select 3"]);
        assert!(matches!(classify_batch(Engine::Mssql, "select 1; drop table t"), Risk::Danger(_)));
    }

    #[test]
    fn mysql_procedure_bodies_stay_whole() {
        let sql = "create procedure p() begin\n  if 1 then select 1; end if;\n  select case when 1 then 2 end;\nend;\nselect 9";
        let parts = split(Engine::Mysql, sql);
        assert_eq!(parts.len(), 2, "{parts:?}");
        assert!(parts[0].ends_with("end"));
    }

    #[test]
    fn mysql_delimiter_lines_are_honoured() {
        let sql = "DELIMITER //\ncreate trigger t before insert on x for each row begin set new.a = 1; end//\nDELIMITER ;\nselect 1;";
        let parts = split(Engine::Mysql, sql);
        assert_eq!(parts.len(), 2, "{parts:?}");
        assert!(parts[0].starts_with("create trigger"));
        assert_eq!(parts[1], "select 1");
    }

    #[test]
    fn sqlite_trigger_bodies_stay_whole() {
        let sql = "create trigger t after insert on a begin insert into b values (1); update c set d = 1; end; select 1";
        assert_eq!(split(Engine::Sqlite, sql).len(), 2);
    }

    #[test]
    fn a_plain_begin_is_still_its_own_statement() {
        assert_eq!(split(Engine::Mysql, "begin; update t set a = 1 where b = 2; commit"), vec!["begin", "update t set a = 1 where b = 2", "commit"]);
    }

    #[test]
    fn offsets_are_utf16_positions() {
        let st = analyze(Engine::Postgres, "select 'é😀';  select 2");
        assert_eq!(st[1].from, "select 'é😀';  ".encode_utf16().count());
        assert_eq!(st[0].to, "select 'é😀'".encode_utf16().count());
    }

    #[test]
    fn a_comment_only_script_is_empty() {
        assert!(split(Engine::Postgres, "-- nothing\n/* here */ ;").is_empty());
    }

    #[test]
    fn reads_are_reads() {
        for sql in ["select * from t", "SHOW TABLES", "explain select 1", "with x as (select 1) select * from x", "pragma table_info(t)"] {
            assert_eq!(classify(Engine::Sqlite, sql), Risk::Read, "{sql}");
        }
    }

    #[test]
    fn writes_hidden_in_a_cte_or_explain_analyze_are_caught() {
        assert_eq!(
            classify(Engine::Postgres, "with d as (delete from t returning *) select * from d"),
            Risk::Write
        );
        assert!(matches!(classify(Engine::Postgres, "explain analyze delete from t"), Risk::Danger(_)));
        assert_eq!(classify(Engine::Postgres, "select * into copy from t"), Risk::Write);
    }

    #[test]
    fn missing_where_is_dangerous_but_a_subquery_where_does_not_count() {
        assert!(matches!(classify(Engine::Mysql, "delete from t"), Risk::Danger(_)));
        assert!(matches!(
            classify(Engine::Mysql, "update t set a = (select b from u where u.id = 1)"),
            Risk::Danger(_)
        ));
        assert_eq!(classify(Engine::Mysql, "update t set a = 1 where id = 2"), Risk::Write);
    }

    #[test]
    fn drops_and_truncates_are_dangerous() {
        assert_eq!(
            risks(Engine::Postgres, "drop table t; truncate u; alter table v drop column c; alter table v add column d int"),
            vec![
                Risk::Danger("DROP TABLE".into()),
                Risk::Danger("TRUNCATE empties the whole table".into()),
                Risk::Danger("ALTER … DROP removes a column, constraint or index".into()),
                Risk::Write,
            ]
        );
    }

    #[test]
    fn a_keyword_inside_a_string_does_not_count() {
        assert_eq!(classify(Engine::Postgres, "select 'drop table x; delete from y'"), Risk::Read);
        assert_eq!(classify(Engine::Postgres, "delete from t where note = 'no where here'"), Risk::Write);
    }

    #[test]
    fn transactions_are_session_plumbing() {
        assert_eq!(classify(Engine::Postgres, "BEGIN"), Risk::Session);
        assert_eq!(classify(Engine::Mysql, "use shop"), Risk::Session);
    }
}
