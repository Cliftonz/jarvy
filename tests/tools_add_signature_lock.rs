//! Signature-lock regression guard for `jarvy::tools::add` and its
//! immediate collaborators (`InstallContext`, `InstallError`).
//!
//! Rationale: `tools::add` is called from several integration test
//! binaries (`tools_install.rs`, `tools_jq_install.rs`,
//! `tools_parallel_install.rs`). A signature bump on `tools::add` that
//! misses ONE of those call sites compiles the bin target cleanly and
//! blows up on CI (`cargo test --all-features --all-targets`). This
//! file exists so that ANY signature change forces a visible edit here
//! — the failure surfaces on the first `cargo check --tests` locally
//! rather than 20 minutes into CI.
//!
//! When the signature legitimately changes, update this file to match
//! and cross-reference EVERY other `jarvy::tools::add` caller in the
//! `tests/` directory.

use jarvy::tools;
use jarvy::tools::common::{InstallContext, InstallError};

/// Compile-time pin: `tools::add` must accept `(name: &str, version:
/// &str, ctx: &InstallContext) -> Result<(), InstallError>`. A future
/// signature change forces this file to be edited, at which point the
/// author must also grep `tests/` for other call sites.
#[test]
fn tools_add_signature_is_locked_to_three_arg_form() {
    // Exercises the signature via a real call. The "definitely_missing
    // _tool" name resolves to `Err(InstallError::Parse("unknown tool"))`
    // per `registry::add`'s fall-through, which is stable and doesn't
    // require any package manager to be present on the runner.
    let ctx = InstallContext::none();
    let result = tools::add("definitely_missing_tool_signature_lock", "", &ctx);
    match result {
        Ok(()) => panic!("unexpected success — signature-lock test relies on unknown-tool refusal"),
        Err(InstallError::Parse(_)) => { /* expected */ }
        Err(other) => panic!(
            "signature-lock test expected InstallError::Parse for an unknown tool, got {other:?}"
        ),
    }
}

/// Pin the `InstallContext::none()` shape — a future collapse into
/// `Default::default()` OR a rename of the constructor would silently
/// let `InstallContext::none()` callers rot. Same rationale as the
/// `tools::add` pin above.
#[test]
fn install_context_none_constructor_still_exists() {
    let ctx = InstallContext::none();
    assert!(
        ctx.distribution.is_none(),
        "InstallContext::none() must default distribution to None"
    );
    assert!(
        ctx.fallback,
        "InstallContext::none() must default fallback to true"
    );
}
