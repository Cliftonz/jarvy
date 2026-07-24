//! NuGet freshness backend via `dotnet tool search`.
//!
//! `dotnet tool search <id> --take 1 --detail` returns a
//! bordered table plus a detail block:
//!
//! ```text
//! Package ID       Latest Version   Authors     ...
//! ----------------------------------------------------
//! dotnet-ef        9.0.0            Microsoft   ...
//! ```
//!
//! Rather than parse the visual table (widths depend on the
//! matched package name and shift between .NET SDK minor
//! versions), we search for the `Latest Version` column value
//! by locating the header row and picking the value on the row
//! below whose first token matches the requested id.

use super::{BACKEND_TIMEOUT, BackendError, FreshnessBackend, probe_error};

pub struct NugetBackend;

impl FreshnessBackend for NugetBackend {
    fn name(&self) -> &'static str {
        "nuget"
    }

    fn latest(&self, pkg_id: &str) -> Result<String, BackendError> {
        let probe = crate::tools::common::probe_with_timeout(
            "dotnet",
            &["tool", "search", pkg_id, "--take", "1"],
            BACKEND_TIMEOUT,
        );
        let out = probe_error(probe)?;
        if !out.status.success() {
            return Err(BackendError::Other);
        }
        parse_dotnet_tool_search(pkg_id, &out.stdout)
    }
}

fn parse_dotnet_tool_search(pkg_id: &str, stdout: &[u8]) -> Result<String, BackendError> {
    let text = std::str::from_utf8(stdout).map_err(|_| BackendError::ParseFailed)?;
    let trimmed = text.trim();
    if trimmed.is_empty() || trimmed.contains("No results found") {
        return Err(BackendError::NotFound);
    }

    let mut lines = text.lines().filter(|l| !l.trim().is_empty());
    let header = lines.next().ok_or(BackendError::ParseFailed)?;
    let latest_col = header
        .to_ascii_lowercase()
        .find("latest version")
        .ok_or(BackendError::ParseFailed)?;

    // Skip the `-----` separator row.
    let separator = lines.next().ok_or(BackendError::NotFound)?;
    if !separator
        .trim()
        .chars()
        .all(|c| c == '-' || c.is_whitespace())
    {
        // Some dotnet SDK builds omit the separator; treat the
        // "separator" as a data row and reuse it below.
        return extract_from_row(separator, pkg_id, latest_col);
    }

    let data = lines.next().ok_or(BackendError::NotFound)?;
    extract_from_row(data, pkg_id, latest_col)
}

fn extract_from_row(row: &str, pkg_id: &str, latest_col: usize) -> Result<String, BackendError> {
    let first = row.split_whitespace().next().unwrap_or("");
    if !first.eq_ignore_ascii_case(pkg_id) {
        return Err(BackendError::NotFound);
    }
    // Slice at the header offset to isolate the "Latest Version"
    // column and take its first whitespace-delimited token.
    let slice = row.get(latest_col..).unwrap_or("");
    let v = slice.split_whitespace().next().unwrap_or("");
    if v.is_empty() {
        return Err(BackendError::ParseFailed);
    }
    Ok(v.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &[u8] = b"\nPackage ID                          Latest Version      Authors                                                            Downloads      Verified\n----------------------------------------------------------------------------------------------------------------------------------\ndotnet-ef                           9.0.0               Microsoft                                                          10000000       True\n";

    #[test]
    fn parses_latest_version() {
        assert_eq!(
            parse_dotnet_tool_search("dotnet-ef", FIXTURE).unwrap(),
            "9.0.0"
        );
    }

    #[test]
    fn wrong_name_is_not_found() {
        assert_eq!(
            parse_dotnet_tool_search("nope", FIXTURE).unwrap_err(),
            BackendError::NotFound
        );
    }

    #[test]
    fn no_results_text_is_not_found() {
        let fixture = b"No results found.\n";
        assert_eq!(
            parse_dotnet_tool_search("x", fixture).unwrap_err(),
            BackendError::NotFound
        );
    }

    #[test]
    fn empty_output_is_not_found() {
        assert_eq!(
            parse_dotnet_tool_search("x", b"").unwrap_err(),
            BackendError::NotFound
        );
    }
}
