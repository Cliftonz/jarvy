//! Search available tools that Jarvy can install
//!
//! Provides fuzzy searching, category filtering, and detailed tool information.

use crate::output::{ExitCode, Outputable, colors, header};
use crate::telemetry;
use crate::tools::spec::generate_tool_index;
use serde::Serialize;

/// A single search result
#[derive(Debug, Clone, Serialize)]
pub struct SearchResult {
    pub name: String,
    pub command: String,
    pub platforms: Vec<String>,
    pub has_custom_installer: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relevance: Option<f64>,
}

/// Search results container
#[derive(Debug, Clone, Serialize)]
pub struct SearchResults {
    pub query: String,
    pub count: usize,
    pub results: Vec<SearchResult>,
}

impl Outputable for SearchResults {
    fn to_human(&self) -> String {
        if self.results.is_empty() {
            return format!(
                "No tools found matching \"{}\"\n\nTip: Try 'jarvy search --all' to list all available tools",
                self.query
            );
        }

        let mut output = String::new();

        if self.query.is_empty() {
            output.push_str(&header(&format!("Available Tools ({} tools)", self.count)));
        } else {
            output.push_str(&header(&format!(
                "Tools matching \"{}\" ({} results)",
                self.query, self.count
            )));
        }
        output.push('\n');

        for result in &self.results {
            output.push_str(&format!(
                "\n{}{}{}\n",
                colors::BOLD,
                result.name,
                colors::RESET
            ));

            let platforms_str = result.platforms.join(", ");
            output.push_str(&format!(
                "  {}Platforms:{} {}\n",
                colors::DIM,
                colors::RESET,
                platforms_str
            ));

            if result.has_custom_installer {
                output.push_str(&format!(
                    "  {}Installation:{} Custom installer\n",
                    colors::DIM,
                    colors::RESET
                ));
            }

            output.push_str(&format!(
                "  {}Command:{} {}\n",
                colors::DIM,
                colors::RESET,
                result.command
            ));
        }

        output.push_str(&format!(
            "\n{}Tip:{} Add to jarvy.toml:\n  [tools]\n  {} = \"latest\"\n",
            colors::DIM,
            colors::RESET,
            self.results
                .first()
                .map(|r| r.name.as_str())
                .unwrap_or("tool")
        ));

        output
    }

    fn exit_code(&self) -> ExitCode {
        if self.results.is_empty() {
            ExitCode::Warning
        } else {
            ExitCode::Ok
        }
    }
}

/// Search for tools by name using fuzzy matching
pub fn search_tools(query: &str, show_all: bool) -> SearchResults {
    let index = generate_tool_index();

    let results: Vec<SearchResult> = if show_all || query.is_empty() {
        // Return all tools
        index
            .tools
            .iter()
            .map(|tool| SearchResult {
                name: tool.name.clone(),
                command: tool.command.clone(),
                platforms: get_platforms(tool),
                has_custom_installer: tool.custom_install.has_custom_installer,
                relevance: None,
            })
            .collect()
    } else {
        // Fuzzy search
        let query_lower = query.to_lowercase();
        let mut scored_results: Vec<(SearchResult, f64)> = index
            .tools
            .iter()
            .filter_map(|tool| {
                let score = calculate_relevance(&tool.name, &query_lower);
                if score > 0.4 {
                    Some((
                        SearchResult {
                            name: tool.name.clone(),
                            command: tool.command.clone(),
                            platforms: get_platforms(tool),
                            has_custom_installer: tool.custom_install.has_custom_installer,
                            relevance: Some(score),
                        },
                        score,
                    ))
                } else {
                    None
                }
            })
            .collect();

        // Sort by relevance (highest first)
        scored_results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        scored_results.into_iter().map(|(r, _)| r).collect()
    };

    let count = results.len();

    // Emit telemetry
    telemetry::search_executed(query, count);

    // When a user searched for something specific and no result name
    // *contains* the query, that's a demand signal. Score alone can't
    // gate this: jaro-winkler can push nonsense queries above the 0.7
    // "contains" bucket, so we check the substring relationship
    // directly on the top result — the same criterion a human would
    // use to say "yes, that's what I meant". Same event shape as
    // diagnose/doctor so all three surfaces are queryable by source.
    let query_lower = query.to_lowercase();
    let has_strong_match = results
        .first()
        .is_some_and(|r| r.name.to_lowercase().contains(&query_lower));
    if !query.is_empty() && !show_all && !has_strong_match {
        if crate::observability::telemetry_gate::is_enabled() {
            tracing::warn!(
                event = "tool.unsupported",
                tool = %query,
                source = %telemetry::Source::Search,
                platform = %std::env::consts::OS,
            );
        }
        telemetry::tool_not_supported(query, None, telemetry::Source::Search);
    }

    SearchResults {
        query: query.to_string(),
        count,
        results,
    }
}

