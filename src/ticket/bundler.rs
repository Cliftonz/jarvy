//! ZIP archive bundler for tickets
//!
//! Creates compressed archives containing ticket data.

use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

use zip::ZipWriter;
use zip::write::FileOptions;

use super::{TicketData, TicketError, default_tickets_directory};

/// Ticket bundler that creates ZIP archives
pub struct TicketBundler {
    /// Output directory for tickets
    output_dir: PathBuf,
}

impl TicketBundler {
    /// Create a new bundler with default output directory
    pub fn new() -> Self {
        Self {
            output_dir: default_tickets_directory(),
        }
    }

    /// Create a bundler with custom output directory
    pub fn with_output_dir(output_dir: PathBuf) -> Self {
        Self { output_dir }
    }

    /// Bundle ticket data into a ZIP archive
    ///
    /// Returns the path to the created ZIP file.
    pub fn bundle(&self, ticket: &TicketData) -> Result<PathBuf, TicketError> {
        // Ensure output directory exists
        std::fs::create_dir_all(&self.output_dir)?;

        // Create the ZIP file path
        let zip_path = self.output_dir.join(format!("{}.zip", ticket.ticket_id));

        // Create the ZIP file. Wrap in BufWriter so the per-entry writes
        // ZipWriter performs don't translate to one syscall each.
        let file = File::create(&zip_path)
            .map_err(|e| TicketError::ArchiveCreationFailed(e.to_string()))?;
        let buffered = std::io::BufWriter::with_capacity(64 * 1024, file);
        let mut zip = ZipWriter::new(buffered);

        let options = FileOptions::<()>::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .unix_permissions(0o644);

        // Write manifest.json with ticket metadata
        let manifest = serde_json::json!({
            "ticket_id": ticket.ticket_id,
            "created_at": ticket.created_at,
            "jarvy_version": ticket.jarvy_version,
            "contents": {
                "system": ticket.system.is_some(),
                "tools_count": ticket.tools.len(),
                "config": ticket.config.is_some(),
                "environment_vars": ticket.environment.len(),
                "log_lines": ticket.logs.len(),
            }
        });
        self.write_json_file(&mut zip, "manifest.json", &manifest, options)?;

        // Write system.json if present
        if let Some(ref system) = ticket.system {
            self.write_json_file(&mut zip, "system.json", system, options)?;
        }

        // Write tools.json if present
        if !ticket.tools.is_empty() {
            self.write_json_file(&mut zip, "tools.json", &ticket.tools, options)?;
        }

        // Write config.json if present
        if let Some(ref config) = ticket.config {
            self.write_json_file(&mut zip, "config.json", config, options)?;
        }

        // Write environment.json if present
        if !ticket.environment.is_empty() {
            self.write_json_file(&mut zip, "environment.json", &ticket.environment, options)?;
        }

        // Write logs.txt if present
        if !ticket.logs.is_empty() {
            zip.start_file("logs.txt", options)
                .map_err(|e| TicketError::ArchiveCreationFailed(e.to_string()))?;
            for line in &ticket.logs {
                zip.write_all(line.as_bytes())
                    .map_err(|e| TicketError::ArchiveCreationFailed(e.to_string()))?;
                zip.write_all(b"\n")
                    .map_err(|e| TicketError::ArchiveCreationFailed(e.to_string()))?;
            }
        }

        // Write complete ticket.json with all data
        self.write_json_file(&mut zip, "ticket.json", ticket, options)?;

        // Finish the ZIP file
        zip.finish()
            .map_err(|e| TicketError::ArchiveCreationFailed(e.to_string()))?;

        Ok(zip_path)
    }

    /// Bundle ticket data and return as bytes (for direct output)
    pub fn bundle_to_bytes(&self, ticket: &TicketData) -> Result<Vec<u8>, TicketError> {
        let mut buffer = std::io::Cursor::new(Vec::new());
        let mut zip = ZipWriter::new(&mut buffer);

        let options = FileOptions::<()>::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .unix_permissions(0o644);

        // Write manifest.json
        let manifest = serde_json::json!({
            "ticket_id": ticket.ticket_id,
            "created_at": ticket.created_at,
            "jarvy_version": ticket.jarvy_version,
        });
        self.write_json_file(&mut zip, "manifest.json", &manifest, options)?;

        // Write complete ticket.json
        self.write_json_file(&mut zip, "ticket.json", ticket, options)?;

        zip.finish()
            .map_err(|e| TicketError::ArchiveCreationFailed(e.to_string()))?;

        Ok(buffer.into_inner())
    }

