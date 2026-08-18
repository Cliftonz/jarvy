//! E2E coverage for `[git_hooks.native] repo = "..."`.
//!
//! Sets up a local bare git repo in a tempdir with two hook scripts
//! (pre-commit, pre-push), points a project's `jarvy.toml` at it via
//! `git+file://` (allowed under `JARVY_LIBRARY_ALLOW_INSECURE_GIT=1`),
//! runs the `git_hooks::install_hooks` API directly, and asserts
//! both files landed in `.git/hooks/` with the jarvy marker + exec
//! bit set. No network, no real remote — the whole flow is
//! reproducible offline.
//!
//! Uses the `test-bypass` feature so a bare `cargo test` skips it
//! (mirrors `library_registry_git_e2e`). CI runs with
//! `--all-features` so the gate is automatic.

#![cfg(all(unix, feature = "test-bypass"))]

use jarvy::ai_hooks::ConfigOrigin;
use jarvy::git_hooks::config::{HookSource, NativeConfig};
use jarvy::git_hooks::{self, GitHooksConfig, HookFramework};
use serial_test::serial;
use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;

/// Set up a bare git repo at `<tmp>/hooks-remote.git` containing two
/// hook files at the repo root: `pre-commit` and `pre-push`. Returns
/// the file:// URL that can be handed to `git_hooks.native.repo`.
fn setup_bare_repo_with_hooks(tmp: &Path) -> String {
    // 1. Build a working tree with the hook scripts.
    let work = tmp.join("hooks-work");
    fs::create_dir_all(&work).unwrap();
    fs::write(work.join("pre-commit"), "#!/bin/sh\necho e2e-pre-commit\n").unwrap();
    fs::write(work.join("pre-push"), "#!/bin/sh\necho e2e-pre-push\n").unwrap();
    // A non-stage file must be ignored by the scanner, not error.
    fs::write(work.join("README.md"), "hooks lib for e2e test\n").unwrap();

    // 2. `git init`, commit, then clone --bare so we have a URL to
    //    hand out.
    run_git(&["init", "--quiet"], &work);
    run_git(&["config", "user.email", "e2e@example.com"], &work);
    run_git(&["config", "user.name", "E2E"], &work);
    run_git(&["config", "commit.gpgsign", "false"], &work);
    run_git(&["add", "."], &work);
    run_git(&["commit", "--quiet", "-m", "seed"], &work);
    // Force branch to `main` so the ref we pin is deterministic
    // regardless of the runner's git default.
    run_git(&["branch", "-M", "main"], &work);

    let bare = tmp.join("hooks-remote.git");
    let bare_str = bare.to_str().unwrap();
    let work_str = work.to_str().unwrap();
    Command::new("git")
        .args(["clone", "--bare", "--quiet", work_str, bare_str])
        .status()
        .expect("clone --bare");
    format!("git+file://{bare_str}")
}

fn run_git(args: &[&str], cwd: &Path) {
    let status = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .status()
        .unwrap_or_else(|e| panic!("git {args:?}: {e}"));
    assert!(status.success(), "git {args:?} failed");
}

/// Enable the git+file:// bypass ONCE at test-binary startup. Env
/// vars are process-global so removing them in a per-test Drop
/// races parallel siblings; setting once and leaving them is the
/// only sound pattern. JARVY_TEST_HOME needs to be per-test though —
/// each test has its own tempdir — so it's set at call time and
/// left set (subsequent tests overwrite it before they use the cache).
fn scoped_env(tmp: &Path) -> ScopedEnv {
    use std::sync::Once;
    static INIT: Once = Once::new();
    INIT.call_once(|| unsafe {
        std::env::set_var("JARVY_LIBRARY_ALLOW_INSECURE_GIT", "1");
    });
    unsafe {
        std::env::set_var("JARVY_TEST_HOME", tmp);
    }
    ScopedEnv
}

// No Drop — cleanup happens at process exit. See scoped_env doc.
struct ScopedEnv;

fn init_project_git_repo(project: &Path) {
    fs::create_dir_all(project.join(".git").join("hooks")).unwrap();
}

fn cfg_with_repo(repo_url: &str, git_ref: &str) -> GitHooksConfig {
    GitHooksConfig {
        enabled: true,
        framework: Some(HookFramework::Native),
        auto_install: true,
        auto_update: false,
        run_after_install: false,
        allow_remote: false,
        native: Some(NativeConfig {
            repo: Some(repo_url.to_string()),
            git_ref: Some(git_ref.to_string()),
            ..Default::default()
        }),
        origin: ConfigOrigin::Local,
    }
}

