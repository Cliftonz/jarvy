//! JSON Schema generation for jarvy.toml
//!
//! Outputs a JSON Schema that editors can use for autocomplete and validation.

use crate::output::Outputable;
use serde::Serialize;

/// Schema output container
#[derive(Debug, Clone, Serialize)]
pub struct SchemaOutput {
    pub schema: serde_json::Value,
}

impl Outputable for SchemaOutput {
    fn to_human(&self) -> String {
        serde_json::to_string_pretty(&self.schema)
            .unwrap_or_else(|e| format!("{{\"error\":\"{}\"}}", e))
    }
}

/// Generate the JSON Schema for jarvy.toml
///
/// This builds the schema manually since we can't derive it automatically
/// from all the serde structs without adding schemars to every dependency.
pub fn generate_schema() -> SchemaOutput {
    let schema = serde_json::json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": "https://jarvy.dev/schema/jarvy.toml.json",
        "title": "jarvy.toml",
        "description": "Jarvy development environment configuration file",
        "type": "object",
        "properties": {
            "extends": {
                "description": "Parent config(s) to inherit from (URL or local path)",
                "oneOf": [
                    { "type": "string" },
                    { "type": "array", "items": { "type": "string" } }
                ]
            },
            "role": {
                "description": "Role assignment for this config",
                "oneOf": [
                    { "type": "string" },
                    { "type": "array", "items": { "type": "string" } }
                ]
            },
            "provisioner": {
                "description": "Tools to install with version requirements",
                "type": "object",
                "additionalProperties": {
                    "oneOf": [
                        {
                            "type": "string",
                            "description": "Version string (e.g., 'latest', '20', '>=1.0')"
                        },
                        {
                            "type": "object",
                            "properties": {
                                "version": {
                                    "type": "string",
                                    "description": "Version requirement"
                                },
                                "version_manager": {
                                    "type": "boolean",
                                    "description": "Use version manager if available"
                                },
                                "use_sudo": {
                                    "type": "boolean",
                                    "description": "Override sudo usage for this tool"
                                }
                            },
                            "required": ["version"]
                        }
                    ]
                }
            },
            "privileges": {
                "description": "Sudo/privilege configuration",
                "type": "object",
                "properties": {
                    "use_sudo": { "type": "boolean" },
                    "per_os": {
                        "type": "object",
                        "properties": {
                            "linux": { "type": "boolean" },
                            "macos": { "type": "boolean" },
                            "windows": { "type": "boolean" }
                        }
                    }
                }
            },
            "hooks": {
                "description": "Pre/post setup hooks and per-tool hooks",
                "type": "object",
                "properties": {
                    "pre_setup": { "type": "string", "description": "Script to run before tool installation" },
                    "post_setup": { "type": "string", "description": "Script to run after all tools are installed" },
                    "config": {
                        "type": "object",
                        "properties": {
                            "shell": { "type": "string", "description": "Shell for hook execution" },
                            "timeout": { "type": "integer", "description": "Timeout in seconds (default: 300)" },
                            "continue_on_error": { "type": "boolean" }
                        }
                    }
                },
                "additionalProperties": {
                    "type": "object",
                    "properties": {
                        "post_install": { "type": "string", "description": "Script to run after this tool is installed" }
                    }
                }
            },
            "env": {
                "description": "Environment variable configuration",
                "type": "object",
                "properties": {
                    "vars": {
                        "type": "object",
                        "description": "Environment variables to set",
                        "additionalProperties": {
                            "oneOf": [
                                { "type": "string" },
                                {
                                    "type": "object",
                                    "properties": {
                                        "value": { "type": "string" },
                                        "description": { "type": "string" },
                                        "append": { "type": "boolean" },
                                        "per_tool": { "type": "boolean" }
                                    },
                                    "required": ["value"]
                                }
                            ]
                        }
                    },
                    "secrets": {
                        "type": "object",
                        "description": "Secret variables (prompted or loaded from file)",
                        "additionalProperties": true
                    },
                    "config": {
                        "type": "object",
                        "properties": {
                            "shell": { "type": "string" },
                            "update_rc": { "type": "boolean" },
                            "generate_dotenv": { "type": "boolean" },
                            "dotenv_path": { "type": "string" },
                            "add_to_gitignore": { "type": "boolean" },
                            "backup_rc": { "type": "boolean" }
                        }
                    }
                }
            },
            "services": {
                "description": "Service management (Docker Compose, Tilt)",
                "type": "object",
                "properties": {
                    "enabled": { "type": "boolean" },
                    "auto_start": { "type": "boolean" },
                    "compose_file": { "type": "string" },
                    "tilt_file": { "type": "string" },
                    "start_in_ci": { "type": "boolean" }
                }
            },
            "roles": {
                "description": "Role definitions for team-based configurations",
                "type": "object",
                "additionalProperties": {
                    "type": "object",
                    "properties": {
                        "description": { "type": "string" },
                        "extends": { "type": "string", "description": "Parent role to inherit from" },
                        "tools": {
                            "oneOf": [
                                { "type": "array", "items": { "type": "string" } },
                                {
                                    "type": "object",
                                    "additionalProperties": { "type": "string" }
                                }
                            ]
                        }
                    }
                }
            },
            "network": {
                "description": "Proxy and network configuration",
                "type": "object",
                "properties": {
                    "https_proxy": { "type": "string" },
                    "http_proxy": { "type": "string" },
                    "no_proxy": { "type": "array", "items": { "type": "string" } },
                    "auth": {
                        "type": "object",
                        "properties": {
                            "username": { "type": "string" },
                            "password": true
                        }
                    },
                    "tls": {
                        "type": "object",
                        "properties": {
                            "ca_bundle": { "type": "string" }
                        }
                    }
                }
            },
            "npm": {
                "description": "npm package dependencies",
                "type": "object",
                "properties": {
                    "package_manager": { "type": "string", "enum": ["npm", "yarn", "pnpm"] },
                    "from_lockfile": { "type": "boolean" }
                },
                "additionalProperties": {
                    "oneOf": [
                        { "type": "string" },
                        { "type": "object" }
                    ]
                }
            },
            "pip": {
                "description": "pip package dependencies",
                "type": "object",
                "properties": {
                    "venv": { "type": "string" },
                    "create_venv": { "type": "boolean" },
                    "from_lockfile": { "type": "boolean" }
                },
                "additionalProperties": {
                    "oneOf": [
                        { "type": "string" },
                        { "type": "object" }
                    ]
                }
            },
            "cargo": {
                "description": "cargo binary dependencies",
                "type": "object",
                "properties": {
                    "locked": { "type": "boolean" }
                },
                "additionalProperties": {
                    "oneOf": [
                        { "type": "string" },
                        { "type": "object" }
                    ]
                }
            },
            "git": {
                "description": "Git configuration automation",
                "type": "object",
                "properties": {
                    "user_name": true,
                    "user_email": true,
                    "signing": { "type": "boolean" },
                    "signing_key": { "type": "string" },
                    "signing_format": { "type": "string", "enum": ["ssh", "gpg"] },
                    "default_branch": { "type": "string" },
                    "pull_rebase": { "type": "boolean" },
                    "auto_stash": { "type": "boolean" },
                    "push_autosetup": { "type": "boolean" },
                    "editor": { "type": "string" },
                    "autocrlf": true,
                    "eol": { "type": "string" },
                    "credential_helper": { "type": "string" },
                    "scope": { "type": "string", "enum": ["global", "local"] },
                    "aliases": {
                        "type": "object",
                        "additionalProperties": { "type": "string" }
                    }
                }
            },
            "drift": {
                "description": "Configuration drift detection",
                "type": "object",
                "properties": {
                    "enabled": { "type": "boolean" },
                    "check_on_run": { "type": "boolean" },
                    "track_files": { "type": "array", "items": { "type": "string" } },
                    "version_policy": { "type": "string", "enum": ["major", "minor", "patch", "exact"] },
                    "ignore_tools": { "type": "array", "items": { "type": "string" } },
                    "allow_upgrades": { "type": "boolean" }
                }
            },
            "workspace": {
                "description": "Monorepo workspace configuration",
                "type": "object",
                "properties": {
                    "members": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Paths to workspace member directories"
                    },
                    "inherit": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Sections that members inherit from root"
                    }
                }
            },
            "commands": {
                "description": "Custom project commands for interactive menu",
                "type": "object",
                "properties": {
                    "run": { "type": "string" },
                    "test": { "type": "string" },
                    "setup": { "type": "string" }
                }
            }
        }
    });

    SchemaOutput { schema }
}