/// Calculate fuzzy match relevance score between tool name and query
fn calculate_relevance(name: &str, query: &str) -> f64 {
    let name_lower = name.to_lowercase();
    let query_lower = query.to_lowercase();

    // Exact match
    if name_lower == query_lower {
        return 1.0;
    }

    // Starts with query
    if name_lower.starts_with(&query_lower) {
        return 0.9;
    }

    // Contains query
    if name_lower.contains(&query_lower) {
        return 0.7;
    }

    // Use strsim for fuzzy matching
    let jaro = strsim::jaro_winkler(&name_lower, &query_lower);

    // Boost score if query appears as substring of any word
    let boost = if name_lower
        .split(['-', '_'])
        .any(|part| part.starts_with(&query_lower))
    {
        0.1
    } else {
        0.0
    };

    (jaro + boost).min(1.0)
}

/// Get platform list for a tool
fn get_platforms(tool: &crate::tools::spec::ToolIndexEntry) -> Vec<String> {
    let mut platforms = Vec::new();
    let universal = tool.custom_install.has_custom_installer || !tool.fallback.is_empty();

    if tool.macos.is_some() || universal {
        platforms.push("macOS".to_string());
    }
    if tool.linux.is_some() || universal {
        platforms.push("Linux".to_string());
    }
    if tool.windows.is_some() || universal {
        platforms.push("Windows".to_string());
    }

    platforms
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_exact_match() {
        let results = search_tools("git", false);
        assert!(!results.results.is_empty());
        // git should be first or near the top
        assert!(results.results.iter().any(|r| r.name == "git"));
    }

    #[test]
    fn test_search_partial_match() {
        let results = search_tools("doc", false);
        // Should find docker
        assert!(results.results.iter().any(|r| r.name.contains("docker")));
    }

    #[test]
    fn fallback_tool_search_reports_cross_platform_support() {
        let results = search_tools("eas_cli", false);
        let eas = results
            .results
            .iter()
            .find(|result| result.name == "eas_cli")
            .expect("EAS CLI must be searchable");

        assert_eq!(eas.platforms, ["macOS", "Linux", "Windows"]);
    }

    #[test]
    fn test_search_all() {
        let results = search_tools("", true);
        assert!(results.count > 0);
        // Should return all tools
        let index = generate_tool_index();
        assert_eq!(results.count, index.count);
    }

    #[test]
    fn nonsense_query_top_result_does_not_contain_query() {
        // Sanity for the demand-signal event trigger in `search_tools`:
        // for a query that is not a substring of any tool name, the top
        // result (if any) must not contain the query — that is the
        // criterion the event fires on. If a tool name ever begins to
        // contain "zzz-definitely-not-a-tool-xyz", pick a longer nonsense.
        let query = "zzz-definitely-not-a-tool-xyz";
        let results = search_tools(query, false);
        let top_contains = results
            .results
            .first()
            .is_some_and(|r| r.name.to_lowercase().contains(query));
        assert!(
            !top_contains,
            "nonsense query must not be a substring of the top result; got {:?}",
            results.results.first().map(|r| &r.name)
        );
    }

    #[test]
    fn test_search_poor_match() {
        // Use a query that won't match well - should return far fewer results than total tools
        let results = search_tools("qqqqqqqqqqqqqqqqqqq", false);
        let all_results = search_tools("", true);
        // A nonsense query should return much less than all tools
        // (at most 10% of all tools due to fuzzy matching noise)
        assert!(
            results.count < all_results.count / 10,
            "Expected very few results but got {} out of {}",
            results.count,
            all_results.count
        );
    }

    #[test]
    fn test_calculate_relevance_exact() {
        assert_eq!(calculate_relevance("git", "git"), 1.0);
    }

    #[test]
    fn test_calculate_relevance_prefix() {
        let score = calculate_relevance("docker", "doc");
        assert!(score >= 0.9);
    }

    #[test]
    fn test_calculate_relevance_contains() {
        let score = calculate_relevance("lazydocker", "docker");
        assert!(score >= 0.7);
    }

    #[test]
    fn test_search_results_to_human() {
        let results = SearchResults {
            query: "test".to_string(),
            count: 1,
            results: vec![SearchResult {
                name: "test-tool".to_string(),
                command: "test".to_string(),
                platforms: vec!["macOS".to_string(), "Linux".to_string()],
                has_custom_installer: false,
                relevance: Some(0.8),
            }],
        };
        let output = results.to_human();
        assert!(output.contains("test-tool"));
        assert!(output.contains("macOS"));
    }

    #[test]
    fn test_search_results_exit_code() {
        let empty_results = SearchResults {
            query: "xyz".to_string(),
            count: 0,
            results: vec![],
        };
        assert_eq!(empty_results.exit_code(), ExitCode::Warning);

        let results = SearchResults {
            query: "git".to_string(),
            count: 1,
            results: vec![SearchResult {
                name: "git".to_string(),
                command: "git".to_string(),
                platforms: vec!["macOS".to_string()],
                has_custom_installer: false,
                relevance: Some(1.0),
            }],
        };
        assert_eq!(results.exit_code(), ExitCode::Ok);
    }
}
