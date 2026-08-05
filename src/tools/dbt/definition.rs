//! dbt (data build tool) - analytics-engineering CLI
//!
//! `dbt` compiles SQL models into DAG-ordered warehouse
//! transformations with testing, documentation, and lineage.
//! The de-facto standard transform layer of the modern data stack.

use crate::define_tool;

define_tool!(DBT, {
    command: "dbt",
    // No brew formula for dbt-core as of 2026-08; the uv fallback route
    // covers macOS/Linux via PyPI `dbt-core` (bin = `dbt`, verified
    // 2026-08). Winget is first-party on Windows.
    //
    // Caveat: dbt-core alone ships no warehouse adapter — users
    // typically also need dbt-postgres / dbt-snowflake / dbt-bigquery
    // etc., installable via jarvy's [pip] packages.
    windows: { winget: "dbtLabs.dbt-core" },
    fallback: { uv: "dbt-core" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dbt_registration_shape() {
        assert_eq!(DBT.command, "dbt");
        assert!(
            DBT.macos.is_none(),
            "no brew formula; uv fallback covers macOS"
        );
        assert!(
            DBT.linux.is_none(),
            "no distro package; uv fallback covers Linux"
        );
        let win = DBT.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("dbtLabs.dbt-core"));
        assert_eq!(DBT.fallback.len(), 1);
        assert_eq!(DBT.fallback[0].eco, crate::tools::spec::FallbackEco::Uv);
        assert_eq!(DBT.fallback[0].package, "dbt-core");
    }
}
