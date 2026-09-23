//! Building the shell commands that compress and extract files on the remote
//! machine.
//!
//! Deliberately pure: this module decides *what* to run and never runs it, so
//! every decision below is pinned by a test rather than discovered on someone
//! else's server.
//!
//! Two things make this awkward, and both are handled here rather than at the
//! call site.
//!
//! **The tools are not there.** `zip` is missing from most minimal images, and
//! a naive "is zip installed?" check would put a dead menu item in front of
//! half our users. So each operation has an ordered list of ways to do it and
//! takes the first the machine can manage. Every chain bottoms out somewhere
//! sensible, and the tar family bottoms out at `tar`, which is on effectively
//! every Linux, macOS and BSD — so compressing always works, whatever else is
//! missing.
//!
//! **Filenames are hostile.** A file really can be called `a;rm -rf ~` or
//! `$(whoami).txt` or start with a dash. Every operand here goes through
//! `shell_quote`, and every path is written `./name` so a leading dash can
//! never be read as an option. That matters more than the `--` separator,
//! which not every tool honours.
//!
//! There is no `.rar`. Creating one needs a proprietary binary that is not in
//! most default repositories, so the menu entry would usually be a promise we
//! could not keep.

use crate::recipe::invoke::shell_quote;
use serde::{Deserialize, Serialize};

/// Everything worth probing for. Ordered roughly by how likely it is present.
pub const PROBED_TOOLS: [&str; 12] = [
    "tar", "gzip", "bzip2", "xz", "zstd", "zip", "unzip", "bsdtar", "7z", "7za", "7zz", "python3",
];

/// One command that reports which of the tools above exist.
///
/// `command -v` rather than `which`, because `which` is itself a package that
/// minimal images leave out. One round trip, because this runs on every
/// connection and a dozen of them would be felt.
pub fn probe_command() -> String {
    let names = PROBED_TOOLS.join(" ");
    format!("for c in {names}; do command -v \"$c\" >/dev/null 2>&1 && printf '%s\\n' \"$c\"; done")
}

/// What a remote machine can actually do.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ArchiveTools {
    pub present: Vec<String>,
}

impl ArchiveTools {
    pub fn parse(stdout: &str) -> Self {
        let present = stdout
            .lines()
            .map(str::trim)
            .filter(|l| PROBED_TOOLS.contains(l))
            .map(str::to_string)
            .collect();
        Self { present }
    }

    pub fn has(&self, tool: &str) -> bool {
        self.present.iter().any(|t| t == tool)
    }

    /// The first of `candidates` this machine has.
    fn first(&self, candidates: &[&str]) -> Option<String> {
        candidates
            .iter()
            .find(|c| self.has(c))
            .map(|c| (*c).to_string())
    }

    /// Whichever 7-Zip spelling is installed. The name changed twice.
    fn seven_zip(&self) -> Option<String> {
        self.first(&["7zz", "7z", "7za"])
    }
}

/// The formats offered for *creating* an archive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ArchiveFormat {
    TarGz,
    TarBz2,
    TarXz,
    TarZst,
    Zip,
}

impl ArchiveFormat {
    pub fn extension(self) -> &'static str {
        match self {
            Self::TarGz => "tar.gz",
            Self::TarBz2 => "tar.bz2",
            Self::TarXz => "tar.xz",
            Self::TarZst => "tar.zst",
            Self::Zip => "zip",
        }
    }

    /// The compressor `tar` shells out to, if this is a tar format.
    fn compressor(self) -> Option<(&'static str, &'static str)> {
        match self {
            Self::TarGz => Some(("gzip", "-z")),
            Self::TarBz2 => Some(("bzip2", "-j")),
            Self::TarXz => Some(("xz", "-J")),
            Self::TarZst => Some(("zstd", "--zstd")),
            Self::Zip => None,
        }
    }
}

/// A command to run, and what to feed its standard input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    pub command: String,
    /// The tool this plan chose, for telling the user what happened.
    pub tool: String,
}

/// Written `./name` so that a file called `-rf` is an operand, never a flag.
fn operand(name: &str) -> String {
    shell_quote(&format!("./{}", name.trim_start_matches("./")))
}

