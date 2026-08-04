//! Install receipts for fallback-installed tools (PRD-060 Phase 1).
//!
//! Stored at `~/.jarvy/install-receipts.json`, mirroring the
//! `commands/doctor_cache.rs` pattern (schema version, atomic 0600
//! tmp-write + rename, `JARVY_TEST_HOME` hook, corrupt file → empty
//! store).
//!
//! A receipt records WHICH ecosystem route installed a tool so
//! `jarvy check-updates` can probe the right backend and
//! `jarvy upgrade` can re-run the same route (Phase 2 reads). Validity
//! is stat-based, like the doctor cache: the receipt carries the
//! resolved binary path + mtime + size at install time, and consumers
//! honor it only while the live binary still matches — a later native
//! reinstall (brew/apt/winget) changes the stat signature and the
//! receipt goes stale by construction, with no TTL window.
//!
//! Phase 1 is write-only: `record()` is called by the fallback runtime
//! after a successful ecosystem install. Nothing reads this file yet.
//! Receipt strings are treated as hostile input by Phase 2 readers
//! (the file is user-writable) — writers still only ever store
//! compile-time route constants and validated package names.

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

pub const RECEIPTS_SCHEMA_VERSION: u32 = 1;
pub const RECEIPTS_FILENAME: &str = "install-receipts.json";

#[derive(Debug)]
pub enum ReceiptError {
    NoHome,
    Io(std::io::Error),
    Json(serde_json::Error),
}

impl std::fmt::Display for ReceiptError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoHome => f.write_str("cannot resolve home directory"),
            Self::Io(e) => write!(f, "receipt I/O: {}", e),
            Self::Json(e) => write!(f, "receipt JSON: {}", e),
        }
    }
}

impl std::error::Error for ReceiptError {}

impl ReceiptError {
    /// Bounded label for telemetry.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::NoHome => "no_home",
            Self::Io(_) => "io",
            Self::Json(_) => "json",
        }
    }
}

/// One fallback install receipt, keyed by tool name in the store.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Receipt {
    /// Ecosystem route: "go" | "npm" | "cargo" | "uv".
    pub route: String,
    /// First-party package identifier the route installed.
    pub package: String,
    /// Version hint from config at install time ("latest", "1.7.3", ...).
    pub version_hint: String,
    pub installed_at_unix: u64,
    /// Whether jarvy bootstrapped the ecosystem toolchain for this install.
    pub toolchain_bootstrapped: bool,
    /// Resolved binary path at install time; `None` = not resolvable
    /// (e.g. fresh GOPATH/bin not yet on PATH).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bin_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bin_mtime_unix: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bin_size: Option<u64>,
}

/// On-disk receipt document.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ReceiptStore {
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    #[serde(default)]
    pub entries: BTreeMap<String, Receipt>,
}

fn default_schema_version() -> u32 {
    RECEIPTS_SCHEMA_VERSION
}

impl Default for ReceiptStore {
    fn default() -> Self {
        Self {
            schema_version: RECEIPTS_SCHEMA_VERSION,
            entries: BTreeMap::new(),
        }
    }
}

pub fn receipts_path() -> Result<PathBuf, ReceiptError> {
    Ok(jarvy_dir()?.join(RECEIPTS_FILENAME))
}

fn jarvy_dir() -> Result<PathBuf, ReceiptError> {
    if let Ok(v) = std::env::var("JARVY_TEST_HOME") {
        return Ok(PathBuf::from(v).join(".jarvy"));
    }
    let home = home_dir().ok_or(ReceiptError::NoHome)?;
    Ok(home.join(".jarvy"))
}

fn home_dir() -> Option<PathBuf> {
    #[cfg(unix)]
    {
        std::env::var_os("HOME").map(PathBuf::from)
    }
    #[cfg(windows)]
    {
        std::env::var_os("USERPROFILE").map(PathBuf::from)
    }
}

/// Load from disk. Missing file → empty store. Corrupt file → `Err(Json)`;
/// callers fall back to an empty store.
pub fn load() -> Result<ReceiptStore, ReceiptError> {
    let path = receipts_path()?;
    match fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str(&text).map_err(ReceiptError::Json),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(ReceiptStore::default()),
        Err(e) => Err(ReceiptError::Io(e)),
    }
}

/// Persist atomically: 0600 tmp-write → rename → re-assert mode.
pub fn save(store: &ReceiptStore) -> Result<(), ReceiptError> {
    let path = receipts_path()?;
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(ReceiptError::Io)?;
    }
    let tmp = path.with_extension("json.tmp");
    let body = serde_json::to_string_pretty(store).map_err(ReceiptError::Json)?;
    crate::paths::write_0600(&tmp, &body).map_err(ReceiptError::Io)?;
    fs::rename(&tmp, &path).map_err(ReceiptError::Io)?;
    crate::paths::apply_secure_mode_0600(&path);
    Ok(())
}

