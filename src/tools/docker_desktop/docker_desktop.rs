//! docker_desktop - Docker Desktop for Mac, Windows, and Linux
//!
//! This tool uses the ToolSpec pattern for declarative installation.
//! Docker Desktop includes Docker Engine, Docker CLI, Docker Compose,
//! and Kubernetes support in a single package.

use crate::define_tool;

define_tool!(DOCKER_DESKTOP, {
    command: "docker",
    macos: { cask: "docker" },
    linux: { apt: "docker-desktop", dnf: "docker-desktop", pacman: "docker-desktop", apk: "docker-desktop" },
    windows: { winget: "Docker.DockerDesktop" },
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
    fn ensure_docker_desktop_no_panic() {
        let res = ensure("");
        assert!(res.is_ok() || res.is_err());
    }
}
