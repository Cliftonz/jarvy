//! MCP Server Implementation
//!
//! Main server loop that handles JSON-RPC requests and routes them to appropriate handlers.

use crate::mcp::audit::AuditLog;
use crate::mcp::config::McpConfig;
use crate::mcp::error::{McpError, McpResult};
use crate::mcp::prompts;
use crate::mcp::resources;
use crate::mcp::safety::RateLimiter;
use crate::mcp::tools;
use crate::mcp::transport::{JsonRpcRequest, JsonRpcResponse, StdioTransport};
use crate::mcp::{PROTOCOL_VERSION, SERVER_NAME, SERVER_VERSION};
use serde::{Deserialize, Serialize};

/// MCP Server that handles JSON-RPC requests over stdio
pub struct McpServer {
    /// Server configuration
    config: McpConfig,
    /// Rate limiter for safety
    rate_limiter: RateLimiter,
    /// Audit logger
    audit_log: AuditLog,
    /// Client information (set during initialize)
    client_info: Option<ClientInfo>,
    /// Filesystem root every mutating tool is constrained to. Caller-
    /// supplied `project_dir` / `output_path` arguments are resolved
    /// **relative** to this directory and any traversal outside it is
    /// refused. Captured at server start from `JARVY_MCP_WORKSPACE`
    /// (override) or the process's cwd.
    workspace_root: std::path::PathBuf,
}

/// Information about the connected MCP client
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientInfo {
    /// Client name (e.g., "claude-desktop")
    pub name: String,
    /// Client version
    #[serde(default)]
    pub version: Option<String>,
}

/// MCP Initialize request parameters
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InitializeParams {
    /// Protocol version the client supports
    #[allow(dead_code)] // Required by MCP protocol spec for future use
    protocol_version: String,
    /// Client capabilities
    #[serde(default)]
    #[allow(dead_code)] // Required by MCP protocol spec for future use
    capabilities: serde_json::Value,
    /// Client information
    #[serde(default)]
    client_info: Option<ClientInfo>,
}

/// MCP Initialize response
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct InitializeResult {
    /// Protocol version the server supports
    protocol_version: String,
    /// Server capabilities
    capabilities: ServerCapabilities,
    /// Server information
    server_info: ServerInfo,
}

/// Server capabilities
#[derive(Debug, Serialize)]
struct ServerCapabilities {
    /// Tool capabilities
    tools: ToolCapabilities,
    /// Resource capabilities
    resources: ResourceCapabilities,
    /// Prompt capabilities
    prompts: PromptCapabilities,
}

/// Tool capabilities
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ToolCapabilities {
    /// Whether the server supports listing tools that changed
    list_changed: bool,
}

/// Resource capabilities
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ResourceCapabilities {
    /// Whether resources can be subscribed to
    subscribe: bool,
    /// Whether the server supports listing resources that changed
    list_changed: bool,
}

/// Prompt capabilities
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PromptCapabilities {
    /// Whether the server supports listing prompts that changed
    list_changed: bool,
}

/// Server information
#[derive(Debug, Serialize)]
struct ServerInfo {
    /// Server name
    name: String,
    /// Server version
    version: String,
}

impl McpServer {
    /// Create a new MCP server with the given configuration
    pub fn new(config: McpConfig) -> Self {
        let rate_limiter = RateLimiter::new(&config);
        let audit_log = AuditLog::new(&config).unwrap_or_else(|_| AuditLog::disabled());
        // Workspace defaults to the process cwd at startup. Tests +
        // sandboxed launchers can override with JARVY_MCP_WORKSPACE.
        let workspace_root = std::env::var_os("JARVY_MCP_WORKSPACE")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| {
                std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
            });

