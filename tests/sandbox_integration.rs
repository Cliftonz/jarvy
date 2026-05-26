//! Integration tests for `src/sandbox` against real Docker containers.
//!
//! Covers the PRD-053 acceptance scenarios that env-isolated unit
//! tests in `src/sandbox/mod.rs` cannot reach:
//!
//! 1. `/.dockerenv` + non-TTY → seamless-mode banner emitted
//! 2. `jarvy setup` verify-only branch exits `PREREQ_MISSING` on gaps
//! 3. `jarvy setup` verify-only branch exits 0 when all tools present
//! 4. Auto-baseline writes `.jarvy/state.json` on a clean seamless run
//!
//! ## How tests find a Linux jarvy binary
//!
//! The binary mounted into each container must be ELF for a Linux
//! arch — mach-o won't exec inside a Linux container. The helper
//! `host_jarvy_path()` resolves the binary in this order:
//!
//! 1. `JARVY_TEST_BIN` env var (absolute path) — explicit override
//! 2. Cargo's `CARGO_BIN_EXE_jarvy` (the binary cargo just built
//!    for the current host target) — used on Linux CI runners
//!
//! On macOS / Apple Silicon, set `JARVY_TEST_BIN` to a cross-built
//! aarch64-linux binary. The `make test-sandbox` target does this
//! for you (it runs `cross build --target aarch64-unknown-linux-gnu
//! --release` and points `JARVY_TEST_BIN` at the result).
//!
//! Apple Silicon hosts run `linux/arm64` containers natively under
//! Docker Desktop, so the harness runs at full speed — no QEMU.
//!
//! Tests skip silently when Docker is unreachable or when the
//! resolved binary is not a Linux ELF (so a stray `cargo test` on
//! macOS without the cross-build setup doesn't paint the suite red).

mod common;

use std::time::Duration;

use common::{docker_available, host_jarvy_path, is_linux_elf};
use testcontainers::core::{AccessMode, CmdWaitFor, ExecCommand, Mount, WaitFor};
use testcontainers::runners::SyncRunner;
use testcontainers::{GenericImage, ImageExt};

// Pinned image digests. Tags like `bookworm-slim` move under the
// operator's feet — pin by digest so a registry push (or a tag-replay
// MITM) can't swap the test image without an explicit code change.
// To bump: `docker pull <image>` then
// `docker inspect <image> --format '{{.RepoDigests}}'`.
// Pinned 2026-05-13 (PRD-053 security review F8).
const DEBIAN_BOOKWORM_SLIM_DIGEST: &str =
    "sha256:67b30a61dc87758f0caf819646104f29ecbda97d920aaf5edc834128ac8493d3";
const BUILDPACK_DEPS_BOOKWORM_SCM_DIGEST: &str =
    "sha256:2c2f3c4c9796456a30812a9b1276615878d5f3a31f982a319b0ef3c6234ea6c0";

// `docker_available`, `host_jarvy_path`, `is_linux_elf` live in
// `tests/common/mod.rs` — shared with `e2e_install_pipeline.rs`.
// `skip_reason` is **intentionally not extracted** (counterweight per
// the PRD-054 maintainability review F2): the install-pipeline file's
// skip rules add a CI loud-fail and a libc check that don't belong in
// the sandbox flow.

/// Skip if Docker is unreachable, the resolved binary is not a Linux
/// ELF, OR `JARVY_TEST_BIN` was not explicitly set. The last check is
/// the one that catches the Tool E2E workflow case: it does
/// `cargo build --release --bin jarvy` on a glibc-2.39 ubuntu-latest
/// runner, then runs `cargo test --tests` which picks up *all* test
/// binaries — including this one. The host-built jarvy exec'd inside
/// the older-glibc testcontainers fails with
/// `libc.so.6: version GLIBC_2.39 not found` and these sandbox tests
/// red the whole job. The dedicated `make test-sandbox` target sets
/// `JARVY_TEST_BIN` to a cross-built linux-gnu jarvy that exec's
/// cleanly; anything else skips with a clear message.
fn skip_reason() -> Option<String> {
    if !docker_available() {
        return Some("docker daemon not reachable".to_string());
    }
    if std::env::var_os("JARVY_TEST_BIN").is_none() {
        return Some(
            "JARVY_TEST_BIN not set — sandbox tests require a cross-built linux-gnu \
             jarvy (try `make test-sandbox`). Skipping to avoid GLIBC mismatch when \
             exec'd inside an older testcontainer."
                .to_string(),
        );
    }
    let bin = host_jarvy_path();
    if !bin.exists() {
        return Some(format!("jarvy binary not found at {}", bin.display()));
    }
    if !is_linux_elf(&bin) {
        return Some(format!(
            "binary at {} is not a Linux ELF — set JARVY_TEST_BIN to a \
             cross-built aarch64-unknown-linux-gnu / x86_64-unknown-linux-gnu \
             jarvy (try `make test-sandbox`)",
            bin.display()
        ));
    }
    None
}

