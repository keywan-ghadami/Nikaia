//! The build cache: `nikaia.lock` is the record, the key is derived from it.
//!
//! [ADR-019](../../../docs/specification/adr/adr-019.md) decides the shape of
//! this and the reasoning is there; what follows is the part that has to be
//! true in code.
//!
//! * **D2** - the lockfile records everything that *determines* a build, and
//!   only that: input hashes, the toolchain used, the compiler's own version.
//! * **D3** - the compiler version is in the key. `emit` is a function we edit,
//!   so identical input under an edited emitter is a different build.
//! * **D5** - the profile and the backend are *choices*, hashed into the key
//!   and never written to the file. A lockfile that records them is rewritten
//!   by every profile switch, for a diff that means nothing.
//! * **D6** - one key per unit. Hashing the whole lockfile would make one
//!   changed asset invalidate the project.
//! * **D7** - every dimension is listed in one place ([`Key::build`]) and
//!   nothing is captured incidentally. Fields are length-prefixed so that no
//!   two different sets of dimensions can hash alike.
//! * **D8** - artifacts are content-addressed under `target/nikaia/cache/`,
//!   with no index of its own: the key is derived, never looked up.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Bumped when the key derivation changes in a way that would otherwise let an
/// artifact from an older scheme be served under a colliding key.
const KEY_DOMAIN: &str = "nikaia-cache-key-v1";

/// The lockfile format version, so a future reader can refuse rather than
/// misread.
const LOCK_VERSION: u32 = 1;

pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// What was chosen at invocation time (D5).
///
/// These reach the key and never the lockfile. Adding a field here means
/// adding a line to [`Key::build`] - which is the point of keeping them in a
/// struct of their own rather than passing loose strings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Choices {
    pub profile: String,
    pub backend: String,
}

impl Choices {
    pub fn new(profile: impl Into<String>, backend: impl Into<String>) -> Self {
        Self {
            profile: profile.into(),
            backend: backend.into(),
        }
    }
}

/// What the lockfile records about one translation unit: its inputs, hashed.
///
/// The asset list is what makes the next build's lookup possible at all. A
/// compiler cannot know which external files a unit reads without expanding
/// its macros, so the list is carried over from the previous build - and that
/// is sound because a unit that would read a *different* file had to change
/// its own source to say so, which changes `source` and misses the key.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnitRecord {
    /// SHA256 of the unit's own source.
    pub source: String,
    /// Compile-time I/O: asset path -> SHA256 of its contents.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub assets: BTreeMap<String, String>,
}

/// `nikaia.lock` (D2). Committed, and the answer to "does this build the same
/// thing for you as it does for me?".
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lockfile {
    pub version: u32,
    /// The toolchain actually used - a record, not a constraint (D4). The
    /// constraint lives in the manifest.
    pub toolchain: String,
    /// The Nikaia compiler's own version (D3).
    pub compiler: String,
    /// `BTreeMap`, so the file is byte-identical for identical inputs rather
    /// than ordered by whatever a hash map felt like.
    #[serde(default)]
    pub units: BTreeMap<String, UnitRecord>,
}

impl Lockfile {
    pub fn new(toolchain: impl Into<String>, compiler: impl Into<String>) -> Self {
        Self {
            version: LOCK_VERSION,
            toolchain: toolchain.into(),
            compiler: compiler.into(),
            units: BTreeMap::new(),
        }
    }