/// Record a successful fallback install. Load-modify-save; a corrupt
/// existing file is replaced with a fresh store rather than blocking
/// the write (the install already succeeded — the receipt is advisory).
pub fn record(
    tool: &str,
    command: &str,
    route: &'static str,
    package: &str,
    version_hint: &str,
    toolchain_bootstrapped: bool,
) -> Result<(), ReceiptError> {
    let mut store = match load() {
        Ok(s) if s.schema_version == RECEIPTS_SCHEMA_VERSION => s,
        Ok(_) => ReceiptStore::default(),
        Err(ReceiptError::Json(_)) => ReceiptStore::default(),
        Err(e) => return Err(e),
    };
    let (bin_path, bin_mtime_unix, bin_size) = match which_command(command) {
        Some(p) => {
            let sig = stat_sig(&p);
            (Some(p), sig.map(|s| s.0), sig.map(|s| s.1))
        }
        None => (None, None, None),
    };
    store.entries.insert(
        tool.to_string(),
        Receipt {
            route: route.to_string(),
            package: package.to_string(),
            version_hint: version_hint.to_string(),
            installed_at_unix: unix_now_secs(),
            toolchain_bootstrapped,
            bin_path,
            bin_mtime_unix,
            bin_size,
        },
    );
    save(&store)
}

/// A receipt is valid only while the live binary still matches the
/// stat signature captured at install time. A receipt without a
/// signature (`bin_path = None`) is NEVER valid — we can't prove the
/// binary on PATH is the one the route installed.
pub fn is_valid(receipt: &Receipt) -> bool {
    let (Some(path), Some(mtime), Some(size)) =
        (&receipt.bin_path, receipt.bin_mtime_unix, receipt.bin_size)
    else {
        return false;
    };
    stat_sig(path) == Some((mtime, size))
}

/// One disk read → only the receipts whose stat signature still
/// matches. Corrupt file / schema mismatch / no-home → empty map
/// (receipts are advisory; readers never fail on them). Invalid
/// entries are ignored, not pruned — writers are the fallback
/// installer and `jarvy upgrade` only.
pub fn load_valid() -> BTreeMap<String, Receipt> {
    match load() {
        Ok(store) if store.schema_version == RECEIPTS_SCHEMA_VERSION => store
            .entries
            .into_iter()
            .filter(|(_, r)| is_valid(r))
            .collect(),
        _ => BTreeMap::new(),
    }
}

/// Look up `tool` with the same dash ↔ underscore aliasing as
/// `registry::get_tool` / the resolver's `find_spec`.
pub fn lookup<'a>(entries: &'a BTreeMap<String, Receipt>, tool: &str) -> Option<&'a Receipt> {
    if let Some(r) = entries.get(tool) {
        return Some(r);
    }
    let dash = tool.replace('_', "-");
    if dash != tool
        && let Some(r) = entries.get(&dash)
    {
        return Some(r);
    }
    let underscore = tool.replace('-', "_");
    if underscore != tool
        && let Some(r) = entries.get(&underscore)
    {
        return Some(r);
    }
    None
}

/// Delete a tool's receipt (any aliased spelling). Idempotent: absent
/// entry, missing file, and corrupt store are all `Ok` — there is
/// nothing left to remove.
pub fn remove(tool: &str) -> Result<(), ReceiptError> {
    let mut store = match load() {
        Ok(s) if s.schema_version == RECEIPTS_SCHEMA_VERSION => s,
        Ok(_) | Err(ReceiptError::Json(_)) => return Ok(()),
        Err(e) => return Err(e),
    };
    let before = store.entries.len();
    store.entries.remove(tool);
    store.entries.remove(&tool.replace('_', "-"));
    store.entries.remove(&tool.replace('-', "_"));
    if store.entries.len() == before {
        return Ok(());
    }
    save(&store)
}

/// Binary identity at receipt time: (mtime unix seconds, byte size).
fn stat_sig(path: &str) -> Option<(i64, u64)> {
    let meta = fs::metadata(path).ok()?;
    let mtime = meta
        .modified()
        .ok()?
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    Some((mtime, meta.len()))
}

