//! The recipe file format, and the parser for it.
//!
//! A recipe is a bash script that carries its own metadata in leading `#`
//! comments. That shape is deliberate: the file stays a valid, runnable,
//! lintable bash script outside Reach. A format only this app can execute is
//! a format nobody can audit, and a recipe is arbitrary code aimed at
//! someone's production server — being readable by `cat` matters more than
//! being elegant.
//!
//! ```bash
//! #!/usr/bin/env bash
//! # @reach-recipe
//! # id: harden-ssh
//! # name: Harden the SSH daemon
//! # description: Disables root login and password authentication.
//! # version: 1.0.0
//! # author: someone
//! # tags: security, ssh
//! # targets: debian, ubuntu
//! # danger: sensitive
//! # param: SSH_PORT | Port sshd should listen on | 22
//! # param: ADMIN_USER | Account that keeps shell access |
//! # @end
//! set -euo pipefail
//! ...
//! ```
//!
//! Everything after `@end` is the script, verbatim. Everything between the
//! markers is metadata. A file without `@reach-recipe` is not a recipe.

use serde::{Deserialize, Serialize};

use crate::mcp::guard::Danger;

/// One parameter a recipe accepts, surfaced as a form field and delivered to
/// the script as an environment variable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecipeParam {
    /// Environment variable name. Validated, because this string ends up in a
    /// shell assignment and a lax one would be an injection point.
    pub name: String,
    /// Shown next to the field. Falls back to the name.
    pub label: String,
    /// Prefilled value. An empty default makes the field required.
    pub default: String,
}

impl RecipeParam {
    /// A parameter with no default has to be supplied before the run.
    pub fn required(&self) -> bool {
        self.default.is_empty()
    }
}

/// Where a recipe came from, which is the same question as how much to trust it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum Origin {
    /// Written here. Nobody else has touched it.
    Local,
    /// Installed from a registry, pinned to the hash that was verified.
    Registry { repo: String, sha256: String },
}

impl Default for Origin {
    fn default() -> Self {
        Origin::Local
    }
}

/// A parsed recipe.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Recipe {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub tags: Vec<String>,
    /// Distros or platforms the author says this targets. Advisory only —
    /// nothing checks it, because guessing wrong and refusing to run would be
    /// worse than letting someone decide for themselves.
    #[serde(default)]
    pub targets: Vec<String>,
    /// What the author says the risk is. Compared against what analysis finds,
    /// and a recipe claiming to be harmless while deleting things is a signal
    /// worth showing.
    #[serde(default)]
    pub declared_danger: Option<Danger>,
    #[serde(default)]
    pub params: Vec<RecipeParam>,
    /// The body, verbatim, with no metadata.
    pub script: String,
    /// The whole file as written, so the editor round-trips exactly.
    pub source: String,
    #[serde(default)]
    pub origin: Origin,
}

/// Why a file is not a usable recipe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    NotARecipe,
    MissingField(&'static str),
    BadId(String),
    BadParamName(String),
    BadParamLine(String),
    UnknownDanger(String),
    NoEnd,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::NotARecipe => {
                write!(f, "Not a recipe: the file has no `# @reach-recipe` marker")
            }
            ParseError::MissingField(field) => write!(f, "Recipe is missing `{field}`"),
            ParseError::BadId(id) => write!(
                f,
                "Invalid id `{id}`: use lowercase letters, digits and hyphens"
            ),
            ParseError::BadParamName(name) => write!(
                f,
                "Invalid parameter name `{name}`: it becomes a shell variable, so it must match [A-Z_][A-Z0-9_]*"
            ),
            ParseError::BadParamLine(line) => {
                write!(f, "Malformed param line: `{line}` (expected NAME | label | default)")
            }
            ParseError::UnknownDanger(word) => write!(
                f,
                "Unknown danger `{word}`: expected benign, mutating, sensitive or destructive"
            ),
            ParseError::NoEnd => write!(f, "Recipe header is never closed with `# @end`"),
        }
    }
}

const MARKER: &str = "@reach-recipe";
const END: &str = "@end";

/// Ids become filenames, so they are kept to a shape that cannot escape a
/// directory or collide case-insensitively on Windows.
pub fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && !id.starts_with('-')
        && !id.ends_with('-')
}

/// Parameter names are interpolated into a shell assignment, so they are held
/// to the shell's own identifier rules rather than trusted.
pub fn valid_param_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_uppercase() || c == '_')
        && name
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
}

