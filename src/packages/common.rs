//! Common utilities for package handlers
//!
//! Provides shared functionality for running package manager commands
//! and handling errors across different package ecosystems.

use std::path::Path;
use std::process::Command;
use thiserror::Error;

/// Errors that can occur during package installation
#[derive(Debug, Error)]
pub enum PackageError {
    #[error("Package manager not installed: {0}")]
    PackageManagerNotInstalled(String),

    #[error("Lock file not found: {0}")]
    LockfileNotFound(String),

    #[error("Command failed: {0}")]
    CommandFailed(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Virtual environment creation failed: {0}")]
    VenvCreationFailed(String),

    #[error("Package installation failed: {0}")]
    #[allow(dead_code)] // Reserved for future use
    InstallFailed(String),

    /// A `[npm]/[pip]/[cargo]` package name or version was rejected because
    /// it would inject CLI flags or a non-package URL into the underlying
    /// package manager's argument list. Examples:
    ///
    /// ```text
    /// "--registry=http://attacker"
    /// "--git"          (cargo flag)
    /// "../../etc/passwd"
    /// "git+https://attacker/x.git"
    /// ```
    ///
    /// All of these are passed positionally to `npm install` / `cargo install`
    /// / `pip install` and would normally be honored as flags or URL deps.
    #[error("Refused unsafe package spec '{0}': {1}")]
    RefusedUnsafeSpec(String, String),
}

impl PackageError {
    /// Stable discriminant for telemetry. The `Display` form may
    /// embed user-controlled content (package names, file paths,
    /// subprocess stderr); this returns a fixed string so dashboards
    /// can group by error class without parsing free text. Mirrors
    /// the pattern set by `AiHookError::kind()`.
    pub fn kind(&self) -> &'static str {
        match self {
            PackageError::PackageManagerNotInstalled(_) => "package_manager_not_installed",
            PackageError::LockfileNotFound(_) => "lockfile_not_found",
            PackageError::CommandFailed(_) => "command_failed",
            PackageError::Io(_) => "io",
            PackageError::VenvCreationFailed(_) => "venv_creation_failed",
            PackageError::InstallFailed(_) => "install_failed",
            PackageError::RefusedUnsafeSpec(_, _) => "refused_unsafe_spec",
        }
    }
}

/// Validate a package name (`[npm]/[pip]/[cargo]` key) before passing it as
/// a positional arg. Rejects:
/// - empty
/// - leading `-` (collides with CLI flag namespace)
/// - URL/scheme prefixes (`git+`, `http://`, `https://`, `file:`, `npm:`,
///   `./`, `../`) — these are direct-URL deps that bypass the registry
/// - chars outside the safe set `[A-Za-z0-9._@/+:~-]` (npm scoped names use
///   `@scope/name`, pip extras use `[extra]` but those go in the version
///   spec, not the name; cargo crate names are `[A-Za-z0-9_-]+`)
///
/// `purpose` is included in the error message and `tracing::warn!` event for
/// support.
pub fn validate_package_name(name: &str, purpose: &'static str) -> Result<(), PackageError> {
    if name.is_empty() {
        return Err(PackageError::RefusedUnsafeSpec(
            name.to_string(),
            format!("{purpose} package name is empty"),
        ));
    }
    // Reject control bytes (including ESC `\x1b`) and DEL up front. These
    // never appear in legitimate package names but TOML quoted keys
    // preserve them — a hostile `jarvy.toml` lands ANSI / OSC sequences
    // in the operator's terminal during `--dry-run` preview, which is
    // exactly the path operators trust as "safe to inspect untrusted
    // configs." Refuse before the name reaches println!.
    if name
        .chars()
        .any(|c| c.is_control() || c == '\x1b' || c == '\x7f')
    {
        tracing::warn!(
            event = "packages.refused_control_bytes",
            purpose = %purpose,
            // Don't log the name itself — could still contain control
            // bytes redirected at the log viewer. Log only the length.
            name_len = name.chars().count(),
        );
        return Err(PackageError::RefusedUnsafeSpec(
            "<redacted: contained control bytes>".to_string(),
            format!("{purpose} package name contains control bytes (terminal-control injection)"),
        ));
    }
    if name.starts_with('-') {
        tracing::warn!(
            event = "packages.refused_flag_like_name",
            purpose = %purpose,
            name = %name,
        );
        return Err(PackageError::RefusedUnsafeSpec(
            name.to_string(),
            format!("{purpose} package name starts with `-`; would be interpreted as a CLI flag"),
        ));
    }
    const SCHEMES: &[&str] = &["git+", "http://", "https://", "file:", "npm:", "./", "../"];
    for scheme in SCHEMES {
        if name.starts_with(scheme) {
            tracing::warn!(
                event = "packages.refused_url_scheme",
                purpose = %purpose,
                scheme = %scheme,
                name = %name,
            );
            return Err(PackageError::RefusedUnsafeSpec(
                name.to_string(),
                format!(
                    "{purpose} package spec uses scheme `{scheme}` which bypasses the registry; \
                     direct-URL deps are not accepted from jarvy.toml"
                ),
            ));
        }
    }
    if !name.chars().all(|c| {
        c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '@' | '/' | '+' | ':' | '~' | '-')
    }) {
        tracing::warn!(
            event = "packages.refused_unsafe_chars",
            purpose = %purpose,
            name = %name,
        );
        return Err(PackageError::RefusedUnsafeSpec(
            name.to_string(),
            format!("{purpose} package name contains characters outside the safe set"),
        ));
    }
    Ok(())
}

