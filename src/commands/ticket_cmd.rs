//! Handler for the `jarvy ticket` command
//!
//! Generate debug tickets for support.

use std::path::PathBuf;

use crate::cli::TicketAction;
use crate::logging;
use crate::ticket::{self, TicketBundler, TicketCollector, TicketScope, preview_ticket};

/// Handle ticket command dispatch
pub fn run_ticket_command(action: TicketAction) -> i32 {
    match action {
        TicketAction::Create {
            tool,
            logs,
            output,
            dry_run,
            output_format,
        } => handle_ticket_create(tool, logs, output, dry_run, &output_format),
        TicketAction::Show {
            ticket,
            output_format,
        } => handle_ticket_show(&ticket, &output_format),
        TicketAction::List { output_format } => handle_ticket_list(&output_format),
        TicketAction::Clean {
            older_than,
            output_format,
        } => handle_ticket_clean(older_than, &output_format),
    }
}

/// Create a new debug ticket
fn handle_ticket_create(
    tool: Option<String>,
    log_lines: usize,
    output_path: Option<String>,
    dry_run: bool,
    output_format: &str,
) -> i32 {
    if output_format != "json" {
        println!("Collecting diagnostic information...\n");
    }

    // Determine scope based on options
    let mut scope = if let Some(ref t) = tool {
        TicketScope::for_tool(t)
    } else {
        TicketScope::full()
    };
    scope.log_lines = log_lines;

    // Collect data
    let collector = TicketCollector::new(scope);
    let ticket_data = match collector.collect() {
        Ok(data) => data,
        Err(e) => {
            if output_format == "json" {
                println!(
                    "{}",
                    serde_json::json!({"status": "error", "error": e.to_string()})
                );
            } else {
                eprintln!("Error collecting ticket data: {}", e);
            }
            return 1;
        }
    };

    if dry_run {
        if output_format == "json" {
            println!(
                "{}",
                serde_json::json!({
                    "status": "dry_run",
                    "ticket_id": ticket_data.ticket_id,
                    "preview": preview_ticket(&ticket_data),
                })
            );
        } else {
            let preview = preview_ticket(&ticket_data);
            println!("{}", preview);
            println!("\nDry run - no ticket file created.");
        }
        return 0;
    }

    // Create the bundle
    let bundler = if let Some(ref path) = output_path {
        let output_dir = PathBuf::from(path);
        TicketBundler::with_output_dir(output_dir)
    } else {
        TicketBundler::new()
    };

    match bundler.bundle(&ticket_data) {
        Ok(path) => {
            let size_bytes = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
            if output_format == "json" {
                println!(
                    "{}",
                    serde_json::json!({
                        "status": "created",
                        "ticket_id": ticket_data.ticket_id,
                        "path": path.display().to_string(),
                        "size_bytes": size_bytes,
                    })
                );
            } else {
                println!("Debug ticket created successfully!");
                println!("\n  Ticket ID: {}", ticket_data.ticket_id);
                println!("  Location: {}", path.display());
                println!("  Size: {}", logging::format_size(size_bytes));
                let preview = preview_ticket(&ticket_data);
                println!("\n{}", preview);
                println!("\nShare this ticket file when reporting issues.");
            }
            0
        }
        Err(e) => {
            if output_format == "json" {
                println!(
                    "{}",
                    serde_json::json!({"status": "error", "error": e.to_string()})
                );
            } else {
                eprintln!("Error creating ticket: {}", e);
            }
            1
        }
    }
}

