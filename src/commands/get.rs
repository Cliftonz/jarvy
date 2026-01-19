//! Get command handler - display configured tools vs installed status

use serde::Serialize;
use std::fs;

use crate::cli::OutputFormat;
use crate::config::Config;
use crate::report::{Status, ToolReport, collect_reports};

#[derive(Serialize)]
pub struct Reports {
    pub tools: Vec<ToolReport>,
}

/// Get color escape code for a tool status
pub fn color_for_status(status: &Status) -> &'static str {
    match status {
        Status::Match => "\x1b[32m",        // green
        Status::Mismatch => "\x1b[33m",     // yellow
        Status::NotInstalled => "\x1b[31m", // red
    }
}

/// Format reports as pretty human-readable output
pub fn pretty_output(reports: &[ToolReport]) -> String {
    let mut s = String::new();
    s.push_str("Tools status\n");
    for r in reports {
        let color = color_for_status(&r.status);
        let reset = "\x1b[0m";
        let status_label = match r.status {
            Status::Match => "match",
            Status::Mismatch => "mismatch",
            Status::NotInstalled => "not_installed",
        };
        let installed = r.installed.as_deref().unwrap_or("-");
        s.push_str(&format!(
            "{}{}{}: expected={}, installed={} [{}]\n",
            color, r.name, reset, r.expected, installed, status_label
        ));
    }
    s
}

/// Run the get command
pub fn run_get(file: &str, output_format: OutputFormat, output: Option<&str>) {
    let config = Config::new(file);
    let reports = collect_reports(&config);

    let content = match output_format {
        OutputFormat::Json => {
            let wrapper = Reports {
                tools: reports.clone(),
            };
            serde_json::to_string_pretty(&wrapper)
                .unwrap_or_else(|e| format!("{{\"error\":\"{}\"}}", e))
        }
        OutputFormat::Yaml => {
            let wrapper = Reports {
                tools: reports.clone(),
            };
            serde_yaml::to_string(&wrapper).unwrap_or_else(|e| format!("error: {}", e))
        }
        OutputFormat::Toml => {
            let wrapper = Reports {
                tools: reports.clone(),
            };
            toml::to_string(&wrapper).unwrap_or_else(|e| format!("error = \"{}\"", e))
        }
        OutputFormat::Pretty => pretty_output(&reports),
    };

    if let Some(path) = output {
        if let Err(e) = fs::write(path, content) {
            eprintln!("Failed to write output: {}", e);
        }
    } else {
        println!("{}", content);
    }
}