/// Validate a version specifier (`{name} = "<version>"` value). Same shape as
/// the name guard but tolerates additional version characters. Rejects empty,
/// leading `-`, and URL schemes.
pub fn validate_package_version(version: &str, purpose: &'static str) -> Result<(), PackageError> {
    if version.is_empty() {
        return Err(PackageError::RefusedUnsafeSpec(
            version.to_string(),
            format!("{purpose} version is empty"),
        ));
    }
    // Reject control bytes here too — same terminal-injection concern as
    // validate_package_name. Versions get printed via the dry-run preview
    // and `Running: dotnet tool update -g <name> --version <ver>` line.
    if version
        .chars()
        .any(|c| c.is_control() || c == '\x1b' || c == '\x7f')
    {
        tracing::warn!(
            event = "packages.refused_control_bytes_version",
            purpose = %purpose,
            version_len = version.chars().count(),
        );
        return Err(PackageError::RefusedUnsafeSpec(
            "<redacted: contained control bytes>".to_string(),
            format!("{purpose} version contains control bytes (terminal-control injection)"),
        ));
    }
    if version.starts_with('-') {
        return Err(PackageError::RefusedUnsafeSpec(
            version.to_string(),
            format!("{purpose} version starts with `-`; would be interpreted as a CLI flag"),
        ));
    }
    const SCHEMES: &[&str] = &["git+", "http://", "https://", "file:", "npm:"];
    for scheme in SCHEMES {
        if version.starts_with(scheme) {
            return Err(PackageError::RefusedUnsafeSpec(
                version.to_string(),
                format!("{purpose} version uses scheme `{scheme}` which bypasses the registry"),
            ));
        }
    }
    Ok(())
}

