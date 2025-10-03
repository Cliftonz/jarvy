use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

static CLIENT: OnceLock<PosthogClient> = OnceLock::new();
static GLOBAL_CONTEXT: OnceLock<Mutex<serde_json::Map<String, serde_json::Value>>> =
    OnceLock::new();

#[derive(Clone)]
pub struct PosthogClient {
    enabled: bool,
    api_key: Option<String>,
    host: String,
    pub distinct_id: String,
}

impl PosthogClient {
    fn is_enabled(&self) -> bool {
        self.enabled && self.api_key.is_some()
    }
}

/// Returns true if telemetry is effectively enabled (user setting + API key present)
pub fn telemetry_enabled() -> bool {
    if let Some(c) = client() {
        c.is_enabled()
    } else {
        false
    }
}

pub fn init(enable_analytics: bool, distinct_id: String) {
    // Allow disabling analytics via env override
    let env_disable = std::env::var("JARVY_ANALYTICS").ok();
    let enabled = match env_disable.as_deref() {
        Some("0") | Some("false") => false,
        _ => enable_analytics,
    };

    let api_key = std::env::var("JARVY_POSTHOG_API_KEY")
        .ok()
        .or_else(|| std::env::var("POSTHOG_API_KEY").ok());
    let host = std::env::var("JARVY_POSTHOG_HOST")
        .unwrap_or_else(|_| "https://app.posthog.com".to_string());

    let client = PosthogClient {
        enabled: enabled && api_key.is_some(),
        api_key,
        host,
        distinct_id,
    };
    let _ = CLIENT.set(client);
    let _ = GLOBAL_CONTEXT.set(Mutex::new(serde_json::Map::new()));
}

pub fn client() -> Option<&'static PosthogClient> {
    CLIENT.get()
}

pub fn set_context(key: &str, value: serde_json::Value) {
    let map = GLOBAL_CONTEXT.get_or_init(|| Mutex::new(serde_json::Map::new()));
    if let Ok(mut m) = map.lock() {
        m.insert(key.to_string(), value);
    }
}

pub fn set_context_map(ctx: serde_json::Map<String, serde_json::Value>) {
    let map = GLOBAL_CONTEXT.get_or_init(|| Mutex::new(serde_json::Map::new()));
    if let Ok(mut m) = map.lock() {
        for (k, v) in ctx {
            m.insert(k, v);
        }
    }
}

pub fn detect_os() -> String {
    std::env::consts::OS.to_string()
}

pub fn detect_shell() -> String {
    // Unix shells usually expose SHELL
    if let Ok(shell) = std::env::var("SHELL") {
        if !shell.trim().is_empty() {
            return shell;
        }
    }
    // Windows powershell/cmd often expose COMSPEC
    if let Ok(comspec) = std::env::var("COMSPEC") {
        if !comspec.trim().is_empty() {
            return comspec;
        }
    }
    // Fallback
    "unknown".to_string()
}

pub fn now() -> Instant {
    Instant::now()
}

pub fn ms(d: Duration) -> u128 {
    d.as_millis()
}

fn merge_global_context(properties: &mut serde_json::Map<String, serde_json::Value>) {
    if let Some(lock) = GLOBAL_CONTEXT.get() {
        if let Ok(map) = lock.lock() {
            for (k, v) in map.iter() {
                properties.entry(k.clone()).or_insert(v.clone());
            }
        }
    }
}

pub fn capture(event: &str, mut properties: serde_json::Map<String, serde_json::Value>) {
    if let Some(c) = client() {
        if !c.is_enabled() {
            return;
        }
        // Merge global context first so automatic context can still override if desired
        merge_global_context(&mut properties);
        // Always add os and shell context if missing
        properties
            .entry("os".to_string())
            .or_insert(serde_json::Value::String(detect_os()));
        properties
            .entry("shell".to_string())
            .or_insert(serde_json::Value::String(detect_shell()));
        properties
            .entry("version".to_string())
            .or_insert(serde_json::Value::String(
                env!("CARGO_PKG_VERSION").to_string(),
            ));

        let payload = serde_json::json!({
            "api_key": c.api_key.as_ref().unwrap(),
            "event": event,
            "distinct_id": c.distinct_id,
            "properties": properties,
        });

        // Fire and forget; ignore errors to avoid affecting CLI UX
        let _ = ureq::post(&format!("{}/capture/", c.host))
            .header("Content-Type", "application/json")
            .send_json(payload.to_string());
    }
}

/// Capture an exception according to PostHog manual error tracking
/// Sends a `$exception` event with recommended properties
pub fn capture_exception(
    message: &str,
    exception_type: &str,
    stack_trace: Option<String>,
    mut context: serde_json::Map<String, serde_json::Value>,
) {
    // Build exception payload according to docs
    let mut props = serde_json::Map::new();
    props.insert(
        "$exception_message".to_string(),
        serde_json::Value::String(message.to_string()),
    );
    props.insert(
        "$exception_type".to_string(),
        serde_json::Value::String(exception_type.to_string()),
    );
    if let Some(stack) = stack_trace {
        if !stack.is_empty() {
            props.insert(
                "$exception_stack_trace".to_string(),
                serde_json::Value::String(stack),
            );
        }
    }
    if !context.is_empty() {
        props.insert(
            "$exception_properties".to_string(),
            serde_json::Value::Object(context),
        );
    }
    capture("$exception", props);
}

pub fn capture_error(
    event: &str,
    message: &str,
    mut properties: serde_json::Map<String, serde_json::Value>,
) {
    properties.insert(
        "error".to_string(),
        serde_json::Value::String(message.to_string()),
    );
    capture(event, properties);
}

pub fn identify(mut traits: HashMap<String, serde_json::Value>) {
    if let Some(c) = client() {
        if !c.is_enabled() {
            return;
        }
        // Identify via $identify capture event
        let mut props = serde_json::Map::new();
        props.insert(
            "$set".to_string(),
            serde_json::Value::Object(traits.drain().collect()),
        );
        capture("$identify", props);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_detects_something() {
        let s = detect_shell();
        assert!(!s.is_empty());
    }
}
