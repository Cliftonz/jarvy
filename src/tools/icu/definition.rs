//! icu - International Components for Unicode runtime libraries.
//!
//! .NET on Linux dynamically loads ICU at runtime and fails hard
//! ("Couldn't find a valid ICU package installed on the system") if it's
//! missing — this bites `dotnet run` on a fresh Linux/WSL box. Linux-only:
//! macOS ships ICU with the OS and Windows' dotnet installer bundles its own,
//! so neither platform needs jarvy to provision it.
//!
//! Library-only pseudo-tool (no invokable binary to probe), mirroring
//! `vcredist`.

use crate::define_tool;

define_tool!(ICU, {
    command: "icu",
    linux: { apt: "libicu-dev", dnf: "libicu", pacman: "icu", apk: "icu-libs" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icu_registration_shape() {
        assert_eq!(ICU.command, "icu");
        let linux = ICU.linux.unwrap();
        assert_eq!(linux.apt, Some("libicu-dev"));
        assert_eq!(linux.dnf, Some("libicu"));
        assert_eq!(linux.pacman, Some("icu"));
        assert_eq!(linux.apk, Some("icu-libs"));
        assert!(ICU.macos.is_none());
        assert!(ICU.windows.is_none());
        assert!(ICU.bsd.is_none());
    }
}
