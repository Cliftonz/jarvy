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
    fn redis_registration_shape() {
        assert_eq!(REDIS.command, "redis-cli");
        let mac = REDIS.macos.expect("must support macOS");
        assert_eq!(mac.brew, Some("redis"));
        let win = REDIS.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("Redis.Redis"));
    }
}
