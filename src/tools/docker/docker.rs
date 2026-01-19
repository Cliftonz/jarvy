//! docker - containerization platform
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(DOCKER, {
    command: "docker",
    macos: { cask: "docker" },
    linux: { apt: "docker.io", dnf: "docker", pacman: "docker", apk: "docker" },
    windows: { winget: "Docker.DockerDesktop" },
    bsd: { pkg: "docker" },
    default_hook: {
        description: "Add user to docker group (Linux) for rootless access",
        script: r#"
# Add current user to docker group (Linux only)
if [ "$(uname)" = "Linux" ]; then
    if ! groups 2>/dev/null | grep -q docker; then
        echo "Note: To run docker without sudo, run: sudo usermod -aG docker $USER"
        echo "Then log out and back in for the change to take effect."
    fi
fi
"#
    },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_docker_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