// ============================================================================
// The tests
// ============================================================================

#[test]
#[serial]
fn install_clones_repo_and_writes_all_stage_scripts() {
    let tmp = TempDir::new().unwrap();
    let _env = scoped_env(tmp.path());
    let repo_url = setup_bare_repo_with_hooks(tmp.path());

    let project = tmp.path().join("consumer-project");
    init_project_git_repo(&project);

    let cfg = cfg_with_repo(&repo_url, "main");
    git_hooks::install_hooks(&cfg, &project).expect("install must succeed");

    let pre_commit = project.join(".git/hooks/pre-commit");
    let pre_push = project.join(".git/hooks/pre-push");
    assert!(pre_commit.exists(), "pre-commit hook must be installed");
    assert!(pre_push.exists(), "pre-push hook must be installed");

    let pc_body = fs::read_to_string(&pre_commit).unwrap();
    assert!(pc_body.contains("# managed by jarvy"), "marker: {pc_body}");
    assert!(pc_body.contains("e2e-pre-commit"), "body: {pc_body}");

    // README must NOT be installed — dir-scan filter ignores it.
    assert!(!project.join(".git/hooks/README.md").exists());

    // Exec bit set so git can actually invoke the hook.
    let mode = fs::metadata(&pre_commit).unwrap().permissions().mode();
    assert_eq!(
        mode & 0o111,
        0o111,
        "hook must be executable: mode={mode:o}"
    );
}

#[test]
#[serial]
fn install_is_idempotent_across_repeat_runs() {
    let tmp = TempDir::new().unwrap();
    let _env = scoped_env(tmp.path());
    let repo_url = setup_bare_repo_with_hooks(tmp.path());

    let project = tmp.path().join("consumer");
    init_project_git_repo(&project);
    let cfg = cfg_with_repo(&repo_url, "main");

    git_hooks::install_hooks(&cfg, &project).expect("first install");
    let hook_path = project.join(".git/hooks/pre-commit");
    let first_body = fs::read_to_string(&hook_path).unwrap();

    // Second run must not fail (git fetch of the same ref is a
    // no-op, then re-write of jarvy-marker-bearing file succeeds).
    git_hooks::install_hooks(&cfg, &project).expect("second install");
    let second_body = fs::read_to_string(&hook_path).unwrap();

    assert_eq!(first_body, second_body, "idempotent write");
}

#[test]
#[serial]
fn install_refuses_unpinned_repo() {
    // No `ref` field → parse-time error before any clone happens.
    let tmp = TempDir::new().unwrap();
    let _env = scoped_env(tmp.path());
    let project = tmp.path().join("consumer");
    init_project_git_repo(&project);

    let cfg = GitHooksConfig {
        enabled: true,
        framework: Some(HookFramework::Native),
        auto_install: true,
        auto_update: false,
        run_after_install: false,
        allow_remote: false,
        native: Some(NativeConfig {
            repo: Some("git+file:///tmp/does-not-matter".to_string()),
            git_ref: None,
            ..Default::default()
        }),
        origin: ConfigOrigin::Local,
    };
    let err = git_hooks::install_hooks(&cfg, &project).expect_err("must refuse");
    let msg = err.to_string();
    assert!(
        msg.contains("ref") || msg.contains("required"),
        "error must mention missing ref: {msg}"
    );
}

#[test]
#[serial]
fn explicit_inline_hook_overrides_repo_scanned_entry() {
    let tmp = TempDir::new().unwrap();
    let _env = scoped_env(tmp.path());
    let repo_url = setup_bare_repo_with_hooks(tmp.path());

    let project = tmp.path().join("consumer");
    init_project_git_repo(&project);

    let mut overrides = BTreeMap::new();
    overrides.insert(
        "pre-commit".to_string(),
        HookSource::Inline("#!/bin/sh\necho override-wins\n".to_string()),
    );
    let cfg = GitHooksConfig {
        enabled: true,
        framework: Some(HookFramework::Native),
        auto_install: true,
        auto_update: false,
        run_after_install: false,
        allow_remote: false,
        native: Some(NativeConfig {
            repo: Some(repo_url),
            git_ref: Some("main".to_string()),
            hooks: overrides,
            ..Default::default()
        }),
        origin: ConfigOrigin::Local,
    };
    git_hooks::install_hooks(&cfg, &project).expect("install");

    let body = fs::read_to_string(project.join(".git/hooks/pre-commit")).unwrap();
    assert!(
        body.contains("override-wins"),
        "explicit hooks map must win over repo scan: {body}"
    );
    assert!(
        !body.contains("e2e-pre-commit"),
        "repo body must be overridden"
    );
    // pre-push (not overridden) still comes from the repo.
    let pre_push = fs::read_to_string(project.join(".git/hooks/pre-push")).unwrap();
    assert!(pre_push.contains("e2e-pre-push"));
}

