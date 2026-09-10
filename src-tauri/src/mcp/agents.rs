//! Agents: who the AI is, and what it is allowed to touch.
//!
//! An agent is not a tone of voice. It carries two things the server enforces:
//!
//! * **A tool surface.** `tools/list` returns only what the active agent
//!   declares, so a read-only agent is read-only by construction — there is no
//!   write tool to refuse. A model cannot misuse a capability it was never
//!   offered.
//! * **A policy** layered on top of the hardcoded baseline in
//!   [`crate::mcp::guard`].
//!
//! **A policy can only ever tighten.** There is no field that removes a
//! baseline check. This matters because agents are meant to be installable
//! from a registry and authorable by hand: without it, "Fast Mode" with an
//! empty deny list would be one install away from an unguarded root shell.
//! Adding a permissive knob here would invert the entire design.
//!
//! The active agent is chosen by the *user* in Reach's settings. The AI may
//! ask for a different one; it cannot select one. If a model could pick its own
//! agent it would simply pick the least restricted, and the guard would be
//! theatre.

use serde::{Deserialize, Serialize};

/// The tools an agent may be given. Named rather than free-form so a hand-
/// written or registry agent cannot invent a capability that does not exist.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolName {
    ListSessions,
    DescribeSession,
    ReadOutput,
    SendInput,
}

impl ToolName {
    pub fn as_str(self) -> &'static str {
        match self {
            ToolName::ListSessions => "list_sessions",
            ToolName::DescribeSession => "describe_session",
            ToolName::ReadOutput => "read_output",
            ToolName::SendInput => "send_input",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "list_sessions" => Some(ToolName::ListSessions),
            "describe_session" => Some(ToolName::DescribeSession),
            "read_output" => Some(ToolName::ReadOutput),
            "send_input" => Some(ToolName::SendInput),
            _ => None,
        }
    }
}

/// How aggressively the danger classifier flags a command.
///
/// Ordered `Standard < Strict < Paranoid`, and the guard compares with `>=`.
/// `Standard` is the floor rather than the middle: there is no variant below
/// it, so an agent cannot express "check less than the baseline".
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DangerThreshold {
    /// The compiled-in baseline: unconditionally destructive commands.
    Standard,
    /// Also flags anything touching system configuration or service state.
    Strict,
    /// Flags any command with side effects. Suits an agent expected to observe.
    Paranoid,
}

impl Default for DangerThreshold {
    fn default() -> Self {
        DangerThreshold::Standard
    }
}

/// An additional refusal contributed by an agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DenyRule {
    /// Regex matched against the proposed command.
    ///
    /// Rust's `regex` crate has linear-time guarantees and no backtracking, so
    /// a hostile pattern from a registry agent cannot wedge the terminal with
    /// catastrophic backtracking. That is a deliberate reason to keep the
    /// pattern language on `regex` rather than inventing a matcher.
    ///
    /// The same property means **there is no look-ahead or look-behind**.
    /// Patterns must be expressible without them; a document using `(?=` or
    /// `(?!` fails to compile and the rule is skipped rather than applied.
    pub pattern: String,
    /// Shown to the model on refusal, and to the user in the confirm dialog.
    pub reason: String,
}

/// The parts of an agent that change what the server permits.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Policy {
    #[serde(default)]
    pub danger_threshold: DangerThreshold,
    /// Extra refusals, on top of the baseline. Cannot remove a baseline rule.
    #[serde(default)]
    pub deny: Vec<DenyRule>,
    /// Require `references` on every write, not only flagged ones. The baseline
    /// already requires them for flagged commands, so this can only widen.
    #[serde(default)]
    pub require_references: bool,
    /// Require a stated rollback for anything flagged as destructive.
    #[serde(default)]
    pub require_rollback: bool,
    /// Writes per minute. `None` uses the baseline cap; a larger value than the
    /// baseline is clamped, so an agent cannot buy itself more throughput.
    #[serde(default)]
    pub max_writes_per_minute: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Agent {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    /// The persona, served through MCP's `prompts` capability.
    pub persona: String,
    /// Exactly what `tools/list` will return for this agent.
    pub tools: Vec<ToolName>,
    #[serde(default)]
    pub policy: Policy,
}