/// Show contents of a ticket
fn handle_ticket_show(ticket_arg: &str, output_format: &str) -> i32 {
    // Try to find the ticket
    let ticket_path = if ticket_arg.ends_with(".zip") {
        PathBuf::from(ticket_arg)
    } else {
        // Try to find by ticket ID
        let tickets_dir = ticket::default_tickets_directory();
        tickets_dir.join(format!("{}.zip", ticket_arg))
    };

    if !ticket_path.exists() {
        if output_format == "json" {
            println!(
                "{}",
                serde_json::json!({"status": "not_found", "path": ticket_path.display().to_string()})
            );
        } else {
            eprintln!("Ticket not found: {}", ticket_path.display());
        }
        return 1;
    }

    // Open and read the ZIP file
    let file = match std::fs::File::open(&ticket_path) {
        Ok(f) => f,
        Err(e) => {
            if output_format == "json" {
                println!(
                    "{}",
                    serde_json::json!({"status": "error", "error": e.to_string()})
                );
            } else {
                eprintln!("Error opening ticket: {}", e);
            }
            return 1;
        }
    };

    let mut archive = match zip::ZipArchive::new(file) {
        Ok(a) => a,
        Err(e) => {
            if output_format == "json" {
                println!(
                    "{}",
                    serde_json::json!({"status": "error", "error": e.to_string()})
                );
            } else {
                eprintln!("Error reading ticket archive: {}", e);
            }
            return 1;
        }
    };

    if output_format == "json" {
        let mut entries: Vec<serde_json::Value> = Vec::new();
        for i in 0..archive.len() {
            if let Ok(f) = archive.by_index(i) {
                entries.push(serde_json::json!({
                    "name": f.name(),
                    "size": f.size(),
                }));
            }
        }
        let manifest: Option<serde_json::Value> =
            if let Ok(mut mf) = archive.by_name("manifest.json") {
                use std::io::Read;
                let mut contents = String::new();
                if mf.read_to_string(&mut contents).is_ok() {
                    serde_json::from_str(&contents).ok()
                } else {
                    None
                }
            } else {
                None
            };
        let json = serde_json::json!({
            "path": ticket_path.display().to_string(),
            "entries": entries,
            "manifest": manifest,
        });
        println!(
            "{}",
            serde_json::to_string_pretty(&json).unwrap_or_else(|_| json.to_string())
        );
        return 0;
    }

    println!("Ticket: {}\n", ticket_path.display());
    println!("Contents:");

    for i in 0..archive.len() {
        if let Ok(file) = archive.by_index(i) {
            // Zip entry names from a third-party ticket are
            // attacker-controllable. Strip ANSI / C0 controls before
            // emitting to a TTY (review item P2 #21). The JSON path
            // doesn't need this because serde_json escapes them.
            let safe_name = crate::observability::sanitizer::redact_for_display(file.name());
            println!("  {} ({} bytes)", safe_name, file.size());
        }
    }

    // Try to read and display manifest.json
    if let Ok(mut manifest_file) = archive.by_name("manifest.json") {
        use std::io::Read;
        let mut contents = String::new();
        if manifest_file.read_to_string(&mut contents).is_ok() {
            if let Ok(manifest) = serde_json::from_str::<serde_json::Value>(&contents) {
                println!("\nManifest:");
                if let Some(ticket_id) = manifest.get("ticket_id") {
                    println!("  Ticket ID: {}", ticket_id);
                }
                if let Some(created) = manifest.get("created_at") {
                    println!("  Created: {}", created);
                }
                if let Some(version) = manifest.get("jarvy_version") {
                    println!("  Jarvy Version: {}", version);
                }
            }
        }
    }

    0
}

/// List existing tickets
fn handle_ticket_list(output_format: &str) -> i32 {
    match ticket::list_tickets() {
        Ok(tickets) => {
            if output_format == "json" {
                let json = serde_json::json!({
                    "tickets_directory": ticket::default_tickets_directory().display().to_string(),
                    "tickets": tickets.iter().map(|(name, path, size)| {
                        serde_json::json!({
                            "name": name,
                            "path": path.display().to_string(),
                            "size_bytes": size,
                        })
                    }).collect::<Vec<_>>(),
                });
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json).unwrap_or_else(|_| json.to_string())
                );
                return 0;
            }

            if tickets.is_empty() {
                println!("No tickets found.");
                println!("Run `jarvy ticket create` to generate a debug ticket.");
                return 0;
            }

            println!("Existing tickets:\n");
            for (name, path, size) in tickets {
                println!(
                    "  {} ({}) - {}",
                    name,
                    logging::format_size(size),
                    path.display()
                );
            }

            let tickets_dir = ticket::default_tickets_directory();
            println!("\nTickets directory: {}", tickets_dir.display());

            0
        }
        Err(e) => {
            if output_format == "json" {
                println!(
                    "{}",
                    serde_json::json!({"status": "error", "error": e.to_string()})
                );
            } else {
                eprintln!("Error listing tickets: {}", e);
            }
            1
        }
    }
}

/// Clean old tickets
fn handle_ticket_clean(older_than: u32, output_format: &str) -> i32 {
    if output_format != "json" {
        println!("Cleaning tickets older than {} days...\n", older_than);
    }

    match ticket::clean_tickets(older_than) {
        Ok((count, bytes)) => {
            if output_format == "json" {
                println!(
                    "{}",
                    serde_json::json!({
                        "removed_count": count,
                        "removed_bytes": bytes,
                        "older_than_days": older_than,
                    })
                );
            } else if count > 0 {
                println!(
                    "Removed {} tickets ({})",
                    count,
                    logging::format_size(bytes)
                );
            } else {
                println!("No old tickets to remove.");
            }
            0
        }
        Err(e) => {
            if output_format == "json" {
                println!(
                    "{}",
                    serde_json::json!({"status": "error", "error": e.to_string()})
                );
            } else {
                eprintln!("Error cleaning tickets: {}", e);
            }
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ticket_list() {
        // Should not panic even with no tickets
        let _result = handle_ticket_list("pretty");
    }

    #[test]
    fn test_ticket_list_json_path_works() {
        // JSON path also must not panic.
        let _result = handle_ticket_list("json");
    }
}
