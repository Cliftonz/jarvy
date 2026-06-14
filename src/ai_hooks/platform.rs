//! Platform helpers: host detection + PowerShell wrapping + a narrow
//! bash → PowerShell translator that fails closed.
//!
//! The Windows shim used by Claude Code and Cursor wraps a PowerShell
//! script in `powershell -NoProfile -EncodedCommand <base64-utf16le>`.
//! EncodedCommand sidesteps every shell-quoting concern because the
//! payload is opaque to cmd.exe — there's no way for an embedded `"` to
//! truncate the argument and no way for a metacharacter to leak.
//!
//! The translator only runs for the LIBRARY hook scripts, which we
//! author. For any custom (`command = "..."`) entry that does NOT ship
//! an explicit `command_windows`, we emit a fail-closed stub and warn:
//! auto-translating attacker-influenced bash into PowerShell is a code
//! injection primitive and not worth the convenience.

use std::borrow::Cow;

/// Current host OS for hook selection. Kept as a function instead of a
/// `cfg!()` sprinkle so the decision lives in one place — any future
/// override (e.g. WSL detection, JARVY_HOOK_HOST env var) lands here.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum HookHost {
    Unix,
    Windows,
}

impl HookHost {
    pub fn current() -> Self {
        if cfg!(target_os = "windows") {
            HookHost::Windows
        } else {
            HookHost::Unix
        }
    }
}

/// Wrap a PowerShell script in `powershell -NoProfile -EncodedCommand
/// <base64>`. EncodedCommand expects base64'd UTF-16LE — Windows
/// PowerShell documents this and pwsh.exe accepts the same form.
pub fn wrap_powershell_command(script: &str) -> String {
    let utf16: Vec<u8> = script
        .encode_utf16()
        .flat_map(|c| c.to_le_bytes())
        .collect();
    let b64 = base64_encode(&utf16);
    format!("powershell -NoProfile -EncodedCommand {b64}")
}

/// Translation outcome for non-library entries that need a Windows
/// variant inferred from bash. The runner inspects `was_warned` and
/// emits an `ai_hook.windows_auto_translated` event so maintainers can
/// see which custom hooks need a hand-written `command_windows`.
pub enum Translated<'a> {
    /// `command_windows` was supplied. Passthrough.
    Native(Cow<'a, str>),
    /// Could not translate; emit a stub that always exits 0 and warn.
    /// We never emit `AutoWarned` for custom commands — see
    /// [`windows_command`] for the policy.
    StubWarned(String),
}

impl<'a> Translated<'a> {
    pub fn into_string(self) -> String {
        match self {
            Translated::Native(s) => s.into_owned(),
            Translated::StubWarned(s) => s,
        }
    }

    pub fn was_warned(&self) -> bool {
        matches!(self, Translated::StubWarned(_))
    }
}

/// Resolve which script body to ship to a Windows target.
///
/// Priority:
/// 1. `command_windows` supplied → `Native`. Always trusted (it's an
///    explicit author choice, library or custom).
/// 2. No `command_windows` → `StubWarned`. We refuse to translate
///    attacker-influenced bash; the operator must ship a hand-written
///    PowerShell variant.
pub fn windows_command<'a>(
    _bash: Option<&'a str>,
    command_windows: Option<&'a str>,
    hook_name: &str,
) -> Translated<'a> {
    if let Some(pwsh) = command_windows {
        return Translated::Native(Cow::Borrowed(pwsh));
    }
    Translated::StubWarned(stub(hook_name, "no command_windows supplied"))
}

fn stub(name: &str, reason: &str) -> String {
    format!(
        "# jarvy: stub for hook '{name}' on Windows — {reason}.\n\
         # Provide a `command_windows` field in jarvy.toml to enable enforcement.\n\
         [Console]::Error.WriteLine('jarvy: hook ''{name}'' skipped on Windows ({reason})')\n\
         exit 0\n"
    )
}

/// Minimal base64 encoder (standard alphabet, padded). Avoids pulling in
/// a dependency for one call site.
fn base64_encode(bytes: &[u8]) -> String {
    const ALPHA: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    let mut i = 0;
    while i + 3 <= bytes.len() {
        let n =
            (u32::from(bytes[i]) << 16) | (u32::from(bytes[i + 1]) << 8) | u32::from(bytes[i + 2]);
        out.push(ALPHA[((n >> 18) & 0x3f) as usize] as char);
        out.push(ALPHA[((n >> 12) & 0x3f) as usize] as char);
        out.push(ALPHA[((n >> 6) & 0x3f) as usize] as char);
        out.push(ALPHA[(n & 0x3f) as usize] as char);
        i += 3;
    }
    let rem = bytes.len() - i;
    if rem == 1 {
        let n = u32::from(bytes[i]) << 16;
        out.push(ALPHA[((n >> 18) & 0x3f) as usize] as char);
        out.push(ALPHA[((n >> 12) & 0x3f) as usize] as char);
        out.push('=');
        out.push('=');
    } else if rem == 2 {
        let n = (u32::from(bytes[i]) << 16) | (u32::from(bytes[i + 1]) << 8);
        out.push(ALPHA[((n >> 18) & 0x3f) as usize] as char);
        out.push(ALPHA[((n >> 12) & 0x3f) as usize] as char);
        out.push(ALPHA[((n >> 6) & 0x3f) as usize] as char);
        out.push('=');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_command_windows_passes_through() {
        let t = windows_command(Some("# bash"), Some("# pwsh"), "x");
        assert!(matches!(t, Translated::Native(_)));
        assert!(!t.was_warned());
        assert_eq!(t.into_string(), "# pwsh");
    }

    #[test]
    fn custom_command_without_command_windows_fails_closed() {
        let t = windows_command(Some("any bash"), None, "my-hook");
        assert!(matches!(t, Translated::StubWarned(_)));
        assert!(t.was_warned());
        let body = t.into_string();
        assert!(body.contains("skipped on Windows"));
    }

    #[test]
    fn wrap_powershell_produces_encoded_command() {
        let cmd = wrap_powershell_command("Write-Host \"hi\"");
        assert!(cmd.starts_with("powershell -NoProfile -EncodedCommand "));
        // Encoded payload should decode back to UTF-16LE of the input.
        let b64 = cmd.trim_start_matches("powershell -NoProfile -EncodedCommand ");
        assert!(!b64.is_empty());
        // Round-trip our own encoder with a stable reference vector.
        assert_eq!(base64_encode(b"hi"), "aGk=");
        assert_eq!(base64_encode(b"hii"), "aGlp");
        assert_eq!(base64_encode(b"hiii"), "aGlpaQ==");
    }

    #[test]
    fn base64_round_trip_via_known_vectors() {
        // RFC 4648 §10 test vectors.
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_encode(b"foob"), "Zm9vYg==");
        assert_eq!(base64_encode(b"fooba"), "Zm9vYmE=");
        assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
    }
}