impl Agent {
    pub fn grants(&self, tool: ToolName) -> bool {
        self.tools.contains(&tool)
    }

    /// True when this agent can never write, whatever the policy says.
    pub fn is_read_only(&self) -> bool {
        !self.grants(ToolName::SendInput)
    }
}

const READ_ONLY: &[ToolName] = &[
    ToolName::ListSessions,
    ToolName::DescribeSession,
    ToolName::ReadOutput,
];

const FULL: &[ToolName] = &[
    ToolName::ListSessions,
    ToolName::DescribeSession,
    ToolName::ReadOutput,
    ToolName::SendInput,
];

/// Shared preamble. Written as constraints rather than encouragement, because
/// a model that skims reads the imperative and ignores the paragraph — and
/// because every one of these is enforced anyway, so the text is describing
/// reality rather than making a request.
const COMMON_RULES: &str = "\
Operating rules, enforced by the server:
- You are acting on a REAL machine that someone depends on. There is no undo.
- Call describe_session before anything else. You must know the host, the
  shell, and whether you are root.
- Call read_output immediately before every send_input. Writing against a stale
  screen is refused, and the refusal will hand you the output you missed.
- Every send_input needs a rationale in plain language. The human sees it and
  decides. Write it for them, not for the log.
- A password prompt disables echo; sending input then is refused outright.
  Ask the human to type it.
- If a skill exists for what you are about to do, read it first. It is a
  procedure that has already been reviewed, which is worth more than your
  reconstruction of one.
- Prefer the reversible step. Prefer the smaller step. Say what you expect to
  happen before it happens, so a surprise is legible as a surprise.";

