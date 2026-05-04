//! Unified security audit command
//!
//! Runs available security scanning tools against the current directory
//! and produces a combined report.

use crate::output::{ExitCode, Outputable, colors, header, subheader};
use crate::tools::common::has;
use serde::Serialize;
use std::process::Command;

/// Known security scanners and their invocations
const SCANNERS: &[ScannerDef] = &[
    ScannerDef {
        name: "betterleaks",
        command: "betterleaks",
        args: &["git", ".", "--no-banner"],
        category: "Secrets",
    },
    ScannerDef {
        name: "gitleaks",
        command: "gitleaks",
        args: &["detect", "--no-banner"],
        category: "Secrets",
    },
    ScannerDef {
        name: "trufflehog",
        command: "trufflehog",
        args: &["filesystem", "."],
        category: "Secrets",
    },
    ScannerDef {
        name: "trivy",
        command: "trivy",
        args: &["fs", "--scanners", "vuln,secret,misconfig", "."],
        category: "Vulnerability",
    },
    ScannerDef {
        name: "grype",
        command: "grype",
        args: &["dir:."],
        category: "Vulnerability",
    },
    ScannerDef {
        name: "semgrep",
        command: "semgrep",
        args: &["scan", "--config", "auto", "."],
        category: "SAST",
    },
    ScannerDef {
        name: "checkov",
        command: "checkov",
        args: &["-d", "."],
        category: "IaC",
    },
    ScannerDef {
        name: "tfsec",
        command: "tfsec",
        args: &["."],
        category: "IaC",
    },
];

struct ScannerDef {
    name: &'static str,
    command: &'static str,
    args: &'static [&'static str],
    category: &'static str,
}

/// Result from a single scanner run
#[derive(Debug, Clone, Serialize)]
pub struct ScannerResult {
    pub name: String,
    pub category: String,
    pub available: bool,
    pub ran: bool,
    pub passed: bool,
    pub exit_code: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
}

/// Combined audit report
#[derive(Debug, Clone, Serialize)]
pub struct AuditReport {
    pub scanners: Vec<ScannerResult>,
    pub total_available: usize,
    pub total_ran: usize,
    pub total_passed: usize,
    pub total_failed: usize,
}

impl Outputable for AuditReport {
    fn to_human(&self) -> String {
        let mut out = String::new();
        out.push_str(&header("Security Audit Report"));
        out.push('\n');

        if self.total_available == 0 {
            out.push_str(&format!(
                "\n{}No security scanners found.{}\n\nInstall scanners via jarvy.toml:\n  [provisioner]\n  betterleaks = \"latest\"\n  trivy = \"latest\"\n",
                colors::YELLOW, colors::RESET
            ));
            return out;
        }

        // Group by category
        for category in &["Secrets", "Vulnerability", "SAST", "IaC"] {
            let in_cat: Vec<_> = self
                .scanners
                .iter()
                .filter(|s| s.category == *category)
                .collect();
            if in_cat.is_empty() {
                continue;
            }

            out.push_str(&subheader(category));
            out.push('\n');

            for s in in_cat {
                if !s.available {
                    out.push_str(&format!(
                        "  {}[SKIP]{} {} (not installed)\n",
                        colors::DIM,
                        colors::RESET,
                        s.name
                    ));
                } else if s.passed {
                    out.push_str(&format!(
                        "  {}[PASS]{} {}\n",
                        colors::GREEN,
                        colors::RESET,
                        s.name
                    ));
                } else {
                    out.push_str(&format!(
                        "  {}[FAIL]{} {} (exit {})\n",
                        colors::RED,
                        colors::RESET,
                        s.name,
                        s.exit_code.unwrap_or(-1)
                    ));
                    if let Some(ref summary) = s.summary {
                        for line in summary.lines().take(5) {
                            out.push_str(&format!("         {}\n", line));
                        }
                    }
                }
            }
        }

        out.push_str(&format!(
            "\n{}Summary:{} {} ran, {} passed, {} failed, {} skipped\n",
            colors::BOLD,
            colors::RESET,
            self.total_ran,
            self.total_passed,
            self.total_failed,
            self.total_available - self.total_ran + (self.scanners.len() - self.total_available)
        ));

        out
    }

    fn exit_code(&self) -> ExitCode {
        if self.total_failed > 0 {
            ExitCode::Error
        } else if self.total_available == 0 {
            ExitCode::Warning
        } else {
            ExitCode::Ok
        }
    }
}

/// Run security audit with available scanners. Scanners are executed in
/// parallel via rayon — they are independent subprocess invocations and
/// previously serialized, making `jarvy audit` walltime = sum of all
/// scanners.
pub fn run_audit(specific_tool: Option<&str>) -> AuditReport {
    use rayon::prelude::*;

    let active: Vec<&'static ScannerDef> = SCANNERS
        .iter()
        .filter(|s| specific_tool.is_none_or(|f| s.name == f))
        .collect();

    let results: Vec<ScannerResult> = active
        .par_iter()
        .map(|scanner| run_one_scanner(scanner))
        .collect();

    let total_available = results.iter().filter(|r| r.available).count();
    let total_ran = results.iter().filter(|r| r.ran).count();
    let total_passed = results.iter().filter(|r| r.passed).count();
    let total_failed = results.iter().filter(|r| r.ran && !r.passed).count();

    AuditReport {
        scanners: results,
        total_available,
        total_ran,
        total_passed,
        total_failed,
    }
}

fn run_one_scanner(scanner: &ScannerDef) -> ScannerResult {
    let span = tracing::info_span!(
        "audit.scanner",
        scanner = %scanner.name,
        category = %scanner.category,
    );
    let _enter = span.enter();

    let available = has(scanner.command);
    if !available {
        tracing::debug!(
            event = "audit.scanner.skipped",
            scanner = %scanner.name,
            reason = "not_installed"
        );
        return ScannerResult {
            name: scanner.name.to_string(),
            category: scanner.category.to_string(),
            available: false,
            ran: false,
            passed: false,
            exit_code: None,
            summary: None,
        };
    }

    let output = Command::new(scanner.command).args(scanner.args).output();
    match output {
        Ok(o) => {
            let code = o.status.code().unwrap_or(-1);
            let passed = o.status.success();
            let stderr = String::from_utf8_lossy(&o.stderr);
            let summary = if !passed && !stderr.is_empty() {
                Some(stderr.lines().take(10).collect::<Vec<_>>().join("\n"))
            } else {
                None
            };
            tracing::info!(
                event = "audit.scanner.complete",
                scanner = %scanner.name,
                passed = passed,
                exit_code = code,
            );
            ScannerResult {
                name: scanner.name.to_string(),
                category: scanner.category.to_string(),
                available: true,
                ran: true,
                passed,
                exit_code: Some(code),
                summary,
            }
        }
        Err(e) => {
            tracing::warn!(
                event = "audit.scanner.failed",
                scanner = %scanner.name,
                error = %e,
            );
            ScannerResult {
                name: scanner.name.to_string(),
                category: scanner.category.to_string(),
                available: true,
                ran: false,
                passed: false,
                exit_code: None,
                summary: Some(format!("Failed to execute: {}", e)),
            }
        }
    }
}