/// Minimal `jarvy.toml` content used by the setup-path tests. `git`
/// is in the base registry and easy to flip between present and
/// missing.
const SETUP_TOML: &str = r#"
[provisioner]
git = "latest"
"#;

/// Two-tool config used by the partial-match negative tests: `git` is
/// present in `buildpack-deps:bookworm-scm` but `jq` is not, so the
/// version check comes back with one needs_install entry and the
/// auto-baseline gate must refuse to write `state.json`.
const SETUP_TOML_TWO_TOOLS: &str = r#"
[provisioner]
git = "latest"
jq = "latest"
"#;

fn write_workspace_toml(dir: &tempfile::TempDir) {
    std::fs::write(dir.path().join("jarvy.toml"), SETUP_TOML).expect("write jarvy.toml");
}

fn write_workspace_toml_with(dir: &tempfile::TempDir, content: &str) {
    std::fs::write(dir.path().join("jarvy.toml"), content).expect("write jarvy.toml");
}

/// Build a baseline debian image with jarvy mounted in and the
/// project config staged. `tag` is appended with the pinned digest
/// (`<tag>@sha256:...`) so Docker resolves to the exact image bytes
/// the suite was tested against, regardless of registry tag drift.
///
/// `digest` carries the `sha256:...` slug for the image; pass `""`
/// for unpinned (test-development only).
///
/// The mounted jarvy binary is bind-mounted **read-only** so a
/// malicious container can't truncate or replace the host binary
/// mid-test (PRD-053 security review F8).
fn container_request(
    image: &str,
    tag: &str,
    digest: &str,
    workdir: &tempfile::TempDir,
    extra_env: &[(&str, &str)],
) -> testcontainers::ContainerRequest<GenericImage> {
    let jarvy = host_jarvy_path();
    let pinned_tag = if digest.is_empty() {
        tag.to_string()
    } else {
        format!("{tag}@{digest}")
    };
    let mut req = GenericImage::new(image, pinned_tag.as_str())
        .with_wait_for(WaitFor::seconds(1))
        .with_cmd(vec!["sleep", "120"])
        .with_mount(
            Mount::bind_mount(jarvy.to_string_lossy().into_owned(), "/usr/local/bin/jarvy")
                .with_access_mode(AccessMode::ReadOnly),
        )
        .with_mount(Mount::bind_mount(
            workdir.path().to_string_lossy().into_owned(),
            "/workspace",
        ))
        // Keep ~/.jarvy on a writable tmpfs path inside the container
        // so the probe + log writers don't blow up on the read-only
        // root jarvy doesn't actually create.
        .with_env_var("JARVY_HOME", "/tmp/.jarvy")
        .with_env_var("HOME", "/tmp");
    for (k, v) in extra_env {
        req = req.with_env_var(*k, *v);
    }
    req
}

fn exec_wait(cmd: Vec<&str>) -> ExecCommand {
    ExecCommand::new(cmd.into_iter().map(String::from).collect::<Vec<_>>())
        .with_cmd_ready_condition(CmdWaitFor::exit())
}

