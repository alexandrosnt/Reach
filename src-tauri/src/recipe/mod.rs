//! Recipes: bash scripts that carry their own metadata, run against a session,
//! and can be shared through a registry.
//!
//! A snippet is one command you paste. A recipe is a small program with a
//! name, a version, parameters and a declared risk, meant to be run more than
//! once and on more than one machine. The two are close enough to look alike
//! and far enough apart that merging them would make both worse.
//!
//! Storage mirrors themes: plain files on disk under the app data directory,
//! one per recipe, in the format the author wrote. Not the vault — a recipe is
//! not a secret, and a script you cannot read with `cat` is a script nobody
//! will audit.

pub mod invoke;
pub mod registry;
pub mod risk;
pub mod schema;

use std::path::{Path, PathBuf};

pub use schema::{parse, template, ParseError, Recipe, RecipeParam};

/// Where installed and authored recipes live.
pub fn recipes_dir(app_dir: &Path) -> PathBuf {
    app_dir.join("recipes")
}

fn path_for(app_dir: &Path, id: &str) -> Result<PathBuf, String> {
    if !schema::valid_id(id) {
        return Err(format!(
            "Invalid recipe id '{id}': use lowercase letters, digits and hyphens"
        ));
    }
    Ok(recipes_dir(app_dir).join(format!("{id}.sh")))
}

/// Every recipe on disk.
///
/// A file that no longer parses is skipped with a warning rather than failing
/// the whole list. One bad recipe hiding every other recipe is the same
/// failure the vault had, and it is worse here: the list is the only way to
/// reach the editor that would fix it.
pub fn list(app_dir: &Path) -> Vec<Recipe> {
    let dir = recipes_dir(app_dir);
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };

    let mut out = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("sh") {
            continue;
        }
        let Ok(source) = std::fs::read_to_string(&path) else {
            tracing::warn!("Skipping unreadable recipe {:?}", path);
            continue;
        };
        match parse(&source) {
            Ok(mut recipe) => {
                recipe.origin = read_origin(app_dir, &recipe.id);
                out.push(recipe);
            }
            Err(e) => tracing::warn!("Skipping invalid recipe {:?}: {}", path, e),
        }
    }
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    out
}

/// One recipe by id.
pub fn get(app_dir: &Path, id: &str) -> Result<Recipe, String> {
    let path = path_for(app_dir, id)?;
    let source = std::fs::read_to_string(&path).map_err(|e| format!("Cannot read recipe: {e}"))?;
    let mut recipe = parse(&source).map_err(|e| e.to_string())?;
    recipe.origin = read_origin(app_dir, id);
    Ok(recipe)
}

/// Write a recipe, parsing first so an unparseable file never reaches disk.
///
/// The id in the header wins over any id passed alongside it: the file is the
/// truth, and letting the two disagree is how you end up with `a.sh` declaring
/// itself `b`.
pub fn save(app_dir: &Path, source: &str) -> Result<Recipe, String> {
    let recipe = parse(source).map_err(|e| e.to_string())?;
    let path = path_for(app_dir, &recipe.id)?;

    let dir = recipes_dir(app_dir);
    std::fs::create_dir_all(&dir).map_err(|e| format!("Cannot create recipes dir: {e}"))?;
    std::fs::write(&path, source).map_err(|e| format!("Cannot write recipe: {e}"))?;

    Ok(recipe)
}

/// Delete a recipe and any provenance recorded for it.
pub fn delete(app_dir: &Path, id: &str) -> Result<(), String> {
    let path = path_for(app_dir, id)?;
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| format!("Cannot remove recipe: {e}"))?;
    }
    let meta = origin_path(app_dir, id);
    if meta.exists() {
        let _ = std::fs::remove_file(&meta);
    }
    Ok(())
}

/// Rename a recipe's id, moving the file with it.
pub fn origin_path(app_dir: &Path, id: &str) -> PathBuf {
    recipes_dir(app_dir).join(format!(".{id}.origin.json"))
}

/// Provenance is kept beside the script rather than inside it, so editing a
/// recipe cannot forge where it came from.
pub fn write_origin(app_dir: &Path, id: &str, origin: &schema::Origin) -> Result<(), String> {
    let json = serde_json::to_vec_pretty(origin).map_err(|e| e.to_string())?;
    std::fs::write(origin_path(app_dir, id), json).map_err(|e| format!("Cannot record origin: {e}"))
}