/// Run a package manager command with the given arguments.
///
/// Pipes the subprocess's stdout and stderr through reader threads so
/// the operator sees output in real time (cargo install dep-graph
/// resolution, npm peer-dep warnings, etc.) without buffering the
/// whole subprocess output in memory. Each reader tees its stream to
/// the parent handle AND into a bounded 4KB ring buffer; on non-zero
/// exit the stderr ring is sanitized via `redact_for_display` and
/// surfaced both in the `PackageError::CommandFailed` envelope and a
/// `package_command.failed` tracing event.
///
/// Memory is bounded by `STDERR_TAIL_BYTES * 2` regardless of how
/// chatty the subprocess is. Redaction protects against (a) hostile
/// post-install scripts dumping ANSI / OSC into the operator's logs,
/// and (b) a compromised registry mirror leaking control bytes
/// through the deferred error path.
pub fn run_package_command(
    cmd: &str,
    args: &[&str],
    working_dir: &Path,
) -> Result<(), PackageError> {
    use std::io::{BufReader, Read, Write};
    use std::process::Stdio;
    use std::sync::Mutex;

    const STDERR_TAIL_BYTES: usize = 4 * 1024;

    let display_cmd = format!("{} {}", cmd, args.join(" "));
    println!("    Running: {}", display_cmd);

    let mut child = Command::new(cmd)
        .args(args)
        .current_dir(working_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                PackageError::PackageManagerNotInstalled(cmd.to_string())
            } else {
                PackageError::Io(e)
            }
        })?;

    let stdout = child.stdout.take().expect("piped stdout");
    let stderr = child.stderr.take().expect("piped stderr");

    // Shared ring buffer for stderr. The stdout ring is purely
    // pass-through (we don't surface stdout in the error envelope —
    // tools like cargo/npm route real errors to stderr).
    let stderr_ring: Mutex<RingTail> = Mutex::new(RingTail::new(STDERR_TAIL_BYTES));

    std::thread::scope(|s| {
        s.spawn(|| {
            let mut reader = BufReader::new(stdout);
            let mut sink = std::io::stdout().lock();
            let mut buf = [0u8; 4096];
            while let Ok(n) = reader.read(&mut buf) {
                if n == 0 {
                    break;
                }
                let _ = sink.write_all(&buf[..n]);
            }
        });
        s.spawn(|| {
            let mut reader = BufReader::new(stderr);
            let mut sink = std::io::stderr().lock();
            let mut buf = [0u8; 4096];
            while let Ok(n) = reader.read(&mut buf) {
                if n == 0 {
                    break;
                }
                let _ = sink.write_all(&buf[..n]);
                if let Ok(mut ring) = stderr_ring.lock() {
                    ring.extend(&buf[..n]);
                }
            }
        });
    });

    let status = child.wait().map_err(PackageError::Io)?;

    if !status.success() {
        let tail_bytes = stderr_ring
            .into_inner()
            .unwrap_or_else(|e| e.into_inner())
            .into_bytes();
        let raw_tail = String::from_utf8_lossy(&tail_bytes);
        let trimmed = raw_tail.trim();
        let safe_tail = crate::observability::sanitizer::redact_for_display(trimmed);

        // Emit the structured event only when telemetry is opted in;
        // the captured tail can otherwise carry user-sensitive
        // package-manager output to OTLP without consent.
        if crate::observability::telemetry_gate::is_enabled() {
            tracing::warn!(
                event = "package_command.failed",
                cmd = %cmd,
                exit_code = status.code().unwrap_or(-1),
                stderr_tail = %safe_tail,
            );
        }

        let message = if safe_tail.is_empty() {
            format!(
                "'{}' exited with status {}",
                display_cmd,
                status.code().unwrap_or(-1)
            )
        } else {
            format!(
                "'{}' exited with status {}\n--- last output ---\n{}",
                display_cmd,
                status.code().unwrap_or(-1),
                safe_tail
            )
        };
        return Err(PackageError::CommandFailed(message));
    }

    Ok(())
}

/// Fixed-size byte ring that retains the last `cap` bytes pushed.
/// Bounded memory regardless of subprocess output size.
struct RingTail {
    buf: Vec<u8>,
    cap: usize,
    start: usize,
    len: usize,
}

impl RingTail {
    fn new(cap: usize) -> Self {
        Self {
            buf: vec![0u8; cap],
            cap,
            start: 0,
            len: 0,
        }
    }

    fn extend(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.push(b);
        }
    }

    fn push(&mut self, b: u8) {
        if self.cap == 0 {
            return;
        }
        if self.len < self.cap {
            self.buf[(self.start + self.len) % self.cap] = b;
            self.len += 1;
        } else {
            self.buf[self.start] = b;
            self.start = (self.start + 1) % self.cap;
        }
    }

    fn into_bytes(self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.len);
        for i in 0..self.len {
            out.push(self.buf[(self.start + i) % self.cap]);
        }
        out
    }
}