#[test]
fn generic_container_emits_seamless_banner() {
    if let Some(why) = skip_reason() {
        eprintln!("skipping sandbox integration test: {why}");
        return;
    }

    let workdir = tempfile::tempdir().expect("tmpdir");
    write_workspace_toml(&workdir);

    let container = container_request(
        "debian",
        "bookworm-slim",
        DEBIAN_BOOKWORM_SLIM_DIGEST,
        &workdir,
        &[],
    )
    .start()
    .expect("start container");

    // `--dry-run` makes setup non-mutating but still flows through
    // `main()` so the banner emission runs. `/.dockerenv` is
    // present inside any docker container and exec stdin is a pipe
    // (non-TTY), so the generic-container fallback should fire.
    let mut exec = container
        .exec(exec_wait(vec![
            "jarvy",
            "setup",
            "--dry-run",
            "--file",
            "/workspace/jarvy.toml",
        ]))
        .expect("exec jarvy setup --dry-run");

    let stderr = exec.stderr_to_vec().expect("read stderr");
    let stderr = String::from_utf8_lossy(&stderr);
    assert!(
        stderr.contains("seamless mode") && stderr.contains("container"),
        "expected seamless-mode banner mentioning 'container' on stderr; got:\n{stderr}"
    );
}

#[test]
fn verify_only_branch_exits_prereq_missing_on_gap() {
    if let Some(why) = skip_reason() {
        eprintln!("skipping sandbox integration test: {why}");
        return;
    }

    let workdir = tempfile::tempdir().expect("tmpdir");
    write_workspace_toml(&workdir);

    // `bookworm-slim` ships without git. Force the verify-only
    // capability and assert PREREQ_MISSING (3) exit.
    let container = container_request(
        "debian",
        "bookworm-slim",
        DEBIAN_BOOKWORM_SLIM_DIGEST,
        &workdir,
        &[("JARVY_FORCE_VERIFY_ONLY", "1"), ("JARVY_SANDBOX", "1")],
    )
    .start()
    .expect("start container");

    let mut exec = container
        .exec(exec_wait(vec![
            "jarvy",
            "setup",
            "--file",
            "/workspace/jarvy.toml",
        ]))
        .expect("exec jarvy setup");

    let stderr_bytes = exec.stderr_to_vec().expect("read stderr");
    let stderr = String::from_utf8_lossy(&stderr_bytes);
    let exit_code = exec
        .exit_code()
        .expect("exit_code call ok")
        .expect("exit code available");

    assert_eq!(
        exit_code, 3,
        "expected PREREQ_MISSING (3); got {exit_code}. stderr:\n{stderr}"
    );
    assert!(
        stderr.contains("sandbox cannot install tools"),
        "expected verify-only refusal message; got:\n{stderr}"
    );
}

#[test]
fn verify_only_branch_exits_zero_when_all_present() {
    if let Some(why) = skip_reason() {
        eprintln!("skipping sandbox integration test: {why}");
        return;
    }

    let workdir = tempfile::tempdir().expect("tmpdir");
    write_workspace_toml(&workdir);

    // `buildpack-deps:bookworm-scm` is the Debian SCM variant that
    // ships `git` (plus svn/hg/bzr) pre-installed. Plain
    // `debian:bookworm` does *not* — that was a wrong assumption in
    // the PRD draft. The SCM variant is the smallest official Debian
    // image with git baked in.
    let container = container_request(
        "buildpack-deps",
        "bookworm-scm",
        BUILDPACK_DEPS_BOOKWORM_SCM_DIGEST,
        &workdir,
        &[("JARVY_FORCE_VERIFY_ONLY", "1"), ("JARVY_SANDBOX", "1")],
    )
    .start()
    .expect("start container");

    let mut exec = container
        .exec(exec_wait(vec![
            "jarvy",
            "setup",
            "--file",
            "/workspace/jarvy.toml",
        ]))
        .expect("exec jarvy setup");

    let stderr_bytes = exec.stderr_to_vec().expect("read stderr");
    let stderr = String::from_utf8_lossy(&stderr_bytes);
    let exit_code = exec
        .exit_code()
        .expect("exit_code call ok")
        .expect("exit code available");

    assert_eq!(
        exit_code, 0,
        "expected verify-only pass (0); got {exit_code}. stderr:\n{stderr}"
    );
    assert!(
        stderr.contains("verify-only mode passed"),
        "expected verify-only success message; got:\n{stderr}"
    );
}