/// Move into the working directory first, so archives hold relative paths
/// rather than the absolute path of whoever happened to build them.
fn in_dir(dir: &str, rest: &str) -> String {
    format!("cd -- {} && {}", shell_quote(dir), rest)
}

/// Which formats this machine can actually produce.
pub fn creatable_formats(tools: &ArchiveTools) -> Vec<ArchiveFormat> {
    let all = [
        ArchiveFormat::TarGz,
        ArchiveFormat::TarBz2,
        ArchiveFormat::TarXz,
        ArchiveFormat::TarZst,
        ArchiveFormat::Zip,
    ];
    all.into_iter()
        .filter(|f| create_supported(tools, *f))
        .collect()
}

fn create_supported(tools: &ArchiveTools, format: ArchiveFormat) -> bool {
    match format.compressor() {
        Some((tool, _)) => (tools.has("tar") || tools.has("bsdtar")) && tools.has(tool),
        None => {
            tools.has("zip")
                || tools.has("bsdtar")
                || tools.seven_zip().is_some()
                || tools.has("python3")
        }
    }
}

/// How to compress `entries` inside `dir` into `archive`.
///
/// Returns `None` when nothing on the machine can do it, which is the caller's
/// cue to not offer it rather than to fail late.
pub fn create_command(
    tools: &ArchiveTools,
    format: ArchiveFormat,
    dir: &str,
    entries: &[String],
    archive: &str,
) -> Option<Plan> {
    if entries.is_empty() {
        return None;
    }
    let names: Vec<String> = entries.iter().map(|e| operand(e)).collect();
    let joined = names.join(" ");
    let out = operand(archive);

    if let Some((tool, flag)) = format.compressor() {
        if !tools.has(tool) {
            return None;
        }
        // bsdtar understands the same short flags, and is `tar` on macOS.
        let tar = tools.first(&["tar", "bsdtar"])?;
        return Some(Plan {
            command: in_dir(dir, &format!("{tar} {flag} -cf {out} {joined}")),
            tool: tar,
        });
    }

    // Zip, in descending order of how well each does the job.
    if tools.has("zip") {
        return Some(Plan {
            command: in_dir(dir, &format!("zip -r -q {out} {joined}")),
            tool: "zip".into(),
        });
    }
    if tools.has("bsdtar") {
        return Some(Plan {
            command: in_dir(dir, &format!("bsdtar -a -cf {out} {joined}")),
            tool: "bsdtar".into(),
        });
    }
    if let Some(sz) = tools.seven_zip() {
        return Some(Plan {
            command: in_dir(dir, &format!("{sz} a -bso0 -bsp0 -tzip {out} {joined}")),
            tool: sz,
        });
    }
    if tools.has("python3") {
        return Some(Plan {
            command: in_dir(dir, &format!("python3 -m zipfile -c {out} {joined}")),
            tool: "python3".into(),
        });
    }
    None
}

/// The archive families we can open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArchiveKind {
    Tar,
    Zip,
}

/// Whether a filename looks like something we can extract, and how.
///
/// Compression is matched before the bare `.tar` so that `.tar.gz` is not read
/// as a `.gz`, and `.tgz` and friends are included because they are common and
/// cost nothing.
pub fn kind_of(name: &str) -> Option<ArchiveKind> {
    let lower = name.to_ascii_lowercase();
    const TAR: [&str; 9] = [
        ".tar.gz", ".tar.bz2", ".tar.xz", ".tar.zst", ".tgz", ".tbz2", ".txz", ".tzst", ".tar",
    ];
    if TAR.iter().any(|e| lower.ends_with(e)) {
        return Some(ArchiveKind::Tar);
    }
    if lower.ends_with(".zip") {
        return Some(ArchiveKind::Zip);
    }
    None
}