fn unix_now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn which_command(command: &str) -> Option<String> {
    #[cfg(unix)]
    {
        std::process::Command::new("which")
            .arg(command)
            .output()
            .ok()
            .and_then(|o| {
                if o.status.success() {
                    String::from_utf8(o.stdout)
                        .ok()
                        .map(|s| s.trim().to_string())
                } else {
                    None
                }
            })
    }
    #[cfg(windows)]
    {
        std::process::Command::new("where")
            .arg(command)
            .output()
            .ok()
            .and_then(|o| {
                if o.status.success() {
                    String::from_utf8(o.stdout)
                        .ok()
                        .and_then(|s| s.lines().next().map(|l| l.trim().to_string()))
                } else {
                    None
                }
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn store_roundtrips_through_json() {
        let mut store = ReceiptStore::default();
        store.entries.insert(
            "betterleaks".to_string(),
            Receipt {
                route: "go".to_string(),
                package: "github.com/betterleaks/betterleaks".to_string(),
                version_hint: "latest".to_string(),
                installed_at_unix: 1_700_000_000,
                toolchain_bootstrapped: false,
                bin_path: Some("/home/u/go/bin/betterleaks".to_string()),
                bin_mtime_unix: Some(1_700_000_000),
                bin_size: Some(1024),
            },
        );
        let json = serde_json::to_string(&store).expect("serialize");
        let back: ReceiptStore = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.schema_version, RECEIPTS_SCHEMA_VERSION);
        let r = back.entries.get("betterleaks").expect("entry");
        assert_eq!(r.route, "go");
        assert_eq!(r.bin_size, Some(1024));
    }

    #[test]
    fn corrupt_store_is_json_error_and_missing_is_default() {
        let bad: Result<ReceiptStore, _> = serde_json::from_str("not json");
        assert!(bad.is_err());
        let empty: ReceiptStore = serde_json::from_str(r#"{"schema_version":1}"#).expect("parse");
        assert!(empty.entries.is_empty());
    }

    fn receipt_with_sig(path: Option<String>, mtime: Option<i64>, size: Option<u64>) -> Receipt {
        Receipt {
            route: "go".to_string(),
            package: "github.com/x/y".to_string(),
            version_hint: "latest".to_string(),
            installed_at_unix: 1,
            toolchain_bootstrapped: false,
            bin_path: path,
            bin_mtime_unix: mtime,
            bin_size: size,
        }
    }

    #[test]
    fn receipt_without_signature_is_never_valid() {
        assert!(!is_valid(&receipt_with_sig(None, None, None)));
        assert!(!is_valid(&receipt_with_sig(
            Some("/no/such/bin".to_string()),
            None,
            Some(1),
        )));
    }

    #[test]
    fn receipt_validity_tracks_live_binary_stat() {
        let dir = std::env::temp_dir().join(format!("jarvy-receipt-test-{}", std::process::id()));
        fs::create_dir_all(&dir).expect("mkdir");
        let bin = dir.join("fakebin");
        fs::write(&bin, b"v1").expect("write");
        let path_str = bin.to_string_lossy().to_string();
        let (mtime, size) = stat_sig(&path_str).expect("sig");
        let receipt = receipt_with_sig(Some(path_str.clone()), Some(mtime), Some(size));
        assert!(is_valid(&receipt));
        // Size change = native reinstall → stale by construction.
        fs::write(&bin, b"different contents").expect("rewrite");
        assert!(!is_valid(&receipt));
        // Missing binary → invalid, not a panic.
        fs::remove_file(&bin).expect("rm");
        assert!(!is_valid(&receipt));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn lookup_applies_dash_underscore_aliasing() {
        let mut entries = BTreeMap::new();
        entries.insert(
            "nats-server".to_string(),
            receipt_with_sig(None, None, None),
        );
        assert!(lookup(&entries, "nats-server").is_some());
        assert!(lookup(&entries, "nats_server").is_some());
        assert!(lookup(&entries, "natsserver").is_none());

        let mut entries2 = BTreeMap::new();
        entries2.insert("cfn_lint".to_string(), receipt_with_sig(None, None, None));
        assert!(lookup(&entries2, "cfn-lint").is_some());
    }

    #[test]
    fn optional_stat_fields_are_omitted_when_absent() {
        let mut store = ReceiptStore::default();
        store.entries.insert(
            "t".to_string(),
            Receipt {
                route: "uv".to_string(),
                package: "cfn-lint".to_string(),
                version_hint: "latest".to_string(),
                installed_at_unix: 1,
                toolchain_bootstrapped: true,
                bin_path: None,
                bin_mtime_unix: None,
                bin_size: None,
            },
        );
        let json = serde_json::to_string(&store).expect("serialize");
        assert!(!json.contains("bin_path"));
        assert!(!json.contains("bin_mtime_unix"));
    }
}