    /// Write a JSON file to the ZIP archive
    fn write_json_file<W: Write + std::io::Seek, T: serde::Serialize>(
        &self,
        zip: &mut ZipWriter<W>,
        filename: &str,
        data: &T,
        options: FileOptions<()>,
    ) -> Result<(), TicketError> {
        zip.start_file(filename, options)
            .map_err(|e| TicketError::ArchiveCreationFailed(e.to_string()))?;

        let json = serde_json::to_string_pretty(data)
            .map_err(|e| TicketError::ArchiveCreationFailed(e.to_string()))?;

        zip.write_all(json.as_bytes())
            .map_err(|e| TicketError::ArchiveCreationFailed(e.to_string()))?;

        Ok(())
    }
}

impl Default for TicketBundler {
    fn default() -> Self {
        Self::new()
    }
}

/// Preview what would be included in a ticket (dry-run)
pub fn preview_ticket(ticket: &TicketData) -> String {
    let mut output = String::new();

    output.push_str(&format!("Ticket ID: {}\n", ticket.ticket_id));
    output.push_str(&format!("Created: {}\n", ticket.created_at));
    output.push_str(&format!("Jarvy Version: {}\n", ticket.jarvy_version));
    output.push_str("\nContents:\n");

    if let Some(ref system) = ticket.system {
        output.push_str(&format!(
            "  - System info: {} {} ({})\n",
            system.os_name, system.os_version, system.architecture
        ));
    }

    if !ticket.tools.is_empty() {
        let installed = ticket.tools.iter().filter(|t| t.installed).count();
        output.push_str(&format!(
            "  - Tools: {} total, {} installed\n",
            ticket.tools.len(),
            installed
        ));
    }

    if ticket.config.is_some() {
        output.push_str("  - Configuration: included (sanitized)\n");
    }

    if !ticket.environment.is_empty() {
        output.push_str(&format!(
            "  - Environment: {} variables\n",
            ticket.environment.len()
        ));
    }

    if !ticket.logs.is_empty() {
        output.push_str(&format!("  - Logs: {} lines\n", ticket.logs.len()));
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_bundler_new() {
        let bundler = TicketBundler::new();
        assert!(bundler.output_dir.ends_with(".jarvy/tickets"));
    }

    #[test]
    fn test_bundler_with_output_dir() {
        let dir = TempDir::new().unwrap();
        let bundler = TicketBundler::with_output_dir(dir.path().to_path_buf());
        assert_eq!(bundler.output_dir, dir.path());
    }

    #[test]
    fn test_bundle_creates_zip() {
        let dir = TempDir::new().unwrap();
        let bundler = TicketBundler::with_output_dir(dir.path().to_path_buf());

        let ticket = TicketData::new();
        let path = bundler.bundle(&ticket).unwrap();

        assert!(path.exists());
        assert!(path.extension().map(|e| e == "zip").unwrap_or(false));
    }

    #[test]
    fn test_bundle_to_bytes() {
        let bundler = TicketBundler::new();
        let ticket = TicketData::new();

        let bytes = bundler.bundle_to_bytes(&ticket).unwrap();
        assert!(!bytes.is_empty());

        // Check ZIP magic bytes
        assert_eq!(&bytes[0..4], b"PK\x03\x04");
    }

    #[test]
    fn test_preview_ticket() {
        let mut ticket = TicketData::new();
        ticket.system = Some(super::super::collector::SystemInfo {
            os_name: "linux".to_string(),
            os_version: "Ubuntu".to_string(),
            os_release: "22.04".to_string(),
            architecture: "x86_64".to_string(),
            cpu_cores: 8,
            memory_total_mb: 16384,
            shell: "/bin/bash".to_string(),
            locale: "en_US.UTF-8".to_string(),
            home_directory: "~".to_string(),
            hostname: "test-host".to_string(),
        });

        let preview = preview_ticket(&ticket);

        assert!(preview.contains("Ticket ID:"));
        assert!(preview.contains("System info:"));
    }
}
