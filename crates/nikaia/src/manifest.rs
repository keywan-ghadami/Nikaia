//! `nikaia.toml`'s build switches, and the CLI overriding them for one build.
//!
//! ADR-037 D5 and [ADR-033](../../../docs/specification/adr/adr-033.md) D8 give
//! the same shape to all three settings: the manifest carries them, because
//! they are properties of a *project* rather than of an invocation and a
//! committed value is one a reviewer sees; a flag overrides for a single build,
//! which is what a benchmark and a bug hunt need.
//!
//! Nothing here decides what a setting *means* - `emit::Build` and
//! `emit::Ordering` own that, and get handed a string either way. This module
//! only answers "which string", and does it once per run so that two places
//! cannot resolve the same setting differently.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{anyhow, Context, Result};

/// Where a `nikaia.toml` was found, and what its `[build]` table said.
///
/// Absent outside a project, which is not an error: a single `.nika` file
/// compiles with the built-in defaults and the flags, and littering a manifest
/// into someone's directory to make that work would be the wrong trade.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Manifest {
    build: BTreeMap<String, String>,
}

/// The keys `[build]` may carry. A key outside this set is a typo until proven
/// otherwise, and saying so beats a switch that silently stayed at its default:
/// `user_parallelism` with an underscore is the mistake this catches.
///
/// `cleanup-deadline` is here without being read. Part III 13.3 documents it
/// and [ADR-006](../../../docs/specification/adr/adr-006.md) decides it, so a
/// manifest that follows the specification must not be refused by a compiler
/// that has not caught up with it yet.
const KNOWN: &[&str] = &["target", "user-parallelism", "ordering", "cleanup-deadline"];

impl Manifest {
    /// Read the manifest governing `input`, if there is one.
    ///
    /// The search is the one the build cache already does - the nearest
    /// `nikaia.toml` at or above the input's directory - so a file cannot be
    /// cached as part of one project and compiled with another's switches.
    pub fn find(input: &Path) -> Result<Manifest> {
        let start = input
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .map(Path::to_path_buf)
            .unwrap_or_else(|| std::path::PathBuf::from("."));
        let start = start.canonicalize().unwrap_or(start);

        for dir in start.ancestors() {
            let path = dir.join("nikaia.toml");
            if path.is_file() {
                let text = std::fs::read_to_string(&path)
                    .with_context(|| format!("reading {}", path.display()))?;
                return Manifest::parse(&text)
                    .with_context(|| format!("in {}", path.display()))
                    .map_err(|e| anyhow!("{e:#}"));
            }
        }
        Ok(Manifest::default())
    }

    /// The `[build]` table, as strings.
    ///
    /// Sub-tables are skipped rather than rejected: `[build.x86_64-linux]`
    /// carries per-target codegen choices (Part III 13.3) that are not
    /// switches and are nothing to do with this.
    pub fn parse(text: &str) -> Result<Manifest> {
        let document: toml::Value = toml::from_str(text).context("this is not valid TOML")?;
        let Some(table) = document.get("build").and_then(toml::Value::as_table) else {
            return Ok(Manifest::default());
        };

        let mut build = BTreeMap::new();
        for (key, value) in table {
            if value.is_table() {
                continue;
            }
            if !KNOWN.contains(&key.as_str()) {
                return Err(anyhow!(
                    "unknown key `{key}` in `[build]` (expected one of: {})",
                    KNOWN.join(", ")
                ));
            }
            // Every switch is a word, so a bare `no` (TOML's boolean) or a
            // count is the plausible mistake. Reporting the type here would
            // hide the *reason* - `Build::parse` explains why a count is not a
            // count - so the value is stringified and passed on to the
            // parser that owns the setting.
            let word = match value {
                toml::Value::String(word) => word.clone(),
                other => other.to_string(),
            };
            build.insert(key.clone(), word);
        }
        Ok(Manifest { build })
    }

    /// The effective value: the flag if one was given, else the manifest, else
    /// the built-in default.
    ///
    /// The order is the whole of D5. A flag that the manifest could override
    /// would make `--target` useless for the one build it exists for, and a
    /// manifest that the built-in default could override would make a
    /// committed switch a suggestion.
    pub fn setting<'a>(&'a self, key: &str, flag: Option<&'a str>, default: &'a str) -> &'a str {
        flag.or_else(|| self.build.get(key).map(String::as_str))
            .unwrap_or(default)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_flag_beats_the_manifest_and_the_manifest_beats_the_default() {
        let manifest = Manifest::parse("[build]\ntarget = \"wasm32-unknown\"\n").expect("parses");
        assert_eq!(
            manifest.setting("target", None, "x86_64-linux"),
            "wasm32-unknown"
        );
        assert_eq!(
            manifest.setting("target", Some("x86_64-linux"), "x86_64-linux"),
            "x86_64-linux"
        );
        assert_eq!(
            manifest.setting("ordering", None, "effects"),
            "effects",
            "a key the manifest does not carry falls through to the default"
        );
    }

    /// No manifest is the single-file case, not a failure.
    #[test]
    fn a_manifest_without_a_build_table_decides_nothing() {
        let manifest = Manifest::parse("[package]\nname = \"x\"\n").expect("parses");
        assert_eq!(manifest, Manifest::default());
        assert_eq!(
            manifest.setting("target", None, "x86_64-linux"),
            "x86_64-linux"
        );
    }

    /// The mistake this exists for: the switch is spelled with a hyphen in the
    /// manifest and an underscore on the CLI, and a silently ignored key would
    /// leave the build at a default the author thought they had changed.
    #[test]
    fn an_unknown_key_is_named_rather_than_ignored() {
        let error = Manifest::parse("[build]\nuser_parallelism = \"yes\"\n")
            .expect_err("an unknown key is refused");
        let text = format!("{error:#}");
        assert!(text.contains("user_parallelism"), "{text}");
        assert!(text.contains("user-parallelism"), "{text}");
    }

    /// `[build.x86_64-linux]` is codegen, not a switch. It has to survive.
    #[test]
    fn a_per_target_table_is_not_a_switch() {
        let manifest = Manifest::parse(
            "[build]\nordering = \"strict\"\n\n[build.x86_64-linux]\nopt-level = 3\nlto = true\n",
        )
        .expect("parses");
        assert_eq!(manifest.setting("ordering", None, "effects"), "strict");
    }

    /// Specified, decided, and not yet read. Refusing it would make the
    /// specification's own example manifest fail to compile.
    #[test]
    fn the_cleanup_deadline_is_accepted_before_it_is_honoured() {
        Manifest::parse("[build]\ncleanup-deadline = \"30s\"\n").expect("parses");
    }

    /// A bare `no` is TOML's boolean, and the reason it is wrong belongs to the
    /// switch's own parser rather than to a type error here.
    #[test]
    fn a_non_string_reaches_the_switch_that_can_explain_it() {
        let manifest = Manifest::parse("[build]\nuser-parallelism = false\n").expect("parses");
        assert_eq!(manifest.setting("user-parallelism", None, "no"), "false");
    }
}
