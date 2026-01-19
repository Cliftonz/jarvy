//! redis - Redis command-line interface (redis-cli)
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(REDIS, {
    command: "redis-cli",
    macos: { brew: "redis" },
    linux: { uniform: "redis" },
    windows: { winget: "Redis.Redis" },
    bsd: { pkg: "redis" },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_redis_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