    /// Reads a lockfile, or starts an empty one if the path does not exist.
    ///
    /// A lockfile from a different toolchain or compiler is *kept*, not
    /// discarded: its entries simply cannot match, because the key is built
    /// from the current identity (D3). Throwing it away would lose the asset
    /// lists that make the next lookup possible.
    pub fn load(
        path: &Path,
        toolchain: impl Into<String>,
        compiler: impl Into<String>,
    ) -> Result<Self> {
        let toolchain = toolchain.into();
        let compiler = compiler.into();
        if !path.exists() {
            return Ok(Self::new(toolchain, compiler));
        }
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("failed to read lockfile {}", path.display()))?;
        let mut lock: Lockfile = toml::from_str(&text)
            .with_context(|| format!("failed to parse lockfile {}", path.display()))?;
        anyhow::ensure!(
            lock.version == LOCK_VERSION,
            "lockfile {} has version {}, this compiler writes version {LOCK_VERSION}",
            path.display(),
            lock.version
        );
        lock.toolchain = toolchain;
        lock.compiler = compiler;
        Ok(lock)
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let body = toml::to_string_pretty(self).context("failed to serialise lockfile")?;
        let text = format!(
            "# AUTO-GENERATED by `nikaia build`. Commit this file like a lockfile.\n\
             # It records what determined the build; build-time choices (profile,\n\
             # backend) are deliberately absent - see ADR-019 D5.\n{body}"
        );
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).ok();
            }
        }
        std::fs::write(path, text)
            .with_context(|| format!("failed to write lockfile {}", path.display()))
    }
}

/// A cache key for one unit (D6).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Key(String);

impl Key {
    /// **The one place every dimension of the key is named** (D7).
    ///
    /// Nothing is hashed that is not listed here, and nothing listed here is
    /// optional. Each field is length-prefixed and tagged, so that no
    /// rearrangement of values can produce the same digest as a different set
    /// of dimensions.
    pub fn build(
        compiler: &str,
        toolchain: &str,
        choices: &Choices,
        unit: &str,
        record: &UnitRecord,
    ) -> Self {
        let mut b = KeyBuilder::new(KEY_DOMAIN);
        b.field("compiler", compiler);
        b.field("toolchain", toolchain);
        b.field("profile", &choices.profile);
        b.field("backend", &choices.backend);
        b.field("unit", unit);
        b.field("source", &record.source);
        // `BTreeMap` iterates in key order, so the same assets hash the same
        // regardless of the order they were discovered in.
        for (path, hash) in &record.assets {
            b.field("asset-path", path);
            b.field("asset-hash", hash);
        }
        Key(b.finish())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

struct KeyBuilder {
    hasher: Sha256,
}

impl KeyBuilder {
    fn new(domain: &str) -> Self {
        let mut b = KeyBuilder {
            hasher: Sha256::new(),
        };
        b.field("domain", domain);
        b
    }

    fn field(&mut self, name: &str, value: &str) {
        // Length prefixes are what make this unambiguous: without them
        // ("ab", "c") and ("a", "bc") would hash identically.
        self.hasher.update((name.len() as u64).to_le_bytes());
        self.hasher.update(name.as_bytes());
        self.hasher.update((value.len() as u64).to_le_bytes());
        self.hasher.update(value.as_bytes());
    }

    fn finish(self) -> String {
        self.hasher
            .finalize()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()
    }
}

/// The content-addressed artifact store (D8), rooted at `target/nikaia/cache/`.
///
/// No index: an entry's path is a function of its key, so the store cannot
/// disagree with the lockfile about what it holds.
#[derive(Debug, Clone)]
pub struct Store {
    root: PathBuf,
}

impl Store {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Store { root: root.into() }
    }

    fn path_for(&self, key: &Key) -> PathBuf {
        // Two-level fan-out, so a large store does not become one directory
        // with a hundred thousand entries in it.
        let k = key.as_str();
        self.root.join(&k[..2]).join(&k[2..])
    }

    pub fn get(&self, key: &Key) -> Option<String> {
        std::fs::read_to_string(self.path_for(key)).ok()
    }

    pub fn put(&self, key: &Key, artifact: &str) -> Result<()> {
        let path = self.path_for(key);
        let dir = path.parent().expect("store paths always have a parent");
        std::fs::create_dir_all(dir)
            .with_context(|| format!("failed to create cache directory {}", dir.display()))?;
        // Write-then-rename: a reader never sees a half-written artifact, and
        // two builds racing on the same key both end up with a complete file.
        let tmp = path.with_extension(format!("tmp{}", std::process::id()));
        std::fs::write(&tmp, artifact)
            .with_context(|| format!("failed to write cache entry {}", tmp.display()))?;
        std::fs::rename(&tmp, &path)
            .with_context(|| format!("failed to commit cache entry {}", path.display()))
    }
}

/// The lockfile and the store, used together.
///
/// The division of labour is ADR-019 D10's: this decides *whether to start*.
/// It is consulted before any work and answers with an artifact or with
/// nothing. It is not the contract ledger, which can only be compared after
/// inference has run.
#[derive(Debug)]
pub struct Cache {
    lock_path: PathBuf,
    store: Store,
    lock: Lockfile,
}

impl Cache {
    pub fn open(
        lock_path: impl Into<PathBuf>,
        store_root: impl Into<PathBuf>,
        toolchain: &str,
        compiler: &str,
    ) -> Result<Self> {
        let lock_path = lock_path.into();
        let lock = Lockfile::load(&lock_path, toolchain, compiler)?;
        Ok(Cache {
            lock_path,
            store: Store::new(store_root),
            lock,
        })
    }

