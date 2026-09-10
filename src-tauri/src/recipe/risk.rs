//! What a recipe will do to a machine, read off the script before it runs.
//!
//! This borrows the MCP guard's classifier rather than growing its own. Two
//! lists of destructive patterns drift, and the one that drifts is the one
//! that stops recognising `rm -rf /`.
//!
//! Be clear about what this is worth: it is pattern matching over shell text,
//! and shell text has unlimited ways to hide. `eval "$(curl …)"` is opaque,
//! and a variable can hold anything. So this is a seatbelt, not a guarantee.
//! What actually decides whether a registry recipe is safe is the person who
//! merged the pull request — the same trust boundary the plugin marketplace
//! has. The analysis exists so an obvious hazard is impossible to miss, not so
//! that a subtle one becomes impossible to write.

use serde::Serialize;

use crate::mcp::agents::DangerThreshold;
use crate::mcp::guard::{danger_of, Danger};

use super::schema::Recipe;

/// One line worth warning about.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Finding {
    /// 1-indexed, counted against the script body — which is what the editor
    /// shows below the header, so the number lands on the right line.
    pub line: u32,
    pub text: String,
    pub danger: Danger,
    pub reason: String,
}

/// The verdict on a whole recipe.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Analysis {
    /// The level Reach acts on: the worse of what the author declared and
    /// what the scan found. It is what the badge shows and what decides
    /// whether running needs a typed confirmation.
    ///
    /// An author who calls their recipe `sensitive` knows something the
    /// scanner does not — that `sed -i` on `"$CONFIG"` is editing sshd's
    /// config, say. Showing that recipe as "Benign" because the path was
    /// behind a variable would be the scanner overruling the one party that
    /// actually read the script.
    pub danger: Danger,
    /// What the scan alone found. `Benign` means nothing matched, not that
    /// it is safe.
    pub found: Danger,
    pub findings: Vec<Finding>,
    /// What the author claimed in the header, if anything.
    pub declared: Option<Danger>,
    /// True when analysis found something worse than the author admitted to.
    ///
    /// Not proof of bad faith — most of the time it is a stale header. But a
    /// recipe advertising itself as harmless while deleting things is exactly
    /// what someone should look at twice before running it as root.
    pub understated: bool,
    /// Lines that fetch code from the network and run it. Called out
    /// separately because no classifier can see what arrives.
    pub opaque: Vec<u32>,
}

/// Lines that pull something off the network and execute it. The content is
/// unknowable at analysis time, so the pattern itself is the warning.
fn is_opaque(line: &str) -> bool {
    let l = line.to_ascii_lowercase();
    let fetches = l.contains("curl ") || l.contains("wget ") || l.contains("fetch ");
    let pipes_to_shell = l.contains("| sh") || l.contains("| bash") || l.contains("|sh") || l.contains("|bash");
    let evaluated = l.contains("eval ") && fetches;
    (fetches && pipes_to_shell) || evaluated
}

/// Strip a trailing comment so `rm -rf /` inside a comment is not a finding.
///
/// Deliberately naive: it does not understand quoting, so a `#` inside a
/// string truncates the line early. That direction is the safe one — it can
/// cause a missed finding, never a false one, and a false alarm on every
/// second recipe is how people learn to click through warnings.
fn strip_comment(line: &str) -> &str {
    match line.find('#') {
        Some(0) => "",
        Some(i) => &line[..i],
        None => line,
    }
}

