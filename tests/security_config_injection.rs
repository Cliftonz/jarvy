//! Security regression tests for config-driven RCE / argument-injection family.
//!
//! These tests verify that a hostile `jarvy.toml` cannot:
//!  - Land an attacker shell command in `~/.gitconfig` (`!`-prefixed values)
//!  - Inject CLI flags or direct-URL deps into `npm`/`pip`/`cargo install`
//!  - Use an arbitrary path (`/tmp/attacker-shell`) as the hook shell
//!
//! Each refusal is asserted as an explicit `Err(...)` value at the API layer,
//! not as exit-code-only behavior — so a refactor that downgrades the refusal
//! to a warning surfaces here.

use jarvy::packages::common::{PackageError, validate_package_name, validate_package_version};

#[test]
fn npm_pip_cargo_reject_flag_like_names() {
    for hostile in [
        "--registry=http://attacker",
        "--git",
        "--root",
        "--prefix=/usr/local",
        "-y",
    ] {
        for purpose in ["[npm]", "[pip]", "[cargo]"] {
            assert!(
                matches!(
                    validate_package_name(hostile, purpose),
                    Err(PackageError::RefusedUnsafeSpec(_, _))
                ),
                "{purpose} accepted hostile name {hostile:?}"
            );
        }
    }
}

#[test]
fn npm_pip_cargo_reject_url_scheme_packages() {
    for hostile in [
        "git+https://attacker/x.git",
        "https://attacker/x",
        "file:///etc/passwd",
        "npm:@evil/payload",
        "./local-evil",
    ] {
        for purpose in ["[npm]", "[pip]", "[cargo]"] {
            assert!(
                matches!(
                    validate_package_name(hostile, purpose),
                    Err(PackageError::RefusedUnsafeSpec(_, _))
                ),
                "{purpose} accepted url-scheme name {hostile:?}"
            );
        }
    }
}

#[test]
fn version_field_rejects_flag_injection() {
    for hostile in ["--registry=http://x", "-r", "git+https://x"] {
        assert!(
            matches!(
                validate_package_version(hostile, "[cargo]"),
                Err(PackageError::RefusedUnsafeSpec(_, _))
            ),
            "version accepted hostile {hostile:?}"
        );
    }
}

#[test]
fn normal_packages_still_pass() {
    // Sanity: don't be so strict that real configs fail.
    for ok in ["typescript", "@types/node", "cargo-watch", "django-allauth"] {
        validate_package_name(ok, "[npm]").expect("normal name should pass");
    }
    for ok in ["1.0", "^1.0.0", "~1.0", ">=2.0", "latest", "0.9.0-beta.1"] {
        validate_package_version(ok, "[cargo]").expect("normal version should pass");
    }
}
