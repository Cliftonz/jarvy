# Jarvy Standard Error Codes

This page lists the standard exit codes used by the Jarvy CLI. Each entry includes a stable anchor so you can link directly to it from your website or documentation.

- Source of truth: the list is defined in code (src/error_codes.rs). This document mirrors those definitions.
- Stable anchors: use the links in the table below (e.g., `#exit_success`).

## Summary Table

| Code | Key                   | Meaning                                                     | Typical remediation                                  | Link |
|-----:|-----------------------|-------------------------------------------------------------|------------------------------------------------------|------|
| 0    | EXIT_SUCCESS          | Command completed successfully.                             | N/A                                                  | [details](#exit_success) |
| 2    | CONFIG_ERROR          | jarvy.toml or jarvy-extension.toml is missing or malformed. | Fix the configuration file(s) and retry.             | [details](#config_error) |
| 3    | PREREQ_MISSING        | Required package manager or dependency is not installed.    | Install the missing prerequisite and rerun.          | [details](#prereq_missing) |
| 4    | NETWORK_TIMEOUT       | Network issue, timeout, or proxy-related failure occurred.  | Check connectivity and proxy settings; retry.        | [details](#network_timeout) |
| 5    | PERMISSION_REQUIRED   | Operation requires elevated privileges (admin/sudo).        | Re-run with admin/sudo or adjust permissions.        | [details](#permission_required) |
| 6    | INCOMPATIBLE_OS_ARCH  | The OS/architecture is unsupported for the requested action.| Adjust the target or use an alternative installer.   | [details](#incompatible_os_arch) |
| 7    | HOOK_FAILED           | A pre_setup, post_install, or post_setup hook script failed.| Check the hook script for errors or use --no-hooks.  | [details](#hook_failed) |

---

<a id="exit_success"></a>
### 0 — EXIT_SUCCESS
- Meaning: Command completed successfully.
- Typical remediation: N/A

<a id="config_error"></a>
### 2 — CONFIG_ERROR
- Meaning: jarvy.toml or jarvy-extension.toml is missing or malformed.
- Typical remediation: Fix the configuration file(s) and retry.

<a id="prereq_missing"></a>
### 3 — PREREQ_MISSING
- Meaning: Required package manager or dependency (e.g., Homebrew) is not installed.
- Typical remediation: Install the missing prerequisite and rerun.

<a id="network_timeout"></a>
### 4 — NETWORK_TIMEOUT
- Meaning: Network issue, timeout, or proxy-related failure occurred.
- Typical remediation: Check connectivity and proxy settings; retry.

<a id="permission_required"></a>
### 5 — PERMISSION_REQUIRED
- Meaning: Operation requires elevated privileges (admin/sudo).
- Typical remediation: Re-run with admin/sudo or adjust permissions.

<a id="incompatible_os_arch"></a>
### 6 — INCOMPATIBLE_OS_ARCH
- Meaning: The current OS/architecture is unsupported for the requested action.
- Typical remediation: Adjust the target or use an alternative installer.

<a id="hook_failed"></a>
### 7 — HOOK_FAILED
- Meaning: A pre_setup, post_install, or post_setup hook script failed.
- Typical remediation: Check the hook script for errors or use `--no-hooks` to skip hooks.