        Self {
            config,
            rate_limiter,
            audit_log,
            client_info: None,
            workspace_root,
        }
    }

    /// Build a [`MutationCtx`] referencing this server's collaborators.
    /// Called per `tools/call` for any mutating extended tool — the
    /// borrow lifetimes are tied to the surrounding match arm and
    /// dropped immediately after the handler returns.
    fn mutation_ctx<'a>(
        &'a self,
        client_name: Option<&'a str>,
    ) -> crate::mcp::extended_tools::MutationCtx<'a> {
        crate::mcp::extended_tools::MutationCtx {
            config: &self.config,
            rate_limiter: &self.rate_limiter,
            audit_log: &self.audit_log,
            client_name,
            workspace_root: self.workspace_root.clone(),
        }
    }

    /// Run the MCP server (blocking)
    pub fn run(mut self) -> McpResult<()> {
        let mut transport = StdioTransport::new();

        loop {
            match transport.read_request() {
                Ok(Some(request)) => {
                    let response = self.handle_request(&request);
                    transport.write_response(&response)?;
                }
                Ok(None) => {
                    // EOF - client disconnected
                    break;
                }
                Err(e) => {
                    // Parse error - send error response
                    let response = JsonRpcResponse::error(None, e);
                    transport.write_response(&response)?;
                }
            }
        }

        Ok(())
    }

    /// Handle a single JSON-RPC request
    fn handle_request(&mut self, request: &JsonRpcRequest) -> JsonRpcResponse {
        let result = match request.method.as_str() {
            // Lifecycle methods
            "initialize" => self.handle_initialize(request),
            "notifications/initialized" => {
                // Notification - no response needed, but we return empty for consistency
                return JsonRpcResponse::success(request.id.clone(), serde_json::json!({}));
            }
            "ping" => Ok(serde_json::json!({})),

            // Tool methods
            "tools/list" => self.handle_tools_list(),
            "tools/call" => self.handle_tools_call(request),

            // Resource methods
            "resources/list" => self.handle_resources_list(),
            "resources/read" => self.handle_resources_read(request),

            // Prompt methods
            "prompts/list" => self.handle_prompts_list(),
            "prompts/get" => self.handle_prompts_get(request),

            _ => Err(McpError::method_not_found(&request.method)),
        };

        match result {
            Ok(value) => JsonRpcResponse::success(request.id.clone(), value),
            Err(e) => JsonRpcResponse::error(request.id.clone(), e),
        }
    }

    /// Handle the initialize request
    fn handle_initialize(&mut self, request: &JsonRpcRequest) -> McpResult<serde_json::Value> {
        let params: InitializeParams = request
            .params
            .as_ref()
            .map(|p| serde_json::from_value(p.clone()))
            .transpose()?
            .unwrap_or(InitializeParams {
                protocol_version: PROTOCOL_VERSION.to_string(),
                capabilities: serde_json::json!({}),
                client_info: None,
            });

        // Store client info for audit logging
        self.client_info = params.client_info;

        let result = InitializeResult {
            protocol_version: PROTOCOL_VERSION.to_string(),
            capabilities: ServerCapabilities {
                tools: ToolCapabilities {
                    list_changed: false,
                },
                resources: ResourceCapabilities {
                    subscribe: false,
                    list_changed: false,
                },
                prompts: PromptCapabilities {
                    list_changed: false,
                },
            },
            server_info: ServerInfo {
                name: SERVER_NAME.to_string(),
                version: SERVER_VERSION.to_string(),
            },
        };

        Ok(serde_json::to_value(result)?)
    }

    /// Handle tools/list request
    fn handle_tools_list(&self) -> McpResult<serde_json::Value> {
        let tools_list = tools::list_tools();
        Ok(serde_json::json!({ "tools": tools_list }))
    }

    /// Handle tools/call request
    fn handle_tools_call(&self, request: &JsonRpcRequest) -> McpResult<serde_json::Value> {
        #[derive(Deserialize)]
        struct ToolsCallParams {
            name: String,
            #[serde(default)]
            arguments: Option<serde_json::Value>,
        }

        let params: ToolsCallParams = request
            .params
            .as_ref()
            .map(|p| serde_json::from_value(p.clone()))
            .transpose()?
            .ok_or_else(|| McpError::invalid_params("Missing params for tools/call"))?;

        let client_name = self.client_info.as_ref().map(|c| c.name.as_str());

        match params.name.as_str() {
            "jarvy_get_install_instructions" => {
                let result = tools::handle_get_install_instructions(params.arguments)?;
                Ok(serde_json::json!({
                    "content": [{
                        "type": "text",
                        "text": serde_json::to_string_pretty(&result)?
                    }]
                }))
            }
            "jarvy_check_self" => {
                let result = tools::handle_check_self()?;
                Ok(serde_json::json!({
                    "content": [{
                        "type": "text",
                        "text": serde_json::to_string_pretty(&result)?
                    }]
                }))
            }
            "jarvy_list_tools" => {
                let result = tools::handle_list_tools(params.arguments)?;
                self.audit_log
                    .log_list_tools(client_name, result["count"].as_u64().unwrap_or(0) as usize);
                Ok(serde_json::json!({
                    "content": [{
                        "type": "text",
                        "text": serde_json::to_string_pretty(&result)?
                    }]
                }))
            }
            "jarvy_get_tool" => {
                let result = tools::handle_get_tool(params.arguments)?;
                Ok(serde_json::json!({
                    "content": [{
                        "type": "text",
                        "text": serde_json::to_string_pretty(&result)?
                    }]
                }))
            }
            "jarvy_check_tool" => {
                self.rate_limiter.check_check_limit().inspect_err(|_e| {
                    self.audit_log.log_rate_limited(client_name, "check_tool");
                })?;

                let result = tools::handle_check_tool(params.arguments)?;
                self.audit_log.log_check_tool(
                    client_name,
                    result["name"].as_str().unwrap_or("unknown"),
                    result["installed"].as_bool().unwrap_or(false),
                    result["version"].as_str(),
                );
                Ok(serde_json::json!({
                    "content": [{
                        "type": "text",
                        "text": serde_json::to_string_pretty(&result)?
                    }]
                }))
            }
            "jarvy_check_multiple" => {
                self.rate_limiter.check_check_limit().inspect_err(|_e| {
                    self.audit_log
                        .log_rate_limited(client_name, "check_multiple");
                })?;

                let result = tools::handle_check_multiple(params.arguments)?;
                Ok(serde_json::json!({
                    "content": [{
                        "type": "text",
                        "text": serde_json::to_string_pretty(&result)?
                    }]
                }))
            }
            "jarvy_install_tool" => {
                let result = tools::handle_install_tool(
                    params.arguments,
                    &self.config,
                    &self.rate_limiter,
                    &self.audit_log,
                    client_name,
                )?;
                Ok(serde_json::json!({
                    "content": [{
                        "type": "text",
                        "text": serde_json::to_string_pretty(&result)?
                    }]
                }))
            }
            // ---- Extended tools (phase 2) -------------------------------
            //
            // Mutating tools take a MutationCtx so the shared guard
            // (rate limit + audit + confirmation + workspace path
            // containment) sits between the agent and any state change.
            "jarvy_ai_hooks_list" => {
                crate::mcp::extended_tools::handle_ai_hooks_list(params.arguments)
            }
            "jarvy_ai_hooks_check" => {
                crate::mcp::extended_tools::handle_ai_hooks_check(params.arguments)
            }
            "jarvy_ai_hooks_apply" => {
                let ctx = self.mutation_ctx(client_name);
                crate::mcp::extended_tools::handle_ai_hooks_apply(params.arguments, &ctx)
            }
            "jarvy_mcp_register_list" => {
                crate::mcp::extended_tools::handle_mcp_register_list(params.arguments)
            }
            "jarvy_mcp_register_check" => {
                crate::mcp::extended_tools::handle_mcp_register_check(params.arguments)
            }
            "jarvy_mcp_register_apply" => {
                let ctx = self.mutation_ctx(client_name);
                crate::mcp::extended_tools::handle_mcp_register_apply(params.arguments, &ctx)
            }
            "jarvy_drift_check" => crate::mcp::extended_tools::handle_drift_check(params.arguments),
            "jarvy_drift_status" => {
                crate::mcp::extended_tools::handle_drift_status(params.arguments)
            }
            "jarvy_roles_list" => crate::mcp::extended_tools::handle_roles_list(params.arguments),
            "jarvy_roles_show" => crate::mcp::extended_tools::handle_roles_show(params.arguments),
            "jarvy_services_status" => {
                crate::mcp::extended_tools::handle_services_status(params.arguments)
            }
            "jarvy_services_start" => {
                let ctx = self.mutation_ctx(client_name);
                crate::mcp::extended_tools::handle_services_start(params.arguments, &ctx)
            }
            "jarvy_templates_list" => {
                crate::mcp::extended_tools::handle_templates_list(params.arguments)
            }
            "jarvy_templates_show" => {
                crate::mcp::extended_tools::handle_templates_show(params.arguments)
            }
            "jarvy_templates_use" => {
                let ctx = self.mutation_ctx(client_name);
                crate::mcp::extended_tools::handle_templates_use(params.arguments, &ctx)
            }
            "jarvy_validate_config" => {
                crate::mcp::extended_tools::handle_validate_config(params.arguments)
            }
            // PRD-044 discover surface
            "jarvy_discover_scan" => {
                crate::mcp::extended_tools::handle_discover_scan(params.arguments)
            }
            "jarvy_discover_apply" => {
                let ctx = self.mutation_ctx(client_name);
                crate::mcp::extended_tools::handle_discover_apply(params.arguments, &ctx)
            }
            // PRD-047 workspace surface
            "jarvy_workspace_list" => {
                crate::mcp::extended_tools::handle_workspace_list(params.arguments)
            }
            "jarvy_workspace_show" => {
                crate::mcp::extended_tools::handle_workspace_show(params.arguments)
            }
            "jarvy_workspace_validate" => {
                crate::mcp::extended_tools::handle_workspace_validate(params.arguments)
            }
            // PRD-054 phase 6 library cache
            "jarvy_library_list" => {
                crate::mcp::extended_tools::handle_library_list(params.arguments)
            }
            "jarvy_library_show" => {
                crate::mcp::extended_tools::handle_library_show(params.arguments)
            }
            // PRD-056 wizard
            "jarvy_wizard_plan" => crate::mcp::extended_tools::handle_wizard_plan(params.arguments),
            _ => Err(McpError::method_not_found(format!(
                "Unknown tool: {}",
                params.name
            ))),
        }
    }

    /// Handle resources/list request
    fn handle_resources_list(&self) -> McpResult<serde_json::Value> {
        let resources_list = resources::list_resources();
        Ok(serde_json::json!({ "resources": resources_list }))
    }

    /// Handle resources/read request
    fn handle_resources_read(&self, request: &JsonRpcRequest) -> McpResult<serde_json::Value> {
        #[derive(Deserialize)]
        struct ResourcesReadParams {
            uri: String,
        }

        let params: ResourcesReadParams = request
            .params
            .as_ref()
            .map(|p| serde_json::from_value(p.clone()))
            .transpose()?
            .ok_or_else(|| McpError::invalid_params("Missing params for resources/read"))?;

        let content = resources::read_resource(&params.uri)?;
        Ok(serde_json::json!({
            "contents": [{
                "uri": params.uri,
                "mimeType": "application/json",
                "text": content
            }]
        }))
    }

    /// Handle prompts/list request
    fn handle_prompts_list(&self) -> McpResult<serde_json::Value> {
        let prompts_list = prompts::list_prompts();
        Ok(serde_json::json!({ "prompts": prompts_list }))
    }

    /// Handle prompts/get request
    fn handle_prompts_get(&self, request: &JsonRpcRequest) -> McpResult<serde_json::Value> {
        #[derive(Deserialize)]
        struct PromptsGetParams {
            name: String,
            #[serde(default)]
            arguments: Option<serde_json::Value>,
        }

        let params: PromptsGetParams = request
            .params
            .as_ref()
            .map(|p| serde_json::from_value(p.clone()))
            .transpose()?
            .ok_or_else(|| McpError::invalid_params("Missing params for prompts/get"))?;

        let prompt = prompts::get_prompt(&params.name, params.arguments)?;
        Ok(prompt)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_creation() {
        let config = McpConfig::default();
        let server = McpServer::new(config);
        assert!(server.client_info.is_none());
    }

    #[test]
    fn test_tools_list_response() {
        let config = McpConfig::default();
        let server = McpServer::new(config);
        let result = server.handle_tools_list().unwrap();
        assert!(result.get("tools").is_some());
    }

    #[test]
    fn test_resources_list_response() {
        let config = McpConfig::default();
        let server = McpServer::new(config);
        let result = server.handle_resources_list().unwrap();
        assert!(result.get("resources").is_some());
    }

    #[test]
    fn test_prompts_list_response() {
        let config = McpConfig::default();
        let server = McpServer::new(config);
        let result = server.handle_prompts_list().unwrap();
        assert!(result.get("prompts").is_some());
    }
}
