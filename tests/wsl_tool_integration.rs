//! End-to-end config + pure-logic tests for the WSL2 bridge.
//!
//! These run on every OS in CI. Real `wsl.exe`, `wsl --install`,
//! rootfs download, and the `IsUserAnAdmin` probe are only reachable
//! on Windows and are covered by manual verification against a
//! Windows 11 VM (see `plans/go-with-b-and-giggly-rain.md`).

use std::path::{Path, PathBuf};

use jarvy::tools::wsl::bootstrap::{
    BootstrapContext, BootstrapDecision, argv_wsl_delegate_setup, argv_wsl_import,
    argv_wsl_install, argv_wsl_jarvy_install, render_refusal, render_remove_refusal,
    shell_single_quote, wsl_should_bootstrap,
};
use jarvy::tools::wsl::refusal::RefusalReason;
use jarvy::tools::wsl::rootfs;
use jarvy::windows::{
    PathTranslateError, WindowsConfig, WslConfig, translate_win_path, validate_channel,
    validate_distro_name, validate_distro_slug, validate_install_location,
};

fn base_cfg() -> WslConfig {
    WslConfig {
        enabled: true,
        distro: "Ubuntu".to_string(),
        name: None,
        auto_bootstrap: true,
        install_location: "C:\\wsl\\jarvy".to_string(),
        install_jarvy: true,
        jarvy_channel: "stable".to_string(),
        run_setup: false,
    }
}

fn base_ctx() -> BootstrapContext {
    BootstrapContext {
        wsl_present: true,
        store_based: true,
        target_present: false,
        elevated: true,
    }
}

//
// -- Config parse + defaults ------------------------------------------
//

#[test]
fn windows_config_parses_wsl_block_with_defaults() {
    let toml_str = r#"
[wsl]
enabled = true
"#;
    let cfg: WindowsConfig = toml::from_str(toml_str).unwrap();
    let wsl = cfg.wsl.expect("wsl block present");
    assert!(wsl.enabled);
    assert_eq!(wsl.distro, "Ubuntu"); // default
    assert_eq!(wsl.jarvy_channel, "stable"); // default
    assert!(wsl.install_jarvy, "install_jarvy defaults on when enabled");
    assert!(!wsl.auto_bootstrap, "auto_bootstrap defaults off");
    assert!(!wsl.run_setup, "run_setup defaults off");
    assert!(wsl.validate().is_ok());
}

#[test]
fn wsl_absent_by_default() {
    let cfg: WindowsConfig = toml::from_str("").unwrap();
    assert!(cfg.wsl.is_none());
}

#[test]
fn wsl_effective_name_defaults_to_distro() {
    let cfg = WslConfig {
        distro: "Ubuntu".to_string(),
        name: None,
        ..Default::default()
    };
    assert_eq!(cfg.effective_name(), "Ubuntu");
}

#[test]
fn wsl_effective_name_uses_override() {
    let cfg = WslConfig {
        distro: "Ubuntu".to_string(),
        name: Some("jarvy-ubuntu".to_string()),
        ..Default::default()
    };
    assert_eq!(cfg.effective_name(), "jarvy-ubuntu");
}

//
// -- Validators -------------------------------------------------------
//

#[test]
fn only_ubuntu_and_debian_in_v1() {
    assert!(validate_distro_slug("Ubuntu").is_ok());
    assert!(validate_distro_slug("Debian").is_ok());
    assert!(validate_distro_slug("Fedora").is_err());
    assert!(validate_distro_slug("Kali").is_err());
    assert!(validate_distro_slug("Alpine").is_err());
}

#[test]
fn distro_name_shell_meta_refused() {
    assert!(validate_distro_name("foo; rm").is_err());
    assert!(validate_distro_name("foo --arg").is_err());
    assert!(validate_distro_name("foo\0evil").is_err());
    assert!(validate_distro_name("foo\"bar").is_err());
    assert!(validate_distro_name("foo bar").is_err());
    assert!(validate_distro_name("foo|bar").is_err());
    assert!(validate_distro_name("foo`cmd`").is_err());
    assert!(validate_distro_name("foo$var").is_err());
}

#[test]
fn distro_name_docker_desktop_reserved() {
    assert!(validate_distro_name("docker-desktop").is_err());
    assert!(validate_distro_name("docker-desktop-data").is_err());
    assert!(
        validate_distro_name("Docker-Desktop").is_err(),
        "case-insensitive"
    );
}

