//! Antigravity JSON runtime asset compilers (`hooks.json`, `mcp_config.json`,
//! and declarative `{skills,rules,plugins}.json` index manifests).

use std::collections::BTreeMap;

use serde::Serialize;

use crate::compiler::{CompatibilityDiagnostic, CompatibilitySeverity, GeneratedFile};
use crate::error::CoreError;
use crate::id::{TargetKind, ToolCapability};
use crate::manifest::{
    HookMatcher, HookSpec, IndexManifestSpec, McpServerSpec, McpTransport, McpVariable,
    ProjectManifest,
};

#[derive(Serialize)]
struct WireHookHandler<'a> {
    #[serde(rename = "type")]
    kind: &'static str,
    command: &'a str,
    timeout: u32,
}

#[derive(Serialize)]
struct WireToolMatcherGroup<'a> {
    matcher: String,
    hooks: Vec<WireHookHandler<'a>>,
}

#[derive(Serialize)]
struct WireMcpConfig<'a> {
    #[serde(rename = "mcpServers")]
    mcp_servers: BTreeMap<String, WireMcpServer<'a>>,
}

#[derive(Serialize)]
struct WireMcpServer<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    command: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    args: Option<&'a [String]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    env: Option<&'a BTreeMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cwd: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    url: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    headers: Option<&'a BTreeMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    disabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    variables: Option<&'a [McpVariable]>,
}

pub(super) fn render_hooks_json(hooks: &[HookSpec]) -> Result<String, CoreError> {
    let mut root = serde_json::Map::new();
    for hook in hooks {
        let mut entry = serde_json::Map::new();
        if !hook.enabled {
            entry.insert("enabled".to_owned(), serde_json::Value::Bool(false));
        }
        let handler = WireHookHandler {
            kind: "command",
            command: &hook.command,
            timeout: hook.timeout_secs,
        };
        let event_key = hook.event.antigravity_key().to_owned();
        if hook.event.uses_tool_matcher() {
            let matcher = resolve_antigravity_matcher(hook.matcher.as_ref());
            let group = WireToolMatcherGroup {
                matcher,
                hooks: vec![handler],
            };
            let val = serde_json::to_value(vec![group]).map_err(|err| CoreError::Compile {
                message: err.to_string(),
            })?;
            entry.insert(event_key, val);
        } else {
            let val = serde_json::to_value(vec![handler]).map_err(|err| CoreError::Compile {
                message: err.to_string(),
            })?;
            entry.insert(event_key, val);
        }
        root.insert(
            hook.name.as_str().to_owned(),
            serde_json::Value::Object(entry),
        );
    }

    let mut out =
        serde_json::to_string_pretty(&serde_json::Value::Object(root)).map_err(|err| {
            CoreError::Compile {
                message: err.to_string(),
            }
        })?;
    out.push('\n');
    Ok(out)
}

pub(super) fn render_mcp_config_json(
    servers: &[McpServerSpec],
    diagnostics: &mut Vec<CompatibilityDiagnostic>,
) -> Result<String, CoreError> {
    let mut map = BTreeMap::new();
    for server in servers {
        let disabled = if server.enabled { None } else { Some(true) };
        let variables = if server.variables.is_empty() {
            None
        } else {
            Some(server.variables.as_slice())
        };
        let wire = match &server.transport {
            McpTransport::Stdio {
                command,
                args,
                env,
                cwd,
            } => WireMcpServer {
                command: Some(command.as_str()),
                args: (!args.is_empty()).then_some(args.as_slice()),
                env: (!env.is_empty()).then_some(env),
                cwd: cwd.as_deref(),
                url: None,
                headers: None,
                disabled,
                variables,
            },
            McpTransport::StreamableHttp { url, headers } => WireMcpServer {
                command: None,
                args: None,
                env: None,
                cwd: None,
                url: Some(url.as_str()),
                headers: (!headers.is_empty()).then_some(headers),
                disabled,
                variables,
            },
            McpTransport::LegacySse { url, headers } => {
                diagnostics.push(CompatibilityDiagnostic {
                    target: TargetKind::Antigravity,
                    severity: CompatibilitySeverity::Warning,
                    subject: format!("mcp:{}", server.name),
                    message: format!(
                        "Antigravity does not support legacy SSE transport (`{url}`); switch to Streamable HTTP or bridge via `npx -y mcp-remote {url}`"
                    ),
                });
                WireMcpServer {
                    command: None,
                    args: None,
                    env: None,
                    cwd: None,
                    url: Some(url.as_str()),
                    headers: (!headers.is_empty()).then_some(headers),
                    disabled,
                    variables,
                }
            }
        };
        map.insert(server.name.as_str().to_owned(), wire);
    }

    let cfg = WireMcpConfig { mcp_servers: map };
    let mut out = serde_json::to_string_pretty(&cfg).map_err(|err| CoreError::Compile {
        message: err.to_string(),
    })?;
    out.push('\n');
    Ok(out)
}

pub(super) fn compile_index_manifests(
    manifest: &ProjectManifest,
    out: &mut Vec<GeneratedFile>,
) -> Result<(), CoreError> {
    let specs: [(&str, &IndexManifestSpec); 3] = [
        (".agents/skills.json", &manifest.manifests.skills),
        (".agents/rules.json", &manifest.manifests.rules),
        (".agents/plugins.json", &manifest.manifests.plugins),
    ];
    for (path, spec) in specs {
        if spec.is_empty() {
            continue;
        }
        let mut json = serde_json::to_string_pretty(spec).map_err(|err| CoreError::Compile {
            message: err.to_string(),
        })?;
        json.push('\n');
        out.push(GeneratedFile::whole_file(path, json));
    }
    Ok(())
}

fn resolve_antigravity_matcher(matcher: Option<&HookMatcher>) -> String {
    match matcher {
        None => "*".to_owned(),
        Some(HookMatcher::Regex { regex }) => regex.clone(),
        Some(HookMatcher::Capability { capability }) => match capability {
            ToolCapability::Shell => "run_command".to_owned(),
            ToolCapability::FileRead => "read_file".to_owned(),
            ToolCapability::FileWrite => "edit_file|write_file".to_owned(),
            ToolCapability::Search => "grep_search|find_by_name".to_owned(),
            ToolCapability::WebSearch => "search_web".to_owned(),
            ToolCapability::Browser => "browser_.*".to_owned(),
            ToolCapability::Subagent => "invoke_subagent".to_owned(),
            ToolCapability::Custom(custom) => custom.clone(),
        },
    }
}