fn split_list(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

fn parse_danger(word: &str) -> Result<Danger, ParseError> {
    match word.trim().to_ascii_lowercase().as_str() {
        "benign" => Ok(Danger::Benign),
        "mutating" => Ok(Danger::Mutating),
        "sensitive" => Ok(Danger::Sensitive),
        "destructive" => Ok(Danger::Destructive),
        other => Err(ParseError::UnknownDanger(other.to_string())),
    }
}

/// Parse a recipe file.
pub fn parse(source: &str) -> Result<Recipe, ParseError> {
    if !source.contains(MARKER) {
        return Err(ParseError::NotARecipe);
    }

    let mut id = String::new();
    let mut name = String::new();
    let mut description = String::new();
    let mut version = String::new();
    let mut author = String::new();
    let mut tags = Vec::new();
    let mut targets = Vec::new();
    let mut declared_danger = None;
    let mut params = Vec::new();

    let mut in_header = false;
    let mut ended = false;
    let mut body_start = 0usize;

    // Line endings are normalised for parsing only; `source` keeps whatever
    // the author wrote so the editor round-trips byte for byte.
    let mut offset = 0usize;
    for raw_line in source.split_inclusive('\n') {
        let line_len = raw_line.len();
        let line = raw_line.trim_end_matches(['\n', '\r']);
        offset += line_len;

        let trimmed = line.trim_start();
        if !trimmed.starts_with('#') {
            // A non-comment line before the header closes means the header was
            // never closed. Shebangs are comments, so they fall through fine.
            if in_header {
                return Err(ParseError::NoEnd);
            }
            continue;
        }

        let content = trimmed.trim_start_matches('#').trim();

        if content == MARKER {
            in_header = true;
            continue;
        }
        if !in_header {
            continue;
        }
        if content == END {
            ended = true;
            body_start = offset;
            break;
        }

        let Some((key, value)) = content.split_once(':') else {
            continue;
        };
        let key = key.trim().to_ascii_lowercase();
        let value = value.trim();

        match key.as_str() {
            "id" => id = value.to_string(),
            "name" => name = value.to_string(),
            "description" => description = value.to_string(),
            "version" => version = value.to_string(),
            "author" => author = value.to_string(),
            "tags" => tags = split_list(value),
            "targets" => targets = split_list(value),
            "danger" => declared_danger = Some(parse_danger(value)?),
            "param" => {
                // NAME | label | default. The label and default are optional,
                // so a bare name is a required parameter labelled by its name.
                let mut parts = value.split('|').map(str::trim);
                let pname = parts.next().unwrap_or("").to_string();
                if !valid_param_name(&pname) {
                    return Err(if pname.is_empty() {
                        ParseError::BadParamLine(value.to_string())
                    } else {
                        ParseError::BadParamName(pname)
                    });
                }
                let label = parts.next().unwrap_or("").to_string();
                let default = parts.next().unwrap_or("").to_string();
                params.push(RecipeParam {
                    label: if label.is_empty() { pname.clone() } else { label },
                    name: pname,
                    default,
                });
            }
            _ => {}
        }
    }

    if !in_header {
        return Err(ParseError::NotARecipe);
    }
    if !ended {
        return Err(ParseError::NoEnd);
    }
    if id.is_empty() {
        return Err(ParseError::MissingField("id"));
    }
    if !valid_id(&id) {
        return Err(ParseError::BadId(id));
    }
    if name.is_empty() {
        return Err(ParseError::MissingField("name"));
    }

    Ok(Recipe {
        id,
        name,
        description,
        version,
        author,
        tags,
        targets,
        declared_danger,
        params,
        script: source[body_start..].to_string(),
        source: source.to_string(),
        origin: Origin::Local,
    })
}

/// The starting point for a new recipe, so nobody has to remember the header
/// shape from memory.
pub fn template(id: &str, name: &str) -> String {
    format!(
        "#!/usr/bin/env bash\n\
         # @reach-recipe\n\
         # id: {id}\n\
         # name: {name}\n\
         # description: What this does, in one line.\n\
         # version: 0.1.0\n\
         # tags: \n\
         # targets: debian, ubuntu\n\
         # danger: mutating\n\
         # param: EXAMPLE | An example parameter | default-value\n\
         # @end\n\
         \n\
         # Runs with `bash -s`, so a failure anywhere stops the whole recipe.\n\
         set -euo pipefail\n\
         \n\
         echo \"Example is: $EXAMPLE\"\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOOD: &str = "#!/usr/bin/env bash\n\
        # @reach-recipe\n\
        # id: harden-ssh\n\
        # name: Harden SSH\n\
        # description: Locks down sshd.\n\
        # version: 1.2.0\n\
        # author: someone\n\
        # tags: security, ssh\n\
        # targets: debian, ubuntu\n\
        # danger: sensitive\n\
        # param: SSH_PORT | Port | 22\n\
        # param: ADMIN_USER | Admin account |\n\
        # @end\n\
        set -euo pipefail\n\
        echo hi\n";

    #[test]
    fn parses_every_field() {
        let r = parse(GOOD).expect("should parse");
        assert_eq!(r.id, "harden-ssh");
        assert_eq!(r.name, "Harden SSH");
        assert_eq!(r.description, "Locks down sshd.");
        assert_eq!(r.version, "1.2.0");
        assert_eq!(r.author, "someone");
        assert_eq!(r.tags, vec!["security", "ssh"]);
        assert_eq!(r.targets, vec!["debian", "ubuntu"]);
        assert_eq!(r.declared_danger, Some(Danger::Sensitive));
        assert_eq!(r.params.len(), 2);
    }

    #[test]
    fn body_excludes_the_header() {
        let r = parse(GOOD).unwrap();
        assert_eq!(r.script, "set -euo pipefail\necho hi\n");
        assert!(!r.script.contains("@reach-recipe"));
        assert!(!r.script.contains("id:"));
    }

    #[test]
    fn source_round_trips_exactly() {
        // The editor shows `source`; losing a byte here would silently rewrite
        // someone's file every time they opened it.
        assert_eq!(parse(GOOD).unwrap().source, GOOD);
    }

    #[test]
    fn a_param_without_a_default_is_required() {
        let r = parse(GOOD).unwrap();
        let port = r.params.iter().find(|p| p.name == "SSH_PORT").unwrap();
        let admin = r.params.iter().find(|p| p.name == "ADMIN_USER").unwrap();
        assert!(!port.required(), "22 is a default");
        assert!(admin.required(), "no default means it must be supplied");
    }

    #[test]
    fn a_bare_param_is_labelled_by_its_name() {
        let src = "# @reach-recipe\n# id: x\n# name: X\n# param: TOKEN\n# @end\nbody\n";
        let r = parse(src).unwrap();
        assert_eq!(r.params[0].name, "TOKEN");
        assert_eq!(r.params[0].label, "TOKEN");
        assert!(r.params[0].required());
    }

    #[test]
    fn rejects_a_file_that_is_not_a_recipe() {
        assert_eq!(parse("echo hello\n").unwrap_err(), ParseError::NotARecipe);
    }

    #[test]
    fn rejects_an_unclosed_header() {
        let src = "# @reach-recipe\n# id: x\n# name: X\necho hi\n";
        assert_eq!(parse(src).unwrap_err(), ParseError::NoEnd);
    }

    #[test]
    fn requires_id_and_name() {
        let no_id = "# @reach-recipe\n# name: X\n# @end\nbody\n";
        assert_eq!(parse(no_id).unwrap_err(), ParseError::MissingField("id"));
        let no_name = "# @reach-recipe\n# id: x\n# @end\nbody\n";
        assert_eq!(parse(no_name).unwrap_err(), ParseError::MissingField("name"));
    }

    #[test]
    fn rejects_an_id_that_could_escape_the_recipes_directory() {
        for bad in ["../evil", "a/b", "UPPER", "trailing-", "-leading", "has space"] {
            let src = format!("# @reach-recipe\n# id: {bad}\n# name: X\n# @end\nbody\n");
            assert!(
                matches!(parse(&src), Err(ParseError::BadId(_))),
                "id `{bad}` must be rejected"
            );
        }
    }

    #[test]
    fn rejects_a_param_name_that_is_not_a_shell_identifier() {
        // This string is interpolated into a shell assignment. A permissive
        // rule here is a command injection.
        for bad in ["lower", "HAS-DASH", "1LEADING", "X;rm -rf /", "HAS SPACE"] {
            let src = format!("# @reach-recipe\n# id: x\n# name: X\n# param: {bad} | l | d\n# @end\nb\n");
            assert!(
                matches!(parse(&src), Err(ParseError::BadParamName(_)) | Err(ParseError::BadParamLine(_))),
                "param name `{bad}` must be rejected"
            );
        }
    }

    #[test]
    fn rejects_an_unknown_danger_word() {
        let src = "# @reach-recipe\n# id: x\n# name: X\n# danger: spicy\n# @end\nb\n";
        assert!(matches!(parse(src), Err(ParseError::UnknownDanger(_))));
    }

    #[test]
    fn handles_crlf() {
        let src = GOOD.replace('\n', "\r\n");
        let r = parse(&src).expect("CRLF files are still recipes");
        assert_eq!(r.id, "harden-ssh");
        assert!(r.script.starts_with("set -euo pipefail"));
    }

    #[test]
    fn the_template_parses() {
        // A template that does not parse would greet every new author with an
        // error on a file they have not touched.
        let r = parse(&template("my-recipe", "My Recipe")).expect("template must parse");
        assert_eq!(r.id, "my-recipe");
        assert_eq!(r.name, "My Recipe");
    }
}