#[test]
fn install_location_absolute_pass_unc_relative_fail() {
    assert!(validate_install_location("auto").is_ok());
    assert!(validate_install_location("C:\\wsl\\jarvy").is_ok());
    assert!(validate_install_location("D:/tools/wsl").is_ok());
    assert!(validate_install_location("%LOCALAPPDATA%\\jarvy\\wsl").is_ok());
    assert!(validate_install_location("\\\\server\\share").is_err());
    assert!(validate_install_location("//server/share").is_err());
    assert!(validate_install_location("wsl").is_err());
    assert!(validate_install_location("./wsl").is_err());
    assert!(validate_install_location("\\foo").is_err());
    assert!(validate_install_location("").is_err());
}

#[test]
fn install_location_refuses_bad_chars() {
    assert!(validate_install_location("C:\\wsl\0evil").is_err());
    assert!(validate_install_location("C:\\wsl\"bad").is_err());
    assert!(validate_install_location("C:\\wsl\nrm -rf /").is_err());
}

#[test]
fn channel_bounded_case_insensitive() {
    for good in ["stable", "beta", "nightly", "STABLE"] {
        assert!(validate_channel(good).is_ok(), "should accept {good}");
    }
    for bad in ["edge", "canary", "", "stable; rm"] {
        assert!(validate_channel(bad).is_err(), "should refuse {bad}");
    }
}

//
// -- Path translator --------------------------------------------------
//

#[test]
fn path_translator_maps_drive_letter() {
    assert_eq!(
        translate_win_path(Path::new(r"C:\Users\me\proj")).unwrap(),
        "/mnt/c/Users/me/proj"
    );
    assert_eq!(
        translate_win_path(Path::new(r"D:\Projects\jarvy")).unwrap(),
        "/mnt/d/Projects/jarvy"
    );
}

#[test]
fn path_translator_refuses_unc() {
    assert_eq!(
        translate_win_path(Path::new(r"\\server\share\proj")).unwrap_err(),
        PathTranslateError::Unc
    );
}

#[test]
fn path_translator_refuses_relative() {
    assert_eq!(
        translate_win_path(Path::new("proj/foo")).unwrap_err(),
        PathTranslateError::NotAbsolute
    );
}

#[test]
fn path_translator_refuses_null_bytes() {
    let err = translate_win_path(Path::new("C:\\proj\0evil")).unwrap_err();
    assert_eq!(err, PathTranslateError::InvalidChars);
}

//
// -- Trust-gate + decision matrix -------------------------------------
//

#[test]
fn decision_refuses_when_opted_out() {
    let cfg = WslConfig::default();
    assert_eq!(
        wsl_should_bootstrap(&cfg, base_ctx()),
        BootstrapDecision::Refuse(RefusalReason::NotOptedIn)
    );
}

#[test]
fn decision_refuses_when_no_wsl_command() {
    let ctx = BootstrapContext {
        wsl_present: false,
        ..base_ctx()
    };
    assert_eq!(
        wsl_should_bootstrap(&base_cfg(), ctx),
        BootstrapDecision::Refuse(RefusalReason::NoWslCommand)
    );
}

#[test]
fn decision_refuses_legacy_wsl() {
    let ctx = BootstrapContext {
        store_based: false,
        ..base_ctx()
    };
    assert_eq!(
        wsl_should_bootstrap(&base_cfg(), ctx),
        BootstrapDecision::Refuse(RefusalReason::NotStoreWsl)
    );
}

#[test]
fn decision_noop_when_target_present() {
    let ctx = BootstrapContext {
        target_present: true,
        ..base_ctx()
    };
    assert_eq!(
        wsl_should_bootstrap(&base_cfg(), ctx),
        BootstrapDecision::NoOp
    );
}

#[test]
fn decision_refuses_auto_bootstrap_off() {
    let cfg = WslConfig {
        auto_bootstrap: false,
        ..base_cfg()
    };
    assert_eq!(
        wsl_should_bootstrap(&cfg, base_ctx()),
        BootstrapDecision::Refuse(RefusalReason::AutoBootstrapOff)
    );
}

#[test]
fn decision_refuses_when_not_elevated() {
    let ctx = BootstrapContext {
        elevated: false,
        ..base_ctx()
    };
    assert_eq!(
        wsl_should_bootstrap(&base_cfg(), ctx),
        BootstrapDecision::Refuse(RefusalReason::NotElevated)
    );
}

#[test]
fn decision_runs_install_when_all_gates_pass() {
    assert_eq!(
        wsl_should_bootstrap(&base_cfg(), base_ctx()),
        BootstrapDecision::RunInstall
    );
}

//
// -- Argv builders ----------------------------------------------------
//

#[test]
fn argv_install_matches_expected_shape() {
    let argv = argv_wsl_install("Ubuntu", "Ubuntu", &PathBuf::from(r"C:\wsl\Ubuntu"));
    assert_eq!(
        argv,
        vec![
            "--install",
            "-d",
            "Ubuntu",
            "--name",
            "Ubuntu",
            "--no-launch",
            "--location",
            r"C:\wsl\Ubuntu",
        ]
    );
}