/// Analyse a recipe's script.
pub fn analyse(recipe: &Recipe) -> Analysis {
    let mut findings = Vec::new();
    let mut opaque = Vec::new();
    let mut worst = Danger::Benign;

    for (index, raw) in recipe.script.lines().enumerate() {
        let line_no = index as u32 + 1;
        let code = strip_comment(raw).trim();
        if code.is_empty() {
            continue;
        }

        if is_opaque(code) {
            opaque.push(line_no);
        }

        // Strict rather than Paranoid: Paranoid calls everything that is not a
        // known read "mutating", which for a recipe — a file that exists to
        // change things — would flag every line and mean nothing.
        let (danger, reason) = danger_of(code, DangerThreshold::Strict);
        if danger > Danger::Benign {
            if danger > worst {
                worst = danger;
            }
            findings.push(Finding {
                line: line_no,
                text: raw.trim().to_string(),
                danger,
                reason: reason.unwrap_or_else(|| "matched a risky pattern".into()),
            });
        }
    }

    let declared = recipe.declared_danger;
    let understated = declared.is_some_and(|d| worst > d);
    let danger = declared.map_or(worst, |d| d.max(worst));

    Analysis {
        danger,
        found: worst,
        findings,
        declared,
        understated,
        opaque,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recipe::schema::parse;

    fn recipe(script: &str, danger_line: &str) -> Recipe {
        let src = format!(
            "# @reach-recipe\n# id: t\n# name: T\n{danger_line}# @end\n{script}"
        );
        parse(&src).expect("test recipe should parse")
    }

    #[test]
    fn a_harmless_script_is_benign() {
        let a = analyse(&recipe("echo hello\nls -la\n", ""));
        assert_eq!(a.danger, Danger::Benign);
        assert!(a.findings.is_empty());
    }

    #[test]
    fn finds_a_destructive_line_and_points_at_it() {
        let a = analyse(&recipe("echo one\nrm -rf /\necho three\n", ""));
        assert_eq!(a.danger, Danger::Destructive);
        assert_eq!(a.findings.len(), 1);
        assert_eq!(a.findings[0].line, 2, "line number must match the editor");
        assert!(a.findings[0].text.contains("rm -rf /"));
    }

    #[test]
    fn reports_the_worst_finding_not_the_last() {
        let a = analyse(&recipe("rm -rf /\nsystemctl restart sshd\n", ""));
        assert_eq!(a.danger, Danger::Destructive);
    }

    #[test]
    fn ignores_a_commented_out_hazard() {
        // Otherwise every recipe that documents what not to do gets flagged.
        let a = analyse(&recipe("# rm -rf / would be bad\necho safe\n", ""));
        assert_eq!(a.danger, Danger::Benign, "a comment is not a command");
    }

    #[test]
    fn ignores_a_trailing_comment() {
        let a = analyse(&recipe("echo safe # not rm -rf /\n", ""));
        assert_eq!(a.danger, Danger::Benign);
    }

    #[test]
    fn flags_curl_piped_into_a_shell() {
        let a = analyse(&recipe("curl -fsSL https://example.com/i.sh | sh\n", ""));
        assert_eq!(a.opaque, vec![1], "nothing can classify what has not arrived yet");
    }

    #[test]
    fn flags_curl_piped_into_bash_without_a_space() {
        let a = analyse(&recipe("wget -qO- https://x/i.sh |bash\n", ""));
        assert_eq!(a.opaque, vec![1]);
    }

    #[test]
    fn a_plain_curl_is_not_opaque() {
        let a = analyse(&recipe("curl -o /tmp/file https://example.com/f\n", ""));
        assert!(a.opaque.is_empty(), "downloading is not executing");
    }

    #[test]
    fn notices_an_author_understating_the_risk() {
        let a = analyse(&recipe("rm -rf /\n", "# danger: benign\n"));
        assert!(a.understated, "claims benign, deletes everything");
        assert_eq!(a.declared, Some(Danger::Benign));
        assert_eq!(a.danger, Danger::Destructive);
    }

    #[test]
    fn an_honest_header_is_not_flagged() {
        let a = analyse(&recipe("rm -rf /\n", "# danger: destructive\n"));
        assert!(!a.understated);
    }

    #[test]
    fn overstating_the_risk_is_fine() {
        // Being more cautious than necessary is never a warning.
        let a = analyse(&recipe("echo hi\n", "# danger: destructive\n"));
        assert!(!a.understated);
    }

    #[test]
    fn the_declared_level_is_honoured_when_the_scan_sees_nothing() {
        // `sed -i` on a path held in a variable is invisible to the scanner.
        // The author said sensitive; that is the level people must see and
        // confirm against, not "Benign" because the path was indirect.
        let a = analyse(&recipe(
            "CONFIG=/etc/ssh/sshd_config\nsed -i 's/x/y/' \"$CONFIG\"\n",
            "# danger: sensitive\n",
        ));
        assert_eq!(a.found, Danger::Benign, "the scan cannot see through the variable");
        assert_eq!(a.danger, Danger::Sensitive, "the author's word must still count");
        assert!(!a.understated);
    }

    #[test]
    fn the_scan_wins_when_it_finds_worse() {
        let a = analyse(&recipe("rm -rf /\n", "# danger: mutating\n"));
        assert_eq!(a.found, Danger::Destructive);
        assert_eq!(a.danger, Danger::Destructive);
        assert!(a.understated);
    }

    #[test]
    fn with_no_declaration_the_scan_is_all_there_is() {
        let a = analyse(&recipe("echo hi\n", ""));
        assert_eq!(a.danger, a.found);
        assert_eq!(a.danger, Danger::Benign);
    }

    #[test]
    fn no_declaration_is_never_understated() {
        let a = analyse(&recipe("rm -rf /\n", ""));
        assert!(!a.understated, "nothing was claimed, so nothing was understated");
        assert_eq!(a.danger, Danger::Destructive);
    }
}