    pub fn lockfile(&self) -> &Lockfile {
        &self.lock
    }

    /// Hashes the assets this unit read on the previous build, as they are on
    /// disk *now*. A recorded asset that has since disappeared is a miss, not
    /// an error: the unit is simply rebuilt, and the build reports the real
    /// problem if the file is still needed.
    fn current_record(&self, unit: &str, source: &str, asset_root: &Path) -> Option<UnitRecord> {
        let previous = self.lock.units.get(unit);
        let mut assets = BTreeMap::new();
        if let Some(previous) = previous {
            for path in previous.assets.keys() {
                let bytes = std::fs::read(asset_root.join(path)).ok()?;
                assets.insert(path.clone(), sha256_hex(&bytes));
            }
        }
        Some(UnitRecord {
            source: sha256_hex(source.as_bytes()),
            assets,
        })
    }

    /// The skip decision. `Some(artifact)` means nothing has to be built.
    pub fn lookup(
        &self,
        unit: &str,
        source: &str,
        choices: &Choices,
        asset_root: &Path,
    ) -> Option<String> {
        let record = self.current_record(unit, source, asset_root)?;
        let key = Key::build(
            &self.lock.compiler,
            &self.lock.toolchain,
            choices,
            unit,
            &record,
        );
        self.store.get(&key)
    }

    /// Records a freshly built unit: its inputs into the lockfile, its output
    /// into the store. `assets` is what the build actually read - an empty map
    /// is correct for a unit that read nothing.
    pub fn record(
        &mut self,
        unit: &str,
        source: &str,
        assets: BTreeMap<String, String>,
        choices: &Choices,
        artifact: &str,
    ) -> Result<()> {
        let record = UnitRecord {
            source: sha256_hex(source.as_bytes()),
            assets,
        };
        let key = Key::build(
            &self.lock.compiler,
            &self.lock.toolchain,
            choices,
            unit,
            &record,
        );
        self.store.put(&key, artifact)?;
        self.lock.units.insert(unit.to_string(), record);
        Ok(())
    }