#[test]
fn argv_import_matches_expected_shape() {
    let argv = argv_wsl_import(
        "jarvy-ubuntu",
        &PathBuf::from(r"C:\wsl\jarvy-ubuntu"),
        &PathBuf::from(r"C:\cache\rootfs.tar.gz"),
    );
    assert_eq!(
        argv,
        vec![
            "--import",
            "jarvy-ubuntu",
            r"C:\wsl\jarvy-ubuntu",
            r"C:\cache\rootfs.tar.gz",
        ]
    );
}

#[test]
fn argv_jarvy_install_runs_as_root_with_channel() {
    let argv = argv_wsl_jarvy_install("Ubuntu", "stable");
    assert_eq!(argv[0], "-d");
    assert_eq!(argv[1], "Ubuntu");
    assert_eq!(argv[2], "-u");
    assert_eq!(argv[3], "root");
    assert_eq!(argv[4], "--");
    assert_eq!(argv[5], "bash");
    assert_eq!(argv[6], "-lc");
    assert!(argv[7].contains("curl -fsSL"));
    assert!(argv[7].contains("--channel stable"));
}

#[test]
fn argv_delegate_setup_bare() {
    let argv = argv_wsl_delegate_setup("Ubuntu", "/mnt/c/proj", None);
    assert_eq!(
        argv,
        vec![
            "-d",
            "Ubuntu",
            "--",
            "bash",
            "-lc",
            "cd '/mnt/c/proj' && jarvy setup",
        ]
    );
}

#[test]
fn argv_delegate_setup_propagates_from_url() {
    let argv = argv_wsl_delegate_setup(
        "Ubuntu",
        "/mnt/c/proj",
        Some("https://example.com/config.toml"),
    );
    assert_eq!(
        argv[5],
        "cd '/mnt/c/proj' && jarvy setup --from 'https://example.com/config.toml'"
    );
}

#[test]
fn shell_single_quote_escapes_correctly() {
    assert_eq!(shell_single_quote("plain"), "'plain'");
    assert_eq!(shell_single_quote("has'apo"), "'has'\\''apo'");
}

//
// -- Refusal messages -------------------------------------------------
//

#[test]
fn not_store_wsl_refusal_avoids_dism_and_shows_manual_command() {
    let cfg = base_cfg();
    let msg = render_refusal(&cfg, RefusalReason::NotStoreWsl);
    assert!(msg.contains("DISM"));
    assert!(msg.contains("wsl --install"));
}

#[test]
fn auto_bootstrap_off_refusal_includes_full_install_command() {
    let cfg = base_cfg();
    let msg = render_refusal(&cfg, RefusalReason::AutoBootstrapOff);
    assert!(msg.contains("wsl --install"));
    assert!(msg.contains("--name Ubuntu"));
    assert!(msg.contains("-d Ubuntu"));
}

#[test]
fn not_elevated_refusal_includes_install_command() {
    let cfg = base_cfg();
    let msg = render_refusal(&cfg, RefusalReason::NotElevated);
    assert!(msg.contains("wsl --install"));
    assert!(msg.contains("elevated"));
}

#[test]
fn remove_refusal_prints_unregister_command() {
    let cfg = base_cfg();
    let msg = render_remove_refusal(&cfg);
    assert!(msg.contains("wsl --unregister"));
    assert!(msg.contains("Ubuntu"));
}

#[test]
fn remove_refusal_uses_effective_name_when_overridden() {
    let cfg = WslConfig {
        distro: "Ubuntu".to_string(),
        name: Some("jarvy-ubuntu".to_string()),
        ..Default::default()
    };
    let msg = render_remove_refusal(&cfg);
    assert!(msg.contains("wsl --unregister jarvy-ubuntu"));
}

//
// -- Rootfs cache -----------------------------------------------------
//

#[test]
fn rootfs_pinned_for_v1_distros() {
    assert!(rootfs::lookup("Ubuntu").is_some());
    assert!(rootfs::lookup("Debian").is_some());
    assert!(rootfs::lookup("Fedora").is_none());
}

#[test]
fn rootfs_filename_is_deterministic() {
    let entry = rootfs::lookup("Ubuntu").unwrap();
    let a = rootfs::cache_filename(entry);
    let b = rootfs::cache_filename(entry);
    assert_eq!(a, b);
    assert!(a.starts_with("ubuntu-"));
    assert!(a.ends_with(".tar.gz"));
}

#[test]
fn rootfs_filename_differs_across_distros() {
    let ubuntu = rootfs::cache_filename(rootfs::lookup("Ubuntu").unwrap());
    let debian = rootfs::cache_filename(rootfs::lookup("Debian").unwrap());
    assert_ne!(ubuntu, debian);
}