#[test]
fn auto_baseline_writes_state_file_on_clean_seamless_run() {
    if let Some(why) = skip_reason() {
        eprintln!("skipping sandbox integration test: {why}");
        return;
    }

    let workdir = tempfile::tempdir().expect("tmpdir");
    write_workspace_toml(&workdir);

    // Force the sandbox detector on so seamless mode kicks in
    // regardless of the runner's own env. `buildpack-deps:bookworm-scm`
    // has git pre-installed so version_check comes back clean and the
    // auto-baseline branch in setup_cmd fires. Plain `debian:bookworm`
    // would not — git is not in the base Debian image.
    let container = container_request(
        "buildpack-deps",
        "bookworm-scm",
        BUILDPACK_DEPS_BOOKWORM_SCM_DIGEST,
        &workdir,
        &[("JARVY_SANDBOX", "1")],
    )
    .start()
    .expect("start container");

    let mut exec = container
        .exec(exec_wait(vec![
            "jarvy",
            "setup",
            "--file",
            "/workspace/jarvy.toml",
        ]))
        .expect("exec jarvy setup");

    let stdout_bytes = exec.stdout_to_vec().unwrap_or_default();
    let stderr_bytes = exec.stderr_to_vec().unwrap_or_default();
    let stdout = String::from_utf8_lossy(&stdout_bytes);
    let stderr = String::from_utf8_lossy(&stderr_bytes);
    let exit_code = exec
        .exit_code()
        .expect("exit_code call ok")
        .expect("exit code available");

    assert_eq!(
        exit_code, 0,
        "expected setup success (0); got {exit_code}.\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );

    // state.json is written into the workspace volume mount, which
    // is the same tmpdir the host sees. Brief settle window in case
    // of bind-mount fs sync delay (instant on most kernels; be
    // polite).
    let state_path = workdir.path().join(".jarvy").join("state.json");
    for _ in 0..10 {
        if state_path.exists() {
            break;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    assert!(
        state_path.exists(),
        "expected .jarvy/state.json after auto-baseline.\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );

    let state = std::fs::read_to_string(&state_path).expect("read state.json");
    assert!(
        state.contains("\"tools\""),
        "expected tools key in state.json; got:\n{state}"
    );
}

/// PRD-053 risk row 2: auto-baseline must NOT paper over a partial
/// doctor match. `buildpack-deps:bookworm-scm` has `git` but not
/// `jq`; the two-tool config triggers `needs_install = [jq]`, so the
/// verify-only branch must refuse and emit `PREREQ_MISSING (3)` with
/// no `state.json` written.
#[test]
fn verify_only_does_not_write_state_on_partial_match() {
    if let Some(why) = skip_reason() {
        eprintln!("skipping sandbox integration test: {why}");
        return;
    }

    let workdir = tempfile::tempdir().expect("tmpdir");
    write_workspace_toml_with(&workdir, SETUP_TOML_TWO_TOOLS);

    let container = container_request(
        "buildpack-deps",
        "bookworm-scm",
        BUILDPACK_DEPS_BOOKWORM_SCM_DIGEST,
        &workdir,
        &[("JARVY_FORCE_VERIFY_ONLY", "1"), ("JARVY_SANDBOX", "1")],
    )
    .start()
    .expect("start container");

    let mut exec = container
        .exec(exec_wait(vec![
            "jarvy",
            "setup",
            "--file",
            "/workspace/jarvy.toml",
        ]))
        .expect("exec jarvy setup");

    let stderr_bytes = exec.stderr_to_vec().expect("read stderr");
    let stderr = String::from_utf8_lossy(&stderr_bytes);
    let exit_code = exec
        .exit_code()
        .expect("exit_code call ok")
        .expect("exit code available");

    assert_eq!(
        exit_code, 3,
        "expected PREREQ_MISSING (3) on partial match; got {exit_code}. stderr:\n{stderr}"
    );
    assert!(
        stderr.contains("missing: jq") || stderr.contains("jq"),
        "expected stderr to name the missing tool jq; got:\n{stderr}"
    );

    let state_path = workdir.path().join(".jarvy").join("state.json");
    assert!(
        !state_path.exists(),
        "auto-baseline must not write state.json on partial match (PRD-053 risk row 2)"
    );
}

/// PRD-053 banner suppression: `--json` mutes the seamless-mode
/// stderr banner so JSON consumers don't see it interleaved with
/// stderr-routed structured output. The tracing event still fires
/// (verified via log file in unit tests); this guards the
/// argv-walking suppression logic in `main.rs`.
#[test]
fn banner_suppressed_with_json_flag() {
    if let Some(why) = skip_reason() {
        eprintln!("skipping sandbox integration test: {why}");
        return;
    }

    let workdir = tempfile::tempdir().expect("tmpdir");
    write_workspace_toml(&workdir);

    let container = container_request(
        "debian",
        "bookworm-slim",
        DEBIAN_BOOKWORM_SLIM_DIGEST,
        &workdir,
        &[],
    )
    .start()
    .expect("start container");

    // `--dry-run --format=json` runs the banner-emission code path
    // but should mute it. We're not testing the JSON output shape
    // here — only that the banner is gone.
    let mut exec = container
        .exec(exec_wait(vec![
            "jarvy",
            "setup",
            "--dry-run",
            "--format=json",
            "--file",
            "/workspace/jarvy.toml",
        ]))
        .expect("exec jarvy setup --dry-run --format=json");

    let stderr = exec.stderr_to_vec().expect("read stderr");
    let stderr = String::from_utf8_lossy(&stderr);
    assert!(
        !stderr.contains("seamless mode"),
        "expected banner muted by --format=json; got stderr:\n{stderr}"
    );
}

/// PRD-053 banner suppression: `JARVY_QUIET=1` mutes the banner.
/// Same path as `--json` but env-var-driven; covers wrappers that
/// can't easily inject CLI flags.
#[test]
fn banner_suppressed_with_jarvy_quiet_env() {
    if let Some(why) = skip_reason() {
        eprintln!("skipping sandbox integration test: {why}");
        return;
    }

    let workdir = tempfile::tempdir().expect("tmpdir");
    write_workspace_toml(&workdir);

    let container = container_request(
        "debian",
        "bookworm-slim",
        DEBIAN_BOOKWORM_SLIM_DIGEST,
        &workdir,
        &[("JARVY_QUIET", "1")],
    )
    .start()
    .expect("start container");

    let mut exec = container
        .exec(exec_wait(vec![
            "jarvy",
            "setup",
            "--dry-run",
            "--file",
            "/workspace/jarvy.toml",
        ]))
        .expect("exec jarvy setup --dry-run");

    let stderr = exec.stderr_to_vec().expect("read stderr");
    let stderr = String::from_utf8_lossy(&stderr);
    assert!(
        !stderr.contains("seamless mode"),
        "expected banner muted by JARVY_QUIET=1; got stderr:\n{stderr}"
    );
}

/// Verify-only success path must NOT overwrite an existing
/// `state.json` — auto-baseline is one-shot per project (PRD-053
/// §"Auto-baseline behavior", item B). The integration test prewrites
/// a sentinel `state.json`, runs the verify-only flow, and asserts
/// the bytes survived.
#[test]
fn verify_only_does_not_overwrite_existing_state_json() {
    if let Some(why) = skip_reason() {
        eprintln!("skipping sandbox integration test: {why}");
        return;
    }

    let workdir = tempfile::tempdir().expect("tmpdir");
    write_workspace_toml(&workdir);
    let state_dir = workdir.path().join(".jarvy");
    std::fs::create_dir_all(&state_dir).expect("mkdir .jarvy");
    let state_path = state_dir.join("state.json");
    let sentinel = r#"{"version":"1","sentinel":"do-not-overwrite"}"#;
    std::fs::write(&state_path, sentinel).expect("write sentinel state.json");

    let container = container_request(
        "buildpack-deps",
        "bookworm-scm",
        BUILDPACK_DEPS_BOOKWORM_SCM_DIGEST,
        &workdir,
        &[("JARVY_FORCE_VERIFY_ONLY", "1"), ("JARVY_SANDBOX", "1")],
    )
    .start()
    .expect("start container");

    let exec = container
        .exec(exec_wait(vec![
            "jarvy",
            "setup",
            "--file",
            "/workspace/jarvy.toml",
        ]))
        .expect("exec jarvy setup");

    let exit_code = exec
        .exit_code()
        .expect("exit_code call ok")
        .expect("exit code available");
    assert_eq!(exit_code, 0, "expected verify-only pass; got {exit_code}");

    let after = std::fs::read_to_string(&state_path).expect("read state.json after");
    assert_eq!(
        after, sentinel,
        "verify-only path must not overwrite an existing state.json"
    );
}