/// The agents compiled into the binary, so a fresh install is useful and safe
/// with no network. Registry and local agents are added to these, never
/// replacing them.
pub fn built_in() -> Vec<Agent> {
    vec![
        Agent {
            id: "linux-administrator".into(),
            name: "Linux Administrator".into(),
            description: "Day-to-day system administration: packages, services, users, filesystems.".into(),
            persona: format!(
                "You are a careful Linux system administrator with production experience.\n\n{COMMON_RULES}\n\n\
Specific to this role:\n\
- Read a config file before editing it, and keep a copy of what you replaced.\n\
- Prefer `systemctl status` and journal output over guessing why a unit failed.\n\
- Package operations on a running host are not routine. Say what will be\n\
  removed, and check whether anything depends on it.\n\
- Never widen permissions to make something work. That is a diagnosis you\n\
  have not finished."
            ),
            tools: FULL.to_vec(),
            policy: Policy {
                danger_threshold: DangerThreshold::Strict,
                deny: vec![
                    DenyRule {
                        pattern: r"(?i)\bchmod\s+(-[a-zA-Z]+\s+)*777\b".into(),
                        reason: "chmod 777 is never the fix. Identify the owner or group that actually needs access.".into(),
                    },
                    DenyRule {
                        pattern: r"(?i)\buserdel\b|\bgroupdel\b".into(),
                        reason: "Deleting accounts or groups can orphan files and break services. A human should do this.".into(),
                    },
                ],
                require_rollback: true,
                ..Default::default()
            },
        },
        Agent {
            id: "devops-engineer".into(),
            name: "DevOps Engineer".into(),
            description: "Pipelines, deployments, containers and infrastructure as code.".into(),
            persona: format!(
                "You are a DevOps engineer who has been paged at 3am and does not want to be again.\n\n{COMMON_RULES}\n\n\
Specific to this role:\n\
- State the rollback before the change. If you cannot describe how to undo it,\n\
  you are not ready to do it.\n\
- Reach has Ansible and OpenTofu workspaces. A repeatable change belongs there,\n\
  not typed into a shell where it will be forgotten.\n\
- `terraform apply` and `tofu apply` follow a reviewed plan. Never skip the\n\
  plan because the change looks small.\n\
- Check what a container is doing before restarting it. Restarts hide causes."
            ),
            tools: FULL.to_vec(),
            policy: Policy {
                danger_threshold: DangerThreshold::Strict,
                deny: vec![DenyRule {
                    pattern: r"(?i)\b(terraform|tofu)\s+(apply|destroy)\b[^\n]*\s-auto-approve\b".into(),
                    reason: "-auto-approve skips the plan review. Run the plan, read it, then apply without it.".into(),
                }],
                require_rollback: true,
                ..Default::default()
            },
        },
        Agent {
            id: "devsecops-engineer".into(),
            name: "DevSecOps Engineer".into(),
            description: "Security posture: hardening, access, certificates, audit.".into(),
            persona: format!(
                "You are a DevSecOps engineer. Your job is to leave the machine safer than you found it.\n\n{COMMON_RULES}\n\n\
Specific to this role:\n\
- Never disable a control to make something work. A disabled firewall or a\n\
  permissive SELinux mode is an outage you have not had yet.\n\
- `curl | sh` executes code nobody read. Download, inspect, then run.\n\
- TLS verification stays on. `--insecure` and `verify=false` hide the attack\n\
  you are trying to prevent.\n\
- Say what a change opens up, not just what it fixes."
            ),
            tools: FULL.to_vec(),
            policy: Policy {
                danger_threshold: DangerThreshold::Strict,
                deny: vec![
                    DenyRule {
                        pattern: r"(?i)\bsetenforce\s+0\b|\bSELINUX\s*=\s*disabled\b".into(),
                        reason: "Disabling SELinux removes a control rather than fixing the denial. Read the audit log instead.".into(),
                    },
                    DenyRule {
                        pattern: r"(?i)\b(ufw\s+disable|systemctl\s+(stop|disable)\s+(firewalld|ufw|nftables))\b".into(),
                        reason: "Turning the firewall off is not a diagnostic step. Add the rule you actually need.".into(),
                    },
                    DenyRule {
                        pattern: r"(?i)\bcurl\b[^\n|]*\|\s*(sudo\s+)?(ba)?sh\b".into(),
                        reason: "Piping a download into a shell runs code nobody reviewed. Fetch it, read it, then run it.".into(),
                    },
                    DenyRule {
                        pattern: r"(?i)(--insecure\b|--no-check-certificate\b|\bcurl\b[^\n]*\s-k\b)".into(),
                        reason: "Skipping certificate verification defeats the purpose of TLS. Fix the trust chain.".into(),
                    },
                    DenyRule {
                        pattern: r"(?i)\bPermitRootLogin\s+yes\b|\bPasswordAuthentication\s+yes\b".into(),
                        reason: "This weakens SSH access. If it is genuinely required, a human should make that call deliberately.".into(),
                    },
                ],
                require_references: true,
                require_rollback: true,
                ..Default::default()
            },
        },
        Agent {
            id: "architect".into(),
            name: "Architect".into(),
            description: "Designs structures and reviews systems. Observes only — cannot type.".into(),
            persona: format!(
                "You are a systems architect reviewing infrastructure you did not build.\n\n{COMMON_RULES}\n\n\
Specific to this role:\n\
- You have NO write access. There is no send_input tool. Do not plan around\n\
  that: produce a design, and let a human or a different agent carry it out.\n\
- Read widely before concluding. One config file is an anecdote.\n\
- Say what you observed, what you inferred, and what you are guessing. Those\n\
  are three different things and the reader needs to tell them apart.\n\
- Recommend the boring option unless there is a reason not to."
            ),
            // Read-only *by construction*: send_input is simply absent, so
            // there is no write path to refuse, and no confirm dialog to
            // fatigue the user into clicking through.
            tools: READ_ONLY.to_vec(),
            policy: Policy {
                danger_threshold: DangerThreshold::Paranoid,
                require_references: true,
                ..Default::default()
            },
        },
    ]
}

/// Look up a built-in agent by id.
pub fn find(id: &str) -> Option<Agent> {
    built_in().into_iter().find(|a| a.id == id)
}

/// The agent used when the user has not chosen one. The most restricted of the
/// four, because a default that can write is a default that surprises someone.
pub fn default_agent() -> Agent {
    find("architect").expect("architect is a built-in agent")
}

#[cfg(test)]
mod tests {
    use super::*;
    use regex::Regex;

