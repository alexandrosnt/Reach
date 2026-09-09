//! The baseline. Every check here is compiled in and cannot be disabled by an
//! agent, a registry document, a hand-written file, or the model asking nicely.
//!
//! The premise is that some clients are careless. Tool descriptions are
//! advisory — a model that skims ignores them — so nothing here is expressed as
//! guidance. Each rule is a precondition that makes the call *fail*, and the
//! failure carries the remedy, because an error a model cannot act on just
//! becomes a retry loop.
//!
//! Order matters. Cheap structural refusals run before expensive ones, and the
//! echo-off check runs first because it is the only one protecting something
//! the user is typing at this instant.
//!
//! An agent's [`Policy`] is layered on top and may only tighten: extra deny
//! rules, a higher threshold, a *lower* rate cap.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use regex::Regex;

use crate::mcp::agents::{Agent, DangerThreshold, ToolName};
use crate::mcp::redact;

/// Writes per minute permitted by the baseline. An agent may lower this; a
/// higher value is clamped, so no agent can buy itself extra throughput.
const BASELINE_WRITES_PER_MINUTE: u32 = 12;

/// Longest single input accepted. A model that has decided to paste a script
/// into a shell should be writing a file instead, where the human can read it.
const MAX_INPUT_BYTES: usize = 2048;

/// Why a call was refused. Rendered to the model as an actionable message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// The active agent does not have this tool.
    ToolNotGranted { tool: &'static str, agent: String },
    /// `describe_session` has not been called for this session.
    SessionNotDescribed,
    /// The screen changed since the last read, or was never read.
    StaleView,
    /// Terminal echo is off — a password is being typed right now.
    EchoDisabled,
    /// No rationale, or one too thin to be worth showing a human.
    RationaleMissing,
    /// The agent or the command class requires cited references.
    ReferencesRequired { reason: String },
    /// A destructive command with no stated rollback.
    RollbackRequired,
    /// Baseline or agent deny rule.
    Denied { reason: String },
    /// The command itself carries a secret in the clear.
    SecretInCommand,
    /// Too many writes too quickly.
    RateLimited { retry_after_secs: u64 },
    /// Oversized input.
    TooLong { limit: usize },
    /// A skill covers this operation and has not been read.
    SkillUnread { skill: String },
}

impl Refusal {
    /// The message the model receives. Every one names the next action, because
    /// a refusal a model cannot act on turns into a retry loop.
    pub fn message(&self) -> String {
        match self {
            Refusal::ToolNotGranted { tool, agent } => format!(
                "The active agent ({agent}) does not have the `{tool}` tool. This is not a \
                 permission you can request — ask the user to switch agents in Reach if a \
                 different capability is genuinely needed."
            ),
            Refusal::SessionNotDescribed =>
                "Call `describe_session` for this session first. You must know the host, the \
                 shell and whether you are root before acting on it.".into(),
            Refusal::StaleView =>
                "The session has produced output since your last `read_output`. Call \
                 `read_output` again and re-read before sending anything — you are looking at \
                 a stale screen.".into(),
            Refusal::EchoDisabled =>
                "Terminal echo is off, which means a password or other secret is being typed \
                 right now. Input is refused. Ask the user to type it themselves.".into(),
            Refusal::RationaleMissing =>
                "`rationale` is required and must be a real sentence. A human reads it and \
                 decides whether to allow the command — write it for them.".into(),
            Refusal::ReferencesRequired { reason } => format!(
                "This command requires `references` (documentation, a man page, a runbook) \
                 because {reason}. Cite what you are basing this on."
            ),
            Refusal::RollbackRequired =>
                "This command is destructive and the active agent requires a `rollback`: state \
                 how to undo it. If you cannot describe the undo, you are not ready to run it.".into(),
            Refusal::Denied { reason } => format!("Refused: {reason}"),
            Refusal::SecretInCommand =>
                "This command contains what looks like a credential in the clear. It would be \
                 written to shell history and to Reach's log. Use a file, an environment \
                 variable, or ask the user to enter it.".into(),
            Refusal::RateLimited { retry_after_secs } => format!(
                "Too many commands too quickly. Wait {retry_after_secs}s. If you are looping, \
                 stop and read the output — something is not working the way you think."
            ),
            Refusal::TooLong { limit } => format!(
                "Input exceeds {limit} bytes. Do not paste a script into a shell: write it to a \
                 file so the user can read it before it runs."
            ),
            Refusal::SkillUnread { skill } => format!(
                "A reviewed procedure covers this operation. Read the resource `skill://{skill}` \
                 before continuing — it has already been checked, which is worth more than \
                 reconstructing it."
            ),
        }
    }
}

