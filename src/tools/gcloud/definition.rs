//! gcloud - Google Cloud CLI
//!
//! The Google Cloud CLI provides the primary command-line interface for
//! Google Cloud Platform, including managing resources, deploying apps,
//! and interacting with GCP services.
//!
//! This tool uses the ToolSpec pattern for declarative installation.

use crate::define_tool;

define_tool!(GCLOUD, {
    command: "gcloud",
    macos: { cask: "gcloud-cli" },
    linux: { apt: "google-cloud-cli", dnf: "google-cloud-cli", pacman: "google-cloud-cli", apk: "google-cloud-sdk" },
    windows: { winget: "Google.CloudSDK" },
    default_hook: {
        description: "Add gcloud shell completion and PATH for components",
        script: r#"
# Add gcloud completion and PATH to .bashrc
if [ -f "$HOME/.bashrc" ] && ! grep -q 'google-cloud-sdk' "$HOME/.bashrc"; then
    # Homebrew location
    if [ -d "$(brew --prefix 2>/dev/null)/share/google-cloud-sdk" ]; then
        GC_SDK="$(brew --prefix)/share/google-cloud-sdk"
        echo "source \"$GC_SDK/path.bash.inc\"" >> "$HOME/.bashrc"
        echo "source \"$GC_SDK/completion.bash.inc\"" >> "$HOME/.bashrc"
        echo "Added gcloud PATH and completion to .bashrc"
    fi
fi

# Add gcloud completion and PATH to .zshrc
if [ -f "$HOME/.zshrc" ] && ! grep -q 'google-cloud-sdk' "$HOME/.zshrc"; then
    if [ -d "$(brew --prefix 2>/dev/null)/share/google-cloud-sdk" ]; then
        GC_SDK="$(brew --prefix)/share/google-cloud-sdk"
        echo "source \"$GC_SDK/path.zsh.inc\"" >> "$HOME/.zshrc"
        echo "source \"$GC_SDK/completion.zsh.inc\"" >> "$HOME/.zshrc"
        echo "Added gcloud PATH and completion to .zshrc"
    fi
fi
"#
    },
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gcloud_registration_shape() {
        assert_eq!(GCLOUD.command, "gcloud");
        let mac = GCLOUD.macos.expect("must support macOS");
        assert_eq!(mac.cask, Some("gcloud-cli"));
        let win = GCLOUD.windows.expect("must support Windows");
        assert_eq!(win.winget, Some("Google.CloudSDK"));
    }
}