fn read_origin(app_dir: &Path, id: &str) -> schema::Origin {
    std::fs::read(origin_path(app_dir, id))
        .ok()
        .and_then(|raw| serde_json::from_slice(&raw).ok())
        .unwrap_or(schema::Origin::Local)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("reach-recipe-test-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    const SRC: &str = "# @reach-recipe\n# id: demo\n# name: Demo\n# @end\necho hi\n";

    #[test]
    fn saves_and_reads_back_byte_for_byte() {
        let dir = temp_dir("roundtrip");
        save(&dir, SRC).unwrap();
        assert_eq!(get(&dir, "demo").unwrap().source, SRC);
    }

    #[test]
    fn refuses_to_save_a_file_that_does_not_parse() {
        // A broken file on disk is only reachable through the editor that
        // would fix it, so it must never get there in the first place.
        let dir = temp_dir("invalid");
        assert!(save(&dir, "echo not a recipe\n").is_err());
        assert!(list(&dir).is_empty());
    }

    #[test]
    fn an_unparseable_file_does_not_hide_the_good_ones() {
        let dir = temp_dir("skip");
        save(&dir, SRC).unwrap();
        std::fs::write(recipes_dir(&dir).join("broken.sh"), "not a recipe").unwrap();
        let all = list(&dir);
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].id, "demo");
    }

    #[test]
    fn an_id_cannot_escape_the_recipes_directory() {
        let dir = temp_dir("escape");
        assert!(path_for(&dir, "../../etc/passwd").is_err());
        assert!(path_for(&dir, "sub/dir").is_err());
        assert!(get(&dir, "../secret").is_err());
        assert!(delete(&dir, "../secret").is_err());
    }

    #[test]
    fn deleting_is_idempotent() {
        let dir = temp_dir("delete");
        save(&dir, SRC).unwrap();
        delete(&dir, "demo").unwrap();
        delete(&dir, "demo").expect("deleting twice is not an error");
        assert!(list(&dir).is_empty());
    }

    #[test]
    fn origin_survives_an_edit() {
        // Provenance lives beside the file precisely so someone editing the
        // script cannot relabel an installed recipe as their own — or the
        // reverse, dressing a local script as registry-vetted.
        let dir = temp_dir("origin");
        save(&dir, SRC).unwrap();
        write_origin(
            &dir,
            "demo",
            &schema::Origin::Registry {
                repo: "someone/recipes".into(),
                sha256: "abc".into(),
            },
        )
        .unwrap();

        let edited = SRC.replace("echo hi", "echo edited");
        save(&dir, &edited).unwrap();

        assert!(matches!(
            get(&dir, "demo").unwrap().origin,
            schema::Origin::Registry { .. }
        ));
    }

    /// The recipes shipped in `registry/`, checked against the parser that
    /// will actually read them.
    ///
    /// The JS index builder has its own copy of the header grammar, so the two
    /// can drift. This is where that shows up — at build time, rather than as
    /// a recipe that indexes cleanly and then refuses to install.
    mod shipped {
        use super::*;
        use crate::mcp::guard::Danger;
        use crate::recipe::risk;

        const SHIPPED: &[(&str, &str)] = &[
            ("disk-report", include_str!("../../../registry/recipes/disk-report.sh")),
            ("harden-ssh", include_str!("../../../registry/recipes/harden-ssh.sh")),
            (
                "fail2ban-baseline",
                include_str!("../../../registry/recipes/fail2ban-baseline.sh"),
            ),
        ];

        #[test]
        fn every_shipped_recipe_parses() {
            for (id, source) in SHIPPED {
                let recipe = parse(source).unwrap_or_else(|e| panic!("{id} does not parse: {e}"));
                assert_eq!(&recipe.id, id, "id must match the filename");
                assert!(!recipe.name.is_empty(), "{id} has no name");
                assert!(!recipe.description.is_empty(), "{id} has no description");
                assert!(!recipe.version.is_empty(), "{id} has no version");
                assert!(!recipe.tags.is_empty(), "{id} has no tags");
            }
        }

        #[test]
        fn no_shipped_recipe_understates_its_risk() {
            // A recipe in the default registry claiming to be tamer than it is
            // teaches people that the warning is noise. These are the ones that
            // most need to be honest.
            for (id, source) in SHIPPED {
                let recipe = parse(source).expect("parses");
                let analysis = risk::analyse(&recipe);
                assert!(
                    !analysis.understated,
                    "{id} declares {:?} but analyses as {:?}",
                    analysis.declared, analysis.found
                );
            }
        }

        #[test]
        fn every_shipped_recipe_declares_a_danger() {
            for (id, source) in SHIPPED {
                let recipe = parse(source).expect("parses");
                assert!(
                    recipe.declared_danger.is_some(),
                    "{id} must state its own risk level"
                );
            }
        }

        #[test]
        fn a_read_only_recipe_stays_benign() {
            // disk-report is the one people will run on a machine that is
            // already in trouble. If it ever grows a side effect, that should
            // fail here rather than in production.
            let (_, source) = SHIPPED[0];
            let analysis = risk::analyse(&parse(source).expect("parses"));
            assert_eq!(
                analysis.found,
                Danger::Benign,
                "disk-report must stay read-only: {:?}",
                analysis.findings
            );
            assert_eq!(analysis.danger, Danger::Benign, "and must say so in its header");
        }

        #[test]
        fn every_shipped_recipe_sets_the_failure_mode() {
            // Without `set -e` a recipe carries on after a failed step, which
            // is how half-configured machines happen.
            for (id, source) in SHIPPED {
                let recipe = parse(source).expect("parses");
                assert!(
                    recipe.script.contains("set -euo pipefail"),
                    "{id} does not set -euo pipefail"
                );
            }
        }

        #[test]
        fn every_shipped_parameter_is_a_shell_identifier() {
            for (id, source) in SHIPPED {
                let recipe = parse(source).expect("parses");
                for param in &recipe.params {
                    assert!(
                        schema::valid_param_name(&param.name),
                        "{id}: parameter {} is not a valid identifier",
                        param.name
                    );
                    assert!(!param.label.is_empty(), "{id}: {} has no label", param.name);
                }
            }
        }
    }

    #[test]
    fn listing_is_sorted_by_name() {
        let dir = temp_dir("sorted");
        save(&dir, "# @reach-recipe\n# id: b\n# name: Zebra\n# @end\nx\n").unwrap();
        save(&dir, "# @reach-recipe\n# id: a\n# name: Apple\n# @end\nx\n").unwrap();
        let names: Vec<_> = list(&dir).into_iter().map(|r| r.name).collect();
        assert_eq!(names, vec!["Apple", "Zebra"]);
    }
}