#[test]
#[serial]
fn subpath_narrows_scan_to_subdirectory() {
    // Build a repo with hooks under `ci/` — subpath = "ci" should
    // pick them up while ignoring root-level files.
    let tmp = TempDir::new().unwrap();
    let _env = scoped_env(tmp.path());

    let work = tmp.path().join("hooks-work-subpath");
    fs::create_dir_all(work.join("ci")).unwrap();
    fs::write(work.join("ci/pre-commit"), "#!/bin/sh\necho ci-hook\n").unwrap();
    // A file at the ROOT with a stage name — must NOT be installed
    // because subpath narrows to `ci/`.
    fs::write(work.join("pre-push"), "#!/bin/sh\necho root-hook\n").unwrap();
    run_git(&["init", "--quiet"], &work);
    run_git(&["config", "user.email", "e2e@example.com"], &work);
    run_git(&["config", "user.name", "E2E"], &work);
    run_git(&["config", "commit.gpgsign", "false"], &work);
    run_git(&["add", "."], &work);
    run_git(&["commit", "--quiet", "-m", "seed"], &work);
    run_git(&["branch", "-M", "main"], &work);
    let bare = tmp.path().join("hooks-subpath.git");
    Command::new("git")
        .args([
            "clone",
            "--bare",
            "--quiet",
            work.to_str().unwrap(),
            bare.to_str().unwrap(),
        ])
        .status()
        .unwrap();
    let repo_url = format!("git+file://{}", bare.display());

    let project = tmp.path().join("consumer-sub");
    init_project_git_repo(&project);
    let cfg = GitHooksConfig {
        enabled: true,
        framework: Some(HookFramework::Native),
        auto_install: true,
        auto_update: false,
        run_after_install: false,
        allow_remote: false,
        native: Some(NativeConfig {
            repo: Some(repo_url),
            git_ref: Some("main".to_string()),
            subpath: Some("ci".to_string()),
            ..Default::default()
        }),
        origin: ConfigOrigin::Local,
    };
    git_hooks::install_hooks(&cfg, &project).expect("install");

    let pc = fs::read_to_string(project.join(".git/hooks/pre-commit")).unwrap();
    assert!(pc.contains("ci-hook"));
    // Root-level pre-push must NOT have been installed — subpath
    // narrowed the scan to ci/.
    assert!(!project.join(".git/hooks/pre-push").exists());
}

/// Regression guard for the trust boundary: a remote-origin config
/// cannot silently clone + install hooks without `allow_remote = true`.
#[test]
#[serial]
fn remote_origin_config_without_allow_remote_refuses_before_clone() {
    let tmp = TempDir::new().unwrap();
    let _env = scoped_env(tmp.path());
    let repo_url = setup_bare_repo_with_hooks(tmp.path());

    let project = tmp.path().join("consumer");
    init_project_git_repo(&project);

    let mut cfg = cfg_with_repo(&repo_url, "main");
    cfg.origin = ConfigOrigin::Remote;
    cfg.allow_remote = false;
    let err = git_hooks::install_hooks(&cfg, &project).expect_err("must refuse");
    let msg = err.to_string();
    assert!(
        msg.contains("remote") || msg.contains("allow_remote"),
        "error must name the trust gate: {msg}"
    );
    // And no clone should have happened.
    let cache_root = PathBuf::from(tmp.path()).join(".jarvy/git_hooks_cache");
    assert!(
        !cache_root.exists() || fs::read_dir(&cache_root).map(|d| d.count()).unwrap_or(0) == 0,
        "no clone must occur on trust-refused path"
    );
}