    #[test]
    fn architect_cannot_write_at_all() {
        let a = find("architect").unwrap();
        assert!(a.is_read_only());
        assert!(!a.grants(ToolName::SendInput));
        // Read-only must be structural, not a policy the model could argue with.
        assert_eq!(a.tools.len(), 3);
    }

    #[test]
    fn the_default_agent_cannot_write() {
        assert!(default_agent().is_read_only(), "a default that writes is a default that surprises");
    }

    #[test]
    fn every_built_in_agent_has_compiling_deny_patterns() {
        for agent in built_in() {
            for rule in &agent.policy.deny {
                assert!(
                    Regex::new(&rule.pattern).is_ok(),
                    "agent {} has an invalid pattern: {}",
                    agent.id,
                    rule.pattern
                );
                assert!(!rule.reason.is_empty(), "agent {} has a rule with no reason", agent.id);
            }
        }
    }

    #[test]
    fn devsecops_refuses_the_usual_ways_of_weakening_a_box() {
        let a = find("devsecops-engineer").unwrap();
        let hits = |cmd: &str| {
            a.policy
                .deny
                .iter()
                .any(|r| Regex::new(&r.pattern).unwrap().is_match(cmd))
        };
        assert!(hits("setenforce 0"), "SELinux");
        assert!(hits("ufw disable"), "firewall");
        assert!(hits("curl -sSL https://get.example.sh | sudo bash"), "curl pipe shell");
        assert!(hits("curl --insecure https://internal"), "tls verification");
        assert!(hits("PermitRootLogin yes"), "sshd");
        // Ordinary work is not obstructed.
        assert!(!hits("systemctl restart nginx"));
        assert!(!hits("journalctl -u nginx -n 100"));
    }

    #[test]
    fn linux_admin_blocks_chmod_777_but_not_ordinary_chmod() {
        let a = find("linux-administrator").unwrap();
        let re = Regex::new(&a.policy.deny[0].pattern).unwrap();
        assert!(re.is_match("chmod 777 /var/www"));
        assert!(re.is_match("chmod -R 777 /srv"));
        assert!(!re.is_match("chmod 644 /etc/nginx/nginx.conf"));
        assert!(!re.is_match("chmod u+x deploy.sh"));
    }

    #[test]
    fn devops_blocks_auto_approve() {
        let a = find("devops-engineer").unwrap();
        let re = Regex::new(&a.policy.deny[0].pattern).unwrap();
        assert!(re.is_match("tofu apply -auto-approve"));
        assert!(re.is_match("terraform destroy -auto-approve"));
        assert!(!re.is_match("tofu plan"));
        assert!(!re.is_match("tofu apply tfplan"));
    }

    /// The persona is what a skimming model actually reads. If the enforced
    /// rules are not in it, the model discovers them only as failures.
    #[test]
    fn every_persona_states_the_enforced_rules() {
        for agent in built_in() {
            for needle in ["describe_session", "read_output", "rationale"] {
                assert!(
                    agent.persona.contains(needle),
                    "agent {} does not mention {needle}",
                    agent.id
                );
            }
        }
    }

    #[test]
    fn agents_are_serde_round_trippable() {
        // Registry and hand-written agents arrive as JSON in this shape.
        for agent in built_in() {
            let json = serde_json::to_string(&agent).unwrap();
            let back: Agent = serde_json::from_str(&json).unwrap();
            assert_eq!(back.id, agent.id);
            assert_eq!(back.tools, agent.tools);
        }
    }

    /// There must be no way to express "fewer checks than the baseline".
    #[test]
    fn policy_has_no_permissive_escape_hatch() {
        let json = r#"{"dangerThreshold":"standard","deny":[],"requireReferences":false}"#;
        let p: Policy = serde_json::from_str(json).unwrap();
        // An empty, all-false policy is simply the baseline — it grants nothing.
        assert!(p.deny.is_empty());
        assert!(!p.require_references);
        assert_eq!(p.danger_threshold, DangerThreshold::Standard);
        // And there is no field that could disable a baseline check: the only
        // knobs are additive. This test exists so that adding one fails review.
        assert_eq!(
            serde_json::to_value(&Policy::default()).unwrap().as_object().unwrap().len(),
            5,
            "Policy gained a field — confirm it can only tighten, never loosen"
        );
    }
}