/// How dangerous a proposed command looks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Danger {
    /// No obvious side effects.
    Benign,
    /// Changes state, recoverable.
    Mutating,
    /// Changes system configuration or service availability.
    Sensitive,
    /// Data loss or an unbootable machine.
    Destructive,
}

struct Classified {
    danger: Danger,
    reason: Option<String>,
}

/// Commands that are destructive regardless of agent. This list is the floor;
/// nothing can remove an entry.
fn baseline_destructive() -> &'static Vec<(Regex, &'static str)> {
    static RULES: std::sync::OnceLock<Vec<(Regex, &'static str)>> = std::sync::OnceLock::new();
    RULES.get_or_init(|| {
        let specs: &[(&str, &str)] = &[
            (r"(?i)\brm\s+(-[a-zA-Z]*\s+)*-[a-zA-Z]*[rR][a-zA-Z]*[fF]|(?i)\brm\s+(-[a-zA-Z]*\s+)*-[a-zA-Z]*[fF][a-zA-Z]*[rR]",
             "recursive forced delete"),
            (r"(?i)\bmkfs(\.[a-z0-9]+)?\b", "formats a filesystem"),
            (r"(?i)\bdd\b[^\n]*\bof=/dev/", "writes directly to a block device"),
            (r"(?i)>\s*/dev/(sd|nvme|vd|hd)", "overwrites a disk"),
            (r"(?i)\bshred\b", "irreversibly destroys data"),
            (r"(?i):\(\)\s*\{\s*:\|:&\s*\}\s*;\s*:", "fork bomb"),
            (r"(?i)\bDROP\s+(DATABASE|TABLE)\b", "drops a database object"),
            (r"(?i)\bTRUNCATE\s+TABLE\b", "empties a table"),
            (r"(?i)\bgit\s+push\b[^\n]*(--force\b|-f\b)", "force push overwrites history"),
            (r"(?i)\bgit\s+reset\s+--hard\b", "discards uncommitted work"),
            (r"(?i)\b(halt|poweroff|shutdown|reboot)\b", "takes the machine down"),
            (r"(?i)\buserdel\b|\bdeluser\b", "removes a user account"),
            (r"(?i)\bvgremove\b|\blvremove\b|\bpvremove\b", "destroys LVM volumes"),
            (r"(?i)\bwipefs\b", "erases filesystem signatures"),
        ];
        specs
            .iter()
            .map(|(p, r)| (Regex::new(p).expect("baseline pattern must compile"), *r))
            .collect()
    })
}

/// Configuration and service changes — flagged at `Strict` and above.
fn sensitive_patterns() -> &'static Vec<(Regex, &'static str)> {
    static RULES: std::sync::OnceLock<Vec<(Regex, &'static str)>> = std::sync::OnceLock::new();
    RULES.get_or_init(|| {
        let specs: &[(&str, &str)] = &[
            (r"(?i)\bsystemctl\s+(stop|disable|mask|restart)\b", "changes service state"),
            (r"(?i)\b(apt|apt-get|yum|dnf|pacman|apk|zypper)\s+(remove|purge|erase|autoremove)\b",
             "removes packages"),
            (r"(?i)>\s*/etc/|\btee\s+/etc/|\b(vi|vim|nano|sed\s+-i)\b[^\n]*\s/etc/",
             "modifies system configuration"),
            (r"(?i)\biptables\b|\bnft\b|\bufw\s+(allow|deny|delete)\b", "changes firewall rules"),
            (r"(?i)\bchown\b|\bchmod\b", "changes ownership or permissions"),
            (r"(?i)\bcrontab\b|/etc/cron", "changes scheduled jobs"),
            // Reach's own configuration. Cannot escalate — the baseline is
            // compiled in and agents are user-selected — but the user should
            // still see that the AI is writing to Reach's directory.
            (r"(?i)\.reach/(profiles|agents|skills|plugins|themes)/", "modifies Reach's own configuration"),
        ];
        specs
            .iter()
            .map(|(p, r)| (Regex::new(p).expect("sensitive pattern must compile"), *r))
            .collect()
    })
}

fn classify(command: &str, threshold: DangerThreshold) -> Classified {
    for (re, reason) in baseline_destructive() {
        if re.is_match(command) {
            return Classified { danger: Danger::Destructive, reason: Some((*reason).into()) };
        }
    }

    if threshold >= DangerThreshold::Strict {
        for (re, reason) in sensitive_patterns() {
            if re.is_match(command) {
                return Classified { danger: Danger::Sensitive, reason: Some((*reason).into()) };
            }
        }
    }

    if threshold == DangerThreshold::Paranoid {
        // Anything that is not obviously a read is treated as mutating.
        static READ_ONLY: &[&str] = &[
            "ls", "cat", "less", "more", "head", "tail", "grep", "find", "stat", "df", "du",
            "ps", "top", "free", "uptime", "who", "id", "uname", "hostname", "date", "pwd",
            "systemctl status", "journalctl", "ip a", "ip r", "ss", "netstat", "which", "whereis",
        ];
        let trimmed = command.trim();
        if !READ_ONLY.iter().any(|p| trimmed.starts_with(p)) {
            return Classified { danger: Danger::Mutating, reason: Some("has side effects".into()) };
        }
    }

    Classified { danger: Danger::Benign, reason: None }
}

/// What the caller supplies with a write.
#[derive(Debug, Clone, Default)]
pub struct WriteRequest {
    pub command: String,
    pub rationale: String,
    pub references: Vec<String>,
    pub rollback: Option<String>,
}

/// Facts about the session at the moment of the call.
#[derive(Debug, Clone)]
pub struct SessionFacts {
    pub described: bool,
    /// The client has read output at or after the session's current sequence.
    pub view_current: bool,
    /// Terminal echo is disabled — a password is being typed.
    pub echo_off: bool,
    /// Skills whose triggers match, and which have not been read.
    pub unread_skills: Vec<String>,
}

/// Sliding window of recent writes, per connected client.
#[derive(Debug, Default)]
pub struct RateWindow {
    hits: VecDeque<Instant>,
}

impl RateWindow {
    fn check(&mut self, limit: u32) -> Result<(), Refusal> {
        let now = Instant::now();
        let minute = Duration::from_secs(60);
        while self.hits.front().is_some_and(|t| now.duration_since(*t) > minute) {
            self.hits.pop_front();
        }
        if self.hits.len() as u32 >= limit {
            let oldest = self.hits.front().copied().unwrap_or(now);
            let wait = minute.saturating_sub(now.duration_since(oldest));
            return Err(Refusal::RateLimited { retry_after_secs: wait.as_secs().max(1) });
        }
        self.hits.push_back(now);
        Ok(())
    }
}

/// What a permitted write carries forward into the confirm dialog.
#[derive(Debug, Clone)]
pub struct Approved {
    pub danger: Danger,
    pub danger_reason: Option<String>,
}

/// Can this agent call this tool at all?
pub fn check_tool(agent: &Agent, tool: ToolName) -> Result<(), Refusal> {
    if agent.grants(tool) {
        Ok(())
    } else {
        Err(Refusal::ToolNotGranted { tool: tool.as_str(), agent: agent.name.clone() })
    }
}

/// The full write gate. Returns what the human confirm dialog needs, or the
/// refusal the model must act on.
pub fn check_write(
    agent: &Agent,
    facts: &SessionFacts,
    req: &WriteRequest,
    window: &mut RateWindow,
) -> Result<Approved, Refusal> {
    // Structural first: no tool, nothing else matters.
    check_tool(agent, ToolName::SendInput)?;

    // Before anything else — someone is typing a password right now.
    if facts.echo_off {
        return Err(Refusal::EchoDisabled);
    }

    if req.command.len() > MAX_INPUT_BYTES {
        return Err(Refusal::TooLong { limit: MAX_INPUT_BYTES });
    }
    if redact::contains_secret(&req.command) {
        return Err(Refusal::SecretInCommand);
    }
    if !facts.described {
        return Err(Refusal::SessionNotDescribed);
    }
    if !facts.view_current {
        return Err(Refusal::StaleView);
    }

    // A rationale of "ok" is not a rationale. The human reads this.
    if req.rationale.trim().split_whitespace().count() < 3 {
        return Err(Refusal::RationaleMissing);
    }

    let policy = &agent.policy;

    for rule in &policy.deny {
        // Compiled per call: agents are user-installable, and a bad pattern must
        // not take the server down. `regex` is linear-time, so this is cheap and
        // cannot be made pathological by a hostile document.
        if let Ok(re) = Regex::new(&rule.pattern) {
            if re.is_match(&req.command) {
                return Err(Refusal::Denied { reason: rule.reason.clone() });
            }
        }
    }

    let classified = classify(&req.command, policy.danger_threshold);

    if let Some(skill) = facts.unread_skills.first() {
        if classified.danger >= Danger::Sensitive {
            return Err(Refusal::SkillUnread { skill: skill.clone() });
        }
    }

    let needs_refs = policy.require_references || classified.danger >= Danger::Sensitive;
    if needs_refs && req.references.is_empty() {
        return Err(Refusal::ReferencesRequired {
            reason: classified
                .reason
                .clone()
                .unwrap_or_else(|| "the active agent requires it".into()),
        });
    }

    if policy.require_rollback
        && classified.danger >= Danger::Destructive
        && req.rollback.as_ref().is_none_or(|r| r.trim().is_empty())
    {
        return Err(Refusal::RollbackRequired);
    }

    // Clamped: an agent may lower the cap, never raise it.
    let limit = policy
        .max_writes_per_minute
        .map_or(BASELINE_WRITES_PER_MINUTE, |n| n.min(BASELINE_WRITES_PER_MINUTE));
    window.check(limit)?;

    Ok(Approved { danger: classified.danger, danger_reason: classified.reason })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::agents;

    fn ok_facts() -> SessionFacts {
        SessionFacts { described: true, view_current: true, echo_off: false, unread_skills: vec![] }
    }

    fn req(cmd: &str) -> WriteRequest {
        WriteRequest {
            command: cmd.into(),
            rationale: "restarting nginx to pick up the new configuration".into(),
            references: vec!["man systemctl".into()],
            rollback: Some("systemctl stop nginx".into()),
        }
    }

    fn admin() -> Agent {
        agents::find("linux-administrator").unwrap()
    }

    #[test]
    fn a_read_only_agent_cannot_write() {
        let architect = agents::find("architect").unwrap();
        let err = check_write(&architect, &ok_facts(), &req("ls"), &mut RateWindow::default())
            .unwrap_err();
        assert!(matches!(err, Refusal::ToolNotGranted { .. }));
        assert!(err.message().contains("does not have"), "{}", err.message());
    }

    #[test]
    fn echo_off_beats_everything_else() {
        let facts = SessionFacts { echo_off: true, described: false, view_current: false, unread_skills: vec![] };
        // Even with every other precondition failing, this is the refusal we
        // must give: something is being typed at a password prompt right now.
        let err = check_write(&admin(), &facts, &req("ls"), &mut RateWindow::default()).unwrap_err();
        assert_eq!(err, Refusal::EchoDisabled);
    }

    #[test]
    fn stale_view_is_refused_and_says_how_to_recover() {
        let facts = SessionFacts { view_current: false, ..ok_facts() };
        let err = check_write(&admin(), &facts, &req("ls"), &mut RateWindow::default()).unwrap_err();
        assert_eq!(err, Refusal::StaleView);
        assert!(err.message().contains("read_output"), "must name the remedy");
    }

    #[test]
    fn describe_is_required_before_writing() {
        let facts = SessionFacts { described: false, ..ok_facts() };
        let err = check_write(&admin(), &facts, &req("ls"), &mut RateWindow::default()).unwrap_err();
        assert_eq!(err, Refusal::SessionNotDescribed);
    }

    #[test]
    fn a_token_rationale_is_rejected() {
        let mut r = req("ls");
        r.rationale = "ok".into();
        let err = check_write(&admin(), &ok_facts(), &r, &mut RateWindow::default()).unwrap_err();
        assert_eq!(err, Refusal::RationaleMissing);
    }

    #[test]
    fn a_credential_in_the_command_is_refused() {
        let r = req("mysql -uroot -pHunter2 app");
        let err = check_write(&admin(), &ok_facts(), &r, &mut RateWindow::default()).unwrap_err();
        assert_eq!(err, Refusal::SecretInCommand);
    }

    #[test]
    fn destructive_commands_need_a_rollback() {
        let mut r = req("rm -rf /var/lib/data");
        r.rollback = None;
        let err = check_write(&admin(), &ok_facts(), &r, &mut RateWindow::default()).unwrap_err();
        assert_eq!(err, Refusal::RollbackRequired);
    }

    #[test]
    fn sensitive_commands_need_references() {
        let mut r = req("systemctl stop nginx");
        r.references.clear();
        let err = check_write(&admin(), &ok_facts(), &r, &mut RateWindow::default()).unwrap_err();
        assert!(matches!(err, Refusal::ReferencesRequired { .. }));
    }

    #[test]
    fn an_agent_deny_rule_refuses_with_its_own_reason() {
        let sec = agents::find("devsecops-engineer").unwrap();
        let err = check_write(&sec, &ok_facts(), &req("ufw disable"), &mut RateWindow::default())
            .unwrap_err();
        match err {
            Refusal::Denied { reason } => assert!(reason.contains("firewall"), "{reason}"),
            other => panic!("expected a deny, got {other:?}"),
        }
    }

    #[test]
    fn an_unread_skill_blocks_a_sensitive_command() {
        let facts = SessionFacts { unread_skills: vec!["harden-sshd".into()], ..ok_facts() };
        let err = check_write(&admin(), &facts, &req("systemctl stop sshd"), &mut RateWindow::default())
            .unwrap_err();
        assert!(matches!(err, Refusal::SkillUnread { .. }));
        assert!(err.message().contains("skill://harden-sshd"), "{}", err.message());
    }

    #[test]
    fn an_ordinary_command_passes() {
        let approved =
            check_write(&admin(), &ok_facts(), &req("systemctl status nginx"), &mut RateWindow::default())
                .expect("should pass");
        assert_eq!(approved.danger, Danger::Benign);
    }

    #[test]
    fn rate_limit_engages_and_reports_a_wait() {
        let mut w = RateWindow::default();
        let agent = admin();
        for i in 0..BASELINE_WRITES_PER_MINUTE {
            check_write(&agent, &ok_facts(), &req("uptime"), &mut w)
                .unwrap_or_else(|e| panic!("write {i} refused: {e:?}"));
        }
        let err = check_write(&agent, &ok_facts(), &req("uptime"), &mut w).unwrap_err();
        assert!(matches!(err, Refusal::RateLimited { .. }));
    }

    /// An agent must not be able to buy itself more throughput than the floor.
    #[test]
    fn an_agent_cannot_raise_the_rate_limit() {
        let mut agent = admin();
        agent.policy.max_writes_per_minute = Some(10_000);
        let mut w = RateWindow::default();
        for _ in 0..BASELINE_WRITES_PER_MINUTE {
            check_write(&agent, &ok_facts(), &req("uptime"), &mut w).unwrap();
        }
        assert!(
            check_write(&agent, &ok_facts(), &req("uptime"), &mut w).is_err(),
            "policy raised the cap above the baseline"
        );
    }

    #[test]
    fn a_long_paste_is_refused() {
        let r = req(&"echo x; ".repeat(400));
        let err = check_write(&admin(), &ok_facts(), &r, &mut RateWindow::default()).unwrap_err();
        assert!(matches!(err, Refusal::TooLong { .. }));
    }

    #[test]
    fn destructive_classification_catches_the_classics() {
        for cmd in [
            "rm -rf /",
            "mkfs.ext4 /dev/sda1",
            "dd if=/dev/zero of=/dev/sda",
            "git push --force origin main",
            "DROP TABLE users;",
            "shutdown -h now",
        ] {
            assert_eq!(
                classify(cmd, DangerThreshold::Standard).danger,
                Danger::Destructive,
                "not flagged: {cmd}"
            );
        }
    }

    #[test]
    fn benign_commands_are_not_flagged_at_the_baseline() {
        for cmd in ["ls -la", "cat /etc/hostname", "grep error /var/log/syslog", "df -h"] {
            assert_eq!(
                classify(cmd, DangerThreshold::Standard).danger,
                Danger::Benign,
                "false positive: {cmd}"
            );
        }
    }

    /// Writing into Reach's own config directory cannot escalate — the baseline
    /// is compiled in and the user picks the agent — but it must be visible.
    #[test]
    fn writes_to_reach_config_are_surfaced() {
        let c = classify("echo '{}' > ~/.reach/agents/fast.json", DangerThreshold::Strict);
        assert_eq!(c.danger, Danger::Sensitive);
        assert!(c.reason.unwrap().contains("Reach's own configuration"));
    }

    /// Every refusal has to tell the model what to do next, or it retries.
    #[test]
    fn every_refusal_is_actionable() {
        let all = [
            Refusal::ToolNotGranted { tool: "send_input", agent: "Architect".into() },
            Refusal::SessionNotDescribed,
            Refusal::StaleView,
            Refusal::EchoDisabled,
            Refusal::RationaleMissing,
            Refusal::ReferencesRequired { reason: "it changes service state".into() },
            Refusal::RollbackRequired,
            Refusal::Denied { reason: "chmod 777 is never the fix; find the owner that needs access".into() },
            Refusal::SecretInCommand,
            Refusal::RateLimited { retry_after_secs: 30 },
            Refusal::TooLong { limit: 2048 },
            Refusal::SkillUnread { skill: "harden-sshd".into() },
        ];
        for r in all {
            let m = r.message();
            assert!(m.len() > 40, "refusal too terse to act on: {m}");
        }
    }
}