/// The directory name to extract into: the archive without its extension.
///
/// Extracting into a fresh directory rather than the current one is a
/// deliberate safety choice. It stops an archive full of loose files from
/// scattering them over the user's working directory, and it means a hostile
/// archive cannot quietly replace a neighbouring file.
pub fn extract_dir_name(archive: &str) -> String {
    let lower = archive.to_ascii_lowercase();
    const STRIP: [&str; 10] = [
        ".tar.gz", ".tar.bz2", ".tar.xz", ".tar.zst", ".tgz", ".tbz2", ".txz", ".tzst", ".tar",
        ".zip",
    ];
    for ext in STRIP {
        if lower.ends_with(ext) {
            return archive[..archive.len() - ext.len()].to_string();
        }
    }
    archive.to_string()
}

/// How to extract `archive` inside `dir` into the subdirectory `dest`.
pub fn extract_command(
    tools: &ArchiveTools,
    dir: &str,
    archive: &str,
    dest: &str,
) -> Option<Plan> {
    let kind = kind_of(archive)?;
    let src = operand(archive);
    let out = operand(dest);
    // The destination is made first; extracting into a missing directory fails
    // differently in every tool.
    let mkdir = format!("mkdir -p -- {}", out);

    match kind {
        ArchiveKind::Tar => {
            // Both tars sniff the compression themselves, so one form covers
            // every member of the family. --no-same-owner keeps an archive
            // built by root from trying to chown on the way out.
            let tar = tools.first(&["tar", "bsdtar"])?;
            Some(Plan {
                command: in_dir(
                    dir,
                    &format!("{mkdir} && {tar} -xf {src} --no-same-owner -C {out}"),
                ),
                tool: tar,
            })
        }
        ArchiveKind::Zip => {
            if tools.has("unzip") {
                return Some(Plan {
                    command: in_dir(dir, &format!("{mkdir} && unzip -o -q {src} -d {out}")),
                    tool: "unzip".into(),
                });
            }
            if tools.has("bsdtar") {
                return Some(Plan {
                    command: in_dir(dir, &format!("{mkdir} && bsdtar -xf {src} -C {out}")),
                    tool: "bsdtar".into(),
                });
            }
            if let Some(sz) = tools.seven_zip() {
                // 7-Zip takes no space after -o, which is easy to get wrong.
                return Some(Plan {
                    command: in_dir(
                        dir,
                        &format!("{mkdir} && {sz} x -y -bso0 -bsp0 -o{out} {src}"),
                    ),
                    tool: sz,
                });
            }
            if tools.has("python3") {
                return Some(Plan {
                    command: in_dir(dir, &format!("{mkdir} && python3 -m zipfile -e {src} {out}")),
                    tool: "python3".into(),
                });
            }
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tools(list: &[&str]) -> ArchiveTools {
        ArchiveTools {
            present: list.iter().map(|s| s.to_string()).collect(),
        }
    }

    fn one(name: &str) -> Vec<String> {
        vec![name.to_string()]
    }

    #[test]
    fn probe_asks_about_every_tool_and_survives_a_missing_which() {
        let cmd = probe_command();
        for tool in PROBED_TOOLS {
            assert!(cmd.contains(tool), "probe never asks about {tool}");
        }
        assert!(cmd.contains("command -v"), "must not depend on `which`");
        assert!(!cmd.contains("which "));
    }

    #[test]
    fn probe_output_is_read_back_and_nonsense_ignored() {
        let t = ArchiveTools::parse("tar\ngzip\nzip\n\nnot-a-tool\n  unzip  \n");
        assert!(t.has("tar") && t.has("gzip") && t.has("zip") && t.has("unzip"));
        assert!(!t.has("not-a-tool"));
        assert!(!t.has("7z"));
    }

    // The point of the whole design: a machine with almost nothing can still
    // compress, because tar and gzip are always there.
    #[test]
    fn a_bare_machine_can_still_make_a_tar_gz() {
        let t = tools(&["tar", "gzip"]);
        assert_eq!(creatable_formats(&t), vec![ArchiveFormat::TarGz]);
        let plan = create_command(&t, ArchiveFormat::TarGz, "/srv", &one("data"), "data.tar.gz")
            .expect("tar.gz must always be possible");
        assert_eq!(
            plan.command,
            "cd -- '/srv' && tar -z -cf './data.tar.gz' './data'"
        );
    }

    #[test]
    fn zip_falls_back_through_every_tool_in_turn() {
        for (have, expected) in [
            (vec!["tar", "gzip", "zip"], "zip"),
            (vec!["tar", "gzip", "bsdtar"], "bsdtar"),
            (vec!["tar", "gzip", "7zz"], "7zz"),
            (vec!["tar", "gzip", "7za"], "7za"),
            (vec!["tar", "gzip", "python3"], "python3"),
        ] {
            let t = tools(&have);
            let plan = create_command(&t, ArchiveFormat::Zip, "/srv", &one("d"), "d.zip")
                .unwrap_or_else(|| panic!("no zip plan with {have:?}"));
            assert_eq!(plan.tool, expected, "wrong choice from {have:?}");
        }
    }

    #[test]
    fn zip_is_not_offered_when_nothing_can_make_one() {
        let t = tools(&["tar", "gzip"]);
        assert!(!creatable_formats(&t).contains(&ArchiveFormat::Zip));
        assert!(create_command(&t, ArchiveFormat::Zip, "/srv", &one("d"), "d.zip").is_none());
    }

    #[test]
    fn a_tar_format_needs_its_compressor_present() {
        let t = tools(&["tar", "gzip"]);
        // tar alone cannot make an xz without xz installed.
        assert!(create_command(&t, ArchiveFormat::TarXz, "/srv", &one("d"), "d.tar.xz").is_none());
        let with_xz = tools(&["tar", "gzip", "xz"]);
        assert!(creatable_formats(&with_xz).contains(&ArchiveFormat::TarXz));
    }

    // Everything below is the part that decides whether this is safe.
    #[test]
    fn a_hostile_filename_cannot_reach_the_shell() {
        let t = tools(&["tar", "gzip"]);
        for nasty in [
            "a; rm -rf ~",
            "$(whoami).txt",
            "`id`",
            "a && reboot",
            "a|b",
            "it's mine",
            "new\nline",
        ] {
            let plan =
                create_command(&t, ArchiveFormat::TarGz, "/srv", &one(nasty), "out.tar.gz").unwrap();
            let quoted = shell_quote(&format!("./{nasty}"));
            assert!(
                plan.command.contains(&quoted),
                "{nasty:?} was not quoted into {}",
                plan.command
            );

            // Stronger than hunting for stray metacharacters, which would
            // trip over our own `cd ... && tar`: the command built for a
            // hostile name must be structurally identical to the one built
            // for a dull name, differing only inside the quoted operand.
            let benign =
                create_command(&t, ArchiveFormat::TarGz, "/srv", &one("plain"), "out.tar.gz")
                    .unwrap();
            assert_eq!(
                plan.command.replace(&quoted, "<OPERAND>"),
                benign
                    .command
                    .replace(&shell_quote("./plain"), "<OPERAND>"),
                "{nasty:?} changed the shape of the command: {}",
                plan.command
            );
        }
    }

    #[test]
    fn a_leading_dash_is_an_operand_not_a_flag() {
        let t = tools(&["tar", "gzip"]);
        let plan =
            create_command(&t, ArchiveFormat::TarGz, "/srv", &one("-rf"), "out.tar.gz").unwrap();
        assert!(plan.command.contains("'./-rf'"), "{}", plan.command);
        assert!(!plan.command.contains(" -rf"), "{}", plan.command);
    }

    #[test]
    fn the_directory_is_quoted_too() {
        let t = tools(&["tar", "gzip"]);
        let plan = create_command(
            &t,
            ArchiveFormat::TarGz,
            "/srv/it's here; rm -rf ~",
            &one("d"),
            "d.tar.gz",
        )
        .unwrap();
        assert!(plan.command.starts_with("cd -- '/srv/it'\\''s here; rm -rf ~'"), "{}", plan.command);
    }

    #[test]
    fn nothing_selected_produces_no_command() {
        let t = tools(&["tar", "gzip"]);
        assert!(create_command(&t, ArchiveFormat::TarGz, "/srv", &[], "x.tar.gz").is_none());
    }

    #[test]
    fn compressed_tars_are_not_mistaken_for_their_compressor() {
        for name in [
            "x.tar.gz", "x.tar.bz2", "x.tar.xz", "x.tar.zst", "x.tgz", "x.tbz2", "x.txz", "x.tzst",
            "x.tar",
        ] {
            assert_eq!(kind_of(name), Some(ArchiveKind::Tar), "{name}");
        }
        assert_eq!(kind_of("x.zip"), Some(ArchiveKind::Zip));
        assert_eq!(kind_of("X.ZIP"), Some(ArchiveKind::Zip), "case must not matter");
        for name in ["notes.txt", "x.rar", "archive", "x.gz", "x.7z"] {
            assert_eq!(kind_of(name), None, "{name} should not offer extraction");
        }
    }

    #[test]
    fn rar_is_offered_nowhere() {
        let everything = tools(&PROBED_TOOLS);
        assert_eq!(kind_of("x.rar"), None);
        let names: Vec<&str> = creatable_formats(&everything)
            .iter()
            .map(|f| f.extension())
            .collect();
        assert!(!names.iter().any(|n| n.contains("rar")), "{names:?}");
    }

    #[test]
    fn the_extraction_directory_drops_the_whole_extension() {
        assert_eq!(extract_dir_name("backup.tar.gz"), "backup");
        assert_eq!(extract_dir_name("backup.tgz"), "backup");
        assert_eq!(extract_dir_name("site.zip"), "site");
        assert_eq!(extract_dir_name("a.b.tar.xz"), "a.b");
        assert_eq!(extract_dir_name("noext"), "noext");
    }

    #[test]
    fn extracting_always_makes_its_own_directory_first() {
        let t = tools(&["tar", "unzip"]);
        for archive in ["x.tar.gz", "x.zip"] {
            let plan = extract_command(&t, "/srv", archive, "x").unwrap();
            assert!(plan.command.contains("mkdir -p -- './x'"), "{}", plan.command);
            let mkdir_at = plan.command.find("mkdir").unwrap();
            let extract_at = plan.command.rfind("./x.").unwrap();
            assert!(mkdir_at < extract_at, "directory must exist first");
        }
    }

    #[test]
    fn one_tar_form_opens_every_compressed_tar() {
        let t = tools(&["tar"]);
        // No gzip, no xz: tar still handles them, which is why the chain does
        // not branch per compression on the way out.
        for archive in ["x.tar.gz", "x.tar.xz", "x.tar.zst", "x.tar"] {
            let plan = extract_command(&t, "/srv", archive, "x").unwrap();
            assert!(plan.command.contains("tar -xf"), "{archive}: {}", plan.command);
            assert!(plan.command.contains("--no-same-owner"), "{archive}");
        }
    }

    #[test]
    fn unzipping_falls_back_through_every_tool_in_turn() {
        for (have, expected) in [
            (vec!["unzip"], "unzip"),
            (vec!["bsdtar"], "bsdtar"),
            (vec!["7z"], "7z"),
            (vec!["python3"], "python3"),
        ] {
            let t = tools(&have);
            let plan = extract_command(&t, "/srv", "a.zip", "a")
                .unwrap_or_else(|| panic!("no unzip plan with {have:?}"));
            assert_eq!(plan.tool, expected, "wrong choice from {have:?}");
        }
        assert!(extract_command(&tools(&["tar"]), "/srv", "a.zip", "a").is_none());
    }

    #[test]
    fn seven_zip_takes_its_output_flag_without_a_space() {
        let t = tools(&["7z"]);
        let plan = extract_command(&t, "/srv", "a.zip", "a").unwrap();
        assert!(plan.command.contains("-o'./a'"), "{}", plan.command);
        assert!(!plan.command.contains("-o './a'"), "{}", plan.command);
    }

    #[test]
    fn a_hostile_archive_name_cannot_reach_the_shell_either() {
        let t = tools(&["tar", "unzip"]);
        let nasty = "a; rm -rf ~.zip";
        let plan = extract_command(&t, "/srv", nasty, "a").unwrap();
        assert!(plan.command.contains(&shell_quote(&format!("./{nasty}"))), "{}", plan.command);
    }
}