    pub fn save(&self) -> Result<()> {
        self.lock.save(&self.lock_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn choices() -> Choices {
        Choices::new("advanced", "rust")
    }

    fn record(source: &str) -> UnitRecord {
        UnitRecord {
            source: sha256_hex(source.as_bytes()),
            assets: BTreeMap::new(),
        }
    }

    fn key_with(compiler: &str, toolchain: &str, c: &Choices, unit: &str, src: &str) -> Key {
        Key::build(compiler, toolchain, c, unit, &record(src))
    }

    #[test]
    fn identical_inputs_produce_identical_keys() {
        let a = key_with("0.1.0", "rustc-x", &choices(), "a.nika", "fn main() {}");
        let b = key_with("0.1.0", "rustc-x", &choices(), "a.nika", "fn main() {}");
        assert_eq!(a, b);
    }

    /// D5. Without this the cache would hand one profile's artifact to the
    /// other, and `the_profiles_agree_on_every_example` would compare an
    /// artifact against itself.
    #[test]
    fn the_profile_changes_the_key() {
        let lite = Choices::new("lite", "rust");
        let advanced = Choices::new("advanced", "rust");
        assert_ne!(
            key_with("0.1.0", "rustc-x", &lite, "a.nika", "src"),
            key_with("0.1.0", "rustc-x", &advanced, "a.nika", "src"),
        );
    }

    /// D3. The emitter is a function we edit; a changed compiler is a changed
    /// build even when nothing else moved.
    #[test]
    fn the_compiler_version_changes_the_key() {
        assert_ne!(
            key_with("0.1.0", "rustc-x", &choices(), "a.nika", "src"),
            key_with("0.2.0", "rustc-x", &choices(), "a.nika", "src"),
        );
    }

    #[test]
    fn the_toolchain_changes_the_key() {
        assert_ne!(
            key_with("0.1.0", "rustc-x", &choices(), "a.nika", "src"),
            key_with("0.1.0", "rustc-y", &choices(), "a.nika", "src"),
        );
    }

    #[test]
    fn the_backend_changes_the_key() {
        assert_ne!(
            key_with(
                "0.1.0",
                "rustc-x",
                &Choices::new("advanced", "rust"),
                "a",
                "s"
            ),
            key_with(
                "0.1.0",
                "rustc-x",
                &Choices::new("advanced", "bridge"),
                "a",
                "s"
            ),
        );
    }

    /// D7. Without length prefixes, moving a character from one field to the
    /// next would leave the digest unchanged.
    #[test]
    fn fields_cannot_run_into_one_another() {
        let ab = Choices::new("ab", "c");
        let a_bc = Choices::new("a", "bc");
        assert_ne!(
            key_with("0.1.0", "t", &ab, "u", "s"),
            key_with("0.1.0", "t", &a_bc, "u", "s"),
        );
    }

    /// D6. The key covers one unit's inputs, so a sibling's change cannot
    /// invalidate it.
    #[test]
    fn a_sibling_unit_does_not_affect_this_key() {
        let mut lock = Lockfile::new("rustc-x", "0.1.0");
        lock.units.insert("a.nika".into(), record("a"));
        let before = Key::build(
            &lock.compiler,
            &lock.toolchain,
            &choices(),
            "a.nika",
            lock.units.get("a.nika").unwrap(),
        );
        lock.units.insert("b.nika".into(), record("b changed"));
        let after = Key::build(
            &lock.compiler,
            &lock.toolchain,
            &choices(),
            "a.nika",
            lock.units.get("a.nika").unwrap(),
        );
        assert_eq!(before, after);
    }

    /// Assets are hashed on content, and the order they were discovered in is
    /// not a dimension.
    #[test]
    fn asset_order_is_not_a_dimension_but_content_is() {
        let mut one = UnitRecord {
            source: sha256_hex(b"s"),
            assets: BTreeMap::new(),
        };
        one.assets.insert("x.sql".into(), sha256_hex(b"X"));
        one.assets.insert("y.sql".into(), sha256_hex(b"Y"));
        let mut two = UnitRecord {
            source: sha256_hex(b"s"),
            assets: BTreeMap::new(),
        };
        two.assets.insert("y.sql".into(), sha256_hex(b"Y"));
        two.assets.insert("x.sql".into(), sha256_hex(b"X"));
        assert_eq!(
            Key::build("0.1.0", "t", &choices(), "u", &one),
            Key::build("0.1.0", "t", &choices(), "u", &two),
        );

        two.assets.insert("y.sql".into(), sha256_hex(b"Y changed"));
        assert_ne!(
            Key::build("0.1.0", "t", &choices(), "u", &one),
            Key::build("0.1.0", "t", &choices(), "u", &two),
        );
    }

    fn scratch(name: &str) -> PathBuf {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static N: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!(
            "nikaia-cache-{name}-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_recorded_unit_is_found_again_and_a_changed_one_is_not() {
        let dir = scratch("roundtrip");
        let mut cache =
            Cache::open(dir.join("nikaia.lock"), dir.join("store"), "t", "0.1.0").unwrap();

        assert!(cache
            .lookup("a.nika", "fn main() {}", &choices(), &dir)
            .is_none());

        cache
            .record(
                "a.nika",
                "fn main() {}",
                BTreeMap::new(),
                &choices(),
                "EMITTED",
            )
            .unwrap();

        assert_eq!(
            cache.lookup("a.nika", "fn main() {}", &choices(), &dir),
            Some("EMITTED".to_string())
        );
        // A changed source is a different unit as far as the key is concerned.
        assert!(cache
            .lookup("a.nika", "fn other() {}", &choices(), &dir)
            .is_none());
        // As is the same source under a different profile.
        assert!(cache
            .lookup(
                "a.nika",
                "fn main() {}",
                &Choices::new("lite", "rust"),
                &dir
            )
            .is_none());

        std::fs::remove_dir_all(&dir).ok();
    }

    /// D5, on disk: the profile and backend must not appear in the committed
    /// file, or switching profile would rewrite it for nothing.
    #[test]
    fn the_lockfile_records_inputs_and_not_choices() {
        let dir = scratch("lockfile");
        let path = dir.join("nikaia.lock");
        let mut cache = Cache::open(&path, dir.join("store"), "rustc-x", "0.1.0").unwrap();
        cache
            .record(
                "a.nika",
                "fn main() {}",
                BTreeMap::new(),
                &choices(),
                "EMITTED",
            )
            .unwrap();
        cache.save().unwrap();

        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("a.nika"), "the unit is recorded:\n{text}");
        assert!(
            text.contains("rustc-x"),
            "the toolchain is recorded:\n{text}"
        );
        assert!(
            text.contains("0.1.0"),
            "the compiler version is recorded:\n{text}"
        );
        assert!(
            !text.contains("advanced"),
            "the profile must not be:\n{text}"
        );

        // And it round-trips.
        let reloaded = Lockfile::load(&path, "rustc-x", "0.1.0").unwrap();
        assert_eq!(reloaded.units, cache.lockfile().units);

        std::fs::remove_dir_all(&dir).ok();
    }

    /// The lookup rehashes the recorded assets from disk, so editing one is a
    /// miss even though the `.nika` source is untouched.
    #[test]
    fn a_changed_asset_invalidates_the_unit() {
        let dir = scratch("assets");
        let asset = dir.join("schema.sql");
        std::fs::write(&asset, "CREATE TABLE a;").unwrap();

        let mut cache =
            Cache::open(dir.join("nikaia.lock"), dir.join("store"), "t", "0.1.0").unwrap();
        let mut assets = BTreeMap::new();
        assets.insert("schema.sql".to_string(), sha256_hex(b"CREATE TABLE a;"));
        cache
            .record("a.nika", "src", assets, &choices(), "EMITTED")
            .unwrap();

        assert_eq!(
            cache.lookup("a.nika", "src", &choices(), &dir),
            Some("EMITTED".to_string())
        );

        std::fs::write(&asset, "CREATE TABLE b;").unwrap();
        assert!(cache.lookup("a.nika", "src", &choices(), &dir).is_none());

        // A recorded asset that is gone is a miss, not a panic.
        std::fs::remove_file(&asset).unwrap();
        assert!(cache.lookup("a.nika", "src", &choices(), &dir).is_none());

        std::fs::remove_dir_all(&dir).ok();
    }
}
