//! Shell completion generation for jarvy CLI
//!
//! Generates shell completions for bash, zsh, fish, PowerShell, elvish,
//! and nushell. Uses clap_complete (plus clap_complete_nushell — nushell
//! isn't in clap_complete's built-in `Shell` enum) to generate completions
//! from the CLI definition.

use clap::Command;
use clap_complete::{Shell, generate};
use std::io;

/// Supported shells for completion generation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionShell {
    Bash,
    Zsh,
    Fish,
    PowerShell,
    Elvish,
    Nushell,
}

impl std::fmt::Display for CompletionShell {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CompletionShell::Bash => write!(f, "bash"),
            CompletionShell::Zsh => write!(f, "zsh"),
            CompletionShell::Fish => write!(f, "fish"),
            CompletionShell::PowerShell => write!(f, "powershell"),
            CompletionShell::Elvish => write!(f, "elvish"),
            CompletionShell::Nushell => write!(f, "nushell"),
        }
    }
}

impl std::str::FromStr for CompletionShell {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "bash" => Ok(CompletionShell::Bash),
            "zsh" => Ok(CompletionShell::Zsh),
            "fish" => Ok(CompletionShell::Fish),
            "powershell" | "pwsh" | "ps1" => Ok(CompletionShell::PowerShell),
            "elvish" => Ok(CompletionShell::Elvish),
            "nushell" | "nu" => Ok(CompletionShell::Nushell),
            _ => Err(format!(
                "Unknown shell '{}'. Supported: bash, zsh, fish, powershell, elvish, nushell",
                s
            )),
        }
    }
}

/// Generate shell completions and write to the provided writer.
/// Nushell dispatches to its own generator type; the rest map onto
/// clap_complete's built-in `Shell` enum (which is why there is no
/// total `From<CompletionShell> for Shell` impl).
pub fn generate_completions<W: io::Write>(cmd: &mut Command, shell: CompletionShell, buf: &mut W) {
    match shell {
        CompletionShell::Nushell => {
            generate(clap_complete_nushell::Nushell, cmd, "jarvy", buf);
        }
        other => {
            let shell = match other {
                CompletionShell::Bash => Shell::Bash,
                CompletionShell::Zsh => Shell::Zsh,
                CompletionShell::Fish => Shell::Fish,
                CompletionShell::PowerShell => Shell::PowerShell,
                CompletionShell::Elvish => Shell::Elvish,
                CompletionShell::Nushell => unreachable!("handled above"),
            };
            generate(shell, cmd, "jarvy", buf);
        }
    }
}

/// Generate shell completions as a string
pub fn generate_completions_string(cmd: &mut Command, shell: CompletionShell) -> String {
    let mut buf = Vec::new();
    generate_completions(cmd, shell, &mut buf);
    String::from_utf8(buf).unwrap_or_else(|_| "# Error generating completions".to_string())
}

/// Get installation instructions for shell completions
pub fn get_install_instructions() -> String {
    r#"Shell Completion Installation
=============================

Bash:
  # Option 1: System-wide (requires root)
  jarvy completions bash | sudo tee /usr/local/etc/bash_completion.d/jarvy > /dev/null

  # Option 2: User-local
  mkdir -p ~/.local/share/bash-completion/completions
  jarvy completions bash > ~/.local/share/bash-completion/completions/jarvy

  # Reload shell or run:
  source ~/.bashrc

Zsh:
  # Create completions directory if needed
  mkdir -p ~/.zsh/completions

  # Generate completions
  jarvy completions zsh > ~/.zsh/completions/_jarvy

  # Add to .zshrc if not present:
  # fpath=(~/.zsh/completions $fpath)
  # autoload -Uz compinit && compinit

  # Reload shell or run:
  source ~/.zshrc

Fish:
  # Generate completions
  jarvy completions fish > ~/.config/fish/completions/jarvy.fish

  # Completions will be available in new shell sessions

PowerShell:
  # Add to your PowerShell profile
  jarvy completions powershell >> $PROFILE

  # Or create a separate file and dot-source it
  jarvy completions powershell > ~/.config/powershell/jarvy.ps1
  # Add to $PROFILE: . ~/.config/powershell/jarvy.ps1

Elvish:
  # Generate completions
  jarvy completions elvish > ~/.elvish/lib/jarvy.elv

  # Add to ~/.elvish/rc.elv:
  # use jarvy

Nushell:
  # Generate completions
  mkdir ~/.config/nushell/completions
  jarvy completions nushell | save -f ~/.config/nushell/completions/jarvy.nu

  # Add to ~/.config/nushell/config.nu:
  # source ~/.config/nushell/completions/jarvy.nu
"#
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_completion_shell_display() {
        assert_eq!(CompletionShell::Bash.to_string(), "bash");
        assert_eq!(CompletionShell::Zsh.to_string(), "zsh");
        assert_eq!(CompletionShell::Fish.to_string(), "fish");
        assert_eq!(CompletionShell::PowerShell.to_string(), "powershell");
    }

    #[test]
    fn test_completion_shell_from_str() {
        assert_eq!(
            "bash".parse::<CompletionShell>().unwrap(),
            CompletionShell::Bash
        );
        assert_eq!(
            "zsh".parse::<CompletionShell>().unwrap(),
            CompletionShell::Zsh
        );
        assert_eq!(
            "fish".parse::<CompletionShell>().unwrap(),
            CompletionShell::Fish
        );
        assert_eq!(
            "powershell".parse::<CompletionShell>().unwrap(),
            CompletionShell::PowerShell
        );
        assert_eq!(
            "pwsh".parse::<CompletionShell>().unwrap(),
            CompletionShell::PowerShell
        );
        assert_eq!(
            "nushell".parse::<CompletionShell>().unwrap(),
            CompletionShell::Nushell
        );
        assert_eq!(
            "nu".parse::<CompletionShell>().unwrap(),
            CompletionShell::Nushell
        );
    }

    #[test]
    fn test_nushell_completions_generate_non_empty() {
        let mut cmd = clap::Command::new("jarvy").subcommand(clap::Command::new("setup"));
        let out = generate_completions_string(&mut cmd, CompletionShell::Nushell);
        // Structural assertions — "contains jarvy" alone would pass on a
        // truncated stub. `export extern` is nushell's completion-signature
        // keyword; the subcommand line proves the Command tree was walked.
        assert!(
            out.contains("export extern"),
            "nushell completions should declare externs; got:\n{out}"
        );
        assert!(
            out.contains("jarvy setup"),
            "nushell completions should cover the subcommand; got:\n{out}"
        );
    }

    #[test]
    fn test_completion_shell_from_str_case_insensitive() {
        assert_eq!(
            "BASH".parse::<CompletionShell>().unwrap(),
            CompletionShell::Bash
        );
        assert_eq!(
            "ZSH".parse::<CompletionShell>().unwrap(),
            CompletionShell::Zsh
        );
    }

    #[test]
    fn test_completion_shell_from_str_invalid() {
        assert!("invalid".parse::<CompletionShell>().is_err());
    }

    #[test]
    fn test_get_install_instructions() {
        let instructions = get_install_instructions();
        assert!(instructions.contains("Bash:"));
        assert!(instructions.contains("Zsh:"));
        assert!(instructions.contains("Fish:"));
        assert!(instructions.contains("PowerShell:"));
        assert!(instructions.contains("Nushell:"));
    }
}