/// Check if a command is available in PATH.
///
/// Thin wrapper over `crate::tools::common::has` — both functions test
/// `<cmd> --version` exit success. Previously the two existed
/// independently (Maint review F-10) so package handlers reached for
/// `command_exists` while tool handlers reached for `has`. Keeping the
/// name here for backwards compatibility while collapsing the body.
pub fn command_exists(cmd: &str) -> bool {
    crate::tools::common::has(cmd)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_exists() {
        // These commands should exist on most systems
        assert!(command_exists("echo") || command_exists("cmd"));
    }

    #[test]
    fn test_command_not_exists() {
        assert!(!command_exists(
            "this_command_definitely_does_not_exist_12345"
        ));
    }

    #[test]
    fn validate_name_accepts_normal_packages() {
        validate_package_name("typescript", "[npm]").unwrap();
        validate_package_name("@types/node", "[npm]").unwrap();
        validate_package_name("cargo-watch", "[cargo]").unwrap();
        validate_package_name("pytest", "[pip]").unwrap();
        validate_package_name("django-allauth", "[pip]").unwrap();
        validate_package_name("requests", "[pip]").unwrap();
    }

    #[test]
    fn validate_name_rejects_flag_like() {
        for hostile in [
            "--registry=http://attacker",
            "--git",
            "--root",
            "-y",
            "-e/etc/passwd",
        ] {
            let err = validate_package_name(hostile, "[npm]").unwrap_err();
            match err {
                PackageError::RefusedUnsafeSpec(_, _) => {}
                other => panic!("expected RefusedUnsafeSpec for {hostile:?}, got {other:?}"),
            }
        }
    }

    #[test]
    fn validate_name_rejects_url_schemes() {
        for hostile in [
            "git+https://attacker/x.git",
            "https://attacker/x",
            "http://attacker/x",
            "file:///etc/passwd",
            "npm:@scope/foo",
            "./local-evil",
            "../escape",
        ] {
            assert!(matches!(
                validate_package_name(hostile, "[cargo]"),
                Err(PackageError::RefusedUnsafeSpec(_, _))
            ));
        }
    }

    #[test]
    fn validate_name_rejects_control_bytes() {
        // The motivating attack: a hostile jarvy.toml lands an ANSI
        // sequence in the dry-run preview. Refuse before it reaches
        // any println!.
        for hostile in [
            "\u{1b}[2J\u{1b}[H",     // CSI clear screen + home
            "dotnet-\u{1b}[31mEVIL", // CSI SGR in the middle
            "name\u{07}",            // BEL
            "name\u{7f}",            // DEL
            "name\u{0}rest",         // NUL split
            "name\n",                // bare newline
        ] {
            assert!(
                matches!(
                    validate_package_name(hostile, "[nuget]"),
                    Err(PackageError::RefusedUnsafeSpec(_, _))
                ),
                "expected control-byte refusal for {hostile:?}"
            );
        }
    }

    #[test]
    fn validate_version_rejects_control_bytes() {
        for hostile in ["1.0.0\u{1b}[31m", "\u{7}1.0.0", "1.0\n0", "\u{1b}[2J"] {
            assert!(
                matches!(
                    validate_package_version(hostile, "[nuget]"),
                    Err(PackageError::RefusedUnsafeSpec(_, _))
                ),
                "expected control-byte refusal for version {hostile:?}"
            );
        }
    }

    #[test]
    fn validate_name_rejects_unsafe_chars() {
        for hostile in ["pkg;rm -rf /", "pkg|other", "pkg&", "pkg`evil`", "pkg$VAR"] {
            assert!(matches!(
                validate_package_name(hostile, "[npm]"),
                Err(PackageError::RefusedUnsafeSpec(_, _))
            ));
        }
    }

    #[test]
    fn validate_name_rejects_empty() {
        assert!(matches!(
            validate_package_name("", "[npm]"),
            Err(PackageError::RefusedUnsafeSpec(_, _))
        ));
    }

    #[test]
    fn validate_version_accepts_normal_specs() {
        validate_package_version("1.0", "[npm]").unwrap();
        validate_package_version("^1.0.0", "[npm]").unwrap();
        validate_package_version("~1.0", "[npm]").unwrap();
        validate_package_version(">=2.0", "[pip]").unwrap();
        validate_package_version("latest", "[cargo]").unwrap();
        validate_package_version("0.9.0-beta.1", "[cargo]").unwrap();
    }

    #[test]
    fn validate_version_rejects_flags_and_urls() {
        for hostile in [
            "--registry=http://x",
            "-r",
            "git+https://x",
            "https://attacker",
        ] {
            assert!(matches!(
                validate_package_version(hostile, "[cargo]"),
                Err(PackageError::RefusedUnsafeSpec(_, _))
            ));
        }
    }
}
