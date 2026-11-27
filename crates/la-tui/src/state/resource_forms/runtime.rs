//! Modal form builders and submit handlers for runtime JSON Antigravity resources:
//! `Hooks` (`.agents/hooks.json`) and `McpServers` (`.agents/mcp_config.json`).

use std::collections::BTreeMap;

use la_core::manifest::{HookMatcher, HookSpec, McpServerSpec, McpTransport, McpVariable};
use la_core::{HookEvent, ProjectManifest, ResourceId, TargetKind};

use crate::action::StudioCommand;
use crate::form::{
    FieldValidator, FormField, FormTab, KeyValueEntry, KeyValueListState, ModalFormState,
};
use crate::i18n::I18n;
use crate::state::WorkspaceSnapshot;

use super::{FormTarget, capability_label, opt_text, parse_capability};

const HOOK_EVENTS: [&str; 6] = [
    "SessionStart",
    "PreToolUse",
    "PostToolUse",
    "PreInvocation",
    "PostInvocation",
    "Stop",
];

struct McpFormDefaults {
    transport_idx: usize,
    url: String,
    command: String,
    args: Vec<String>,
    cwd: String,
    kv_pairs: Vec<KeyValueEntry>,
    var_entries: Vec<KeyValueEntry>,
}

pub(super) fn build_hook_form(
    edit_idx: Option<usize>,
    manifest: &ProjectManifest,
    i18n: I18n,
) -> (ModalFormState, FormTarget) {
    let existing = edit_idx.and_then(|idx| manifest.hooks.get(idx));
    let event_idx = existing.map_or(1, |h| match h.event {
        HookEvent::SessionStart => 0,
        HookEvent::PreToolUse => 1,
        HookEvent::PostToolUse => 2,
        HookEvent::PreInvocation => 3,
        HookEvent::PostInvocation => 4,
        HookEvent::Stop => 5,
    });
    let (matcher_mode, matcher_val) = match existing.and_then(|h| h.matcher.as_ref()) {
        Some(HookMatcher::Capability { capability }) => (0, capability_label(capability)),
        Some(HookMatcher::Regex { regex }) => (1, regex.clone()),
        None => (2, String::new()),
    };
    let timeout_str = existing.map_or_else(|| "15".to_owned(), |h| h.timeout_secs.to_string());

    let title = if edit_idx.is_some() {
        i18n.pick("Edit Lifecycle Hook", "编辑生命周期钩子")
    } else {
        i18n.pick("Add Lifecycle Hook", "新建生命周期钩子")
    };
    let form = ModalFormState::new(
        title,
        vec![
            FormField::text(
                "name",
                i18n.pick("Hook Name", "钩子名称"),
                existing.map_or("", |h| h.name.as_str()),
                "custom-lint-guard",
                FieldValidator::ResourceId,
            ),
            FormField::enum_select(
                "event",
                i18n.pick("Lifecycle Event", "触发事件"),
                HOOK_EVENTS.into_iter().map(str::to_owned).collect(),
                event_idx,
            ),
            FormField::enum_select(
                "matcher_mode",
                i18n.pick("Matcher Kind", "匹配方式"),
                vec!["capability".into(), "regex".into(), "none".into()],
                matcher_mode,
            ),
            FormField::text(
                "matcher_value",
                i18n.pick("Matcher Value", "匹配值"),
                matcher_val,
                "shell or run_command|edit_file",
                FieldValidator::Regex,
            ),
            FormField::text(
                "command",
                i18n.pick("Command", "执行命令"),
                existing.map_or("lazyagents hook eval git-guard", |h| h.command.as_str()),
                "lazyagents hook eval git-guard",
                FieldValidator::Required,
            ),
            FormField::text(
                "timeout_secs",
                i18n.pick("Timeout (sec)", "超时秒数"),
                timeout_str,
                "15",
                FieldValidator::PositiveU32,
            ),
            FormField::toggle(
                "enabled",
                i18n.pick("Enabled", "启用"),
                existing.is_none_or(|h| h.enabled),
            ),
        ],
    );
    (form, FormTarget::Hook { edit_idx })
}

pub(super) fn build_mcp_form(
    edit_idx: Option<usize>,
    manifest: &ProjectManifest,
    i18n: I18n,
) -> (ModalFormState, FormTarget) {
    let existing = edit_idx.and_then(|idx| manifest.mcp_servers.get(idx));
    let defaults = extract_mcp_defaults(existing);
    let title = if edit_idx.is_some() {
        i18n.pick("Edit MCP Server", "编辑 MCP 服务")
    } else {
        i18n.pick("Add MCP Server", "新建 MCP 服务")
    };
    let form = ModalFormState::new(
        title,
        vec![
            FormField::text(
                "name",
                i18n.pick("Server Name", "服务名称"),
                existing.map_or("", |m| m.name.as_str()),
                "docs-mcp",
                FieldValidator::ResourceId,
            ),
            FormField::enum_select(
                "transport",
                i18n.pick("Transport", "传输协议"),
                vec![
                    "streamable_http".into(),
                    "stdio".into(),
                    "legacy_sse".into(),
                ],
                defaults.transport_idx,
            ),
            FormField::text(
                "url",
                i18n.pick("HTTP URL", "远程 URL"),
                defaults.url,
                "https://api.githubcopilot.com/mcp/",
                FieldValidator::None,
            ),
            FormField::text(
                "command",
                i18n.pick("Stdio Command", "Stdio 命令"),
                defaults.command,
                "npx",
                FieldValidator::None,
            ),
            FormField::string_list(
                "args",
                i18n.pick("Stdio Args", "命令参数"),
                defaults.args,
                "-y",
                FieldValidator::Required,
            ),
            FormField::text(
                "cwd",
                i18n.pick("Stdio CWD", "工作目录"),
                defaults.cwd,
                "/abs/path (optional)",
                FieldValidator::None,
            ),
            FormField::toggle(
                "enabled",
                i18n.pick("Enabled", "启用"),
                existing.is_none_or(|m| m.enabled),
            ),
            FormField::kv_list(
                "env_or_headers",
                i18n.pick("Headers / Env", "请求头 / 环境变量"),
                KeyValueListState::new(defaults.kv_pairs, "Key", "Value (${VAR})", None, true),
            )
            .on_tab(1),
            FormField::kv_list(
                "variables",
                i18n.pick("Setup Variables", "声明变量"),
                KeyValueListState::new(
                    defaults.var_entries,
                    "VAR_NAME",
                    "Title",
                    Some("Description".to_owned()),
                    true,
                ),
            )
            .on_tab(1),
        ],
    )
    .with_tabs(vec![
        FormTab::general(i18n.pick("Transport", "传输连接")),
        FormTab::for_target(
            i18n.pick("Vars & Headers", "变量与请求头"),
            TargetKind::Antigravity,
        ),
    ]);
    (form, FormTarget::McpServer { edit_idx })
}

fn extract_mcp_defaults(existing: Option<&McpServerSpec>) -> McpFormDefaults {
    let (transport_idx, url, command, args, cwd, kv_pairs) = match existing.map(|m| &m.transport) {
        Some(McpTransport::Stdio {
            command,
            args,
            env,
            cwd,
        }) => (
            1,
            String::new(),
            command.clone(),
            args.clone(),
            cwd.clone().unwrap_or_default(),
            map_to_kv_entries(env),
        ),
        Some(McpTransport::LegacySse { url, headers }) => (
            2,
            url.clone(),
            String::new(),
            Vec::new(),
            String::new(),
            map_to_kv_entries(headers),
        ),
        Some(McpTransport::StreamableHttp { url, headers }) => (
            0,
            url.clone(),
            String::new(),
            Vec::new(),
            String::new(),
            map_to_kv_entries(headers),
        ),
        None => (
            0,
            "https://mcp.example.com/mcp".to_owned(),
            String::new(),
            Vec::new(),
            String::new(),
            Vec::new(),
        ),
    };
    let var_entries: Vec<KeyValueEntry> = existing.map_or_default(|m| {
        m.variables
            .iter()
            .map(|v| KeyValueEntry {
                key: v.name.clone(),
                value: v.title.clone(),
                extra: Some(v.description.clone()),
            })
            .collect()
    });

    McpFormDefaults {
        transport_idx,
        url,
        command,
        args,
        cwd,
        kv_pairs,
        var_entries,
    }
}

pub(super) fn apply_hook(
    edit_idx: Option<usize>,
    form: &ModalFormState,
    workspace: &mut WorkspaceSnapshot,
) -> Result<(String, Vec<StudioCommand>), String> {
    let name = ResourceId::parse(form.text_value("name").unwrap_or_default())
        .map_err(|err| err.to_string())?;
    let event = match form.enum_value("event").unwrap_or("PreToolUse") {
        "SessionStart" => HookEvent::SessionStart,
        "PostToolUse" => HookEvent::PostToolUse,
        "PreInvocation" => HookEvent::PreInvocation,
        "PostInvocation" => HookEvent::PostInvocation,
        "Stop" => HookEvent::Stop,
        _ => HookEvent::PreToolUse,
    };
    let matcher_val = form.text_value("matcher_value").unwrap_or_default();
    let matcher = match form.enum_value("matcher_mode").unwrap_or("none") {
        "capability" if !matcher_val.is_empty() => Some(HookMatcher::Capability {
            capability: parse_capability(matcher_val),
        }),
        "regex" if !matcher_val.is_empty() => Some(HookMatcher::Regex {
            regex: matcher_val.to_owned(),
        }),
        _ => None,
    };
    let command = opt_text(form, "command").ok_or_else(|| "Hook command is required".to_owned())?;
    let timeout_secs: u32 = form
        .text_value("timeout_secs")
        .unwrap_or("30")
        .parse()
        .map_err(|_| "Timeout must be a positive integer".to_owned())?;
    if timeout_secs == 0 {
        return Err("Timeout must be greater than 0".to_owned());
    }

    let duplicate = workspace
        .manifest
        .hooks
        .iter()
        .enumerate()
        .any(|(i, h)| Some(i) != edit_idx && h.name == name);
    if duplicate {
        return Err(format!("Hook `{name}` already exists"));
    }

    let spec = HookSpec {
        name: name.clone(),
        enabled: form.bool_value("enabled").unwrap_or(true),
        event,
        matcher,
        command,
        timeout_secs,
    };
    if let Some(idx) = edit_idx {
        let Some(slot) = workspace.manifest.hooks.get_mut(idx) else {
            return Err("Hook no longer exists".to_owned());
        };
        *slot = spec;
        Ok((format!("Updated hook `{name}`"), Vec::new()))
    } else {
        workspace.manifest.hooks.push(spec);
        Ok((format!("Added hook `{name}`"), Vec::new()))
    }
}

pub(super) fn apply_mcp(
    edit_idx: Option<usize>,
    form: &ModalFormState,
    workspace: &mut WorkspaceSnapshot,
) -> Result<(String, Vec<StudioCommand>), String> {
    let name = ResourceId::parse(form.text_value("name").unwrap_or_default())
        .map_err(|err| err.to_string())?;
    let duplicate = workspace
        .manifest
        .mcp_servers
        .iter()
        .enumerate()
        .any(|(i, m)| Some(i) != edit_idx && m.name == name);
    if duplicate {
        return Err(format!("MCP server `{name}` already exists"));
    }

    let kv_map = kv_entries_to_map(form.kv_entries("env_or_headers").unwrap_or(&[]));
    let transport = match form.enum_value("transport").unwrap_or("streamable_http") {
        "stdio" => {
            let command = opt_text(form, "command")
                .ok_or_else(|| "Stdio MCP server requires a `command`".to_owned())?;
            let args = form
                .string_list_values("args")
                .map_or_default(<[String]>::to_vec);
            McpTransport::Stdio {
                command,
                args,
                env: kv_map,
                cwd: opt_text(form, "cwd"),
            }
        }
        "legacy_sse" => {
            let url = opt_text(form, "url")
                .ok_or_else(|| "SSE MCP server requires a `url`".to_owned())?;
            McpTransport::LegacySse {
                url,
                headers: kv_map,
            }
        }
        _ => {
            let url = opt_text(form, "url")
                .ok_or_else(|| "Streamable HTTP MCP server requires a `url`".to_owned())?;
            McpTransport::StreamableHttp {
                url,
                headers: kv_map,
            }
        }
    };

    let variables: Vec<McpVariable> = form
        .kv_entries("variables")
        .unwrap_or(&[])
        .iter()
        .map(|entry| McpVariable {
            name: entry.key.clone(),
            title: if entry.value.is_empty() {
                entry.key.clone()
            } else {
                entry.value.clone()
            },
            description: entry.extra.clone().unwrap_or_default(),
        })
        .collect();

    let spec = McpServerSpec {
        name: name.clone(),
        enabled: form.bool_value("enabled").unwrap_or(true),
        transport,
        variables,
    };
    if let Some(idx) = edit_idx {
        let Some(slot) = workspace.manifest.mcp_servers.get_mut(idx) else {
            return Err("MCP server no longer exists".to_owned());
        };
        *slot = spec;
        Ok((format!("Updated MCP server `{name}`"), Vec::new()))
    } else {
        workspace.manifest.mcp_servers.push(spec);
        Ok((format!("Added MCP server `{name}`"), Vec::new()))
    }
}

fn map_to_kv_entries(map: &BTreeMap<String, String>) -> Vec<KeyValueEntry> {
    map.iter()
        .map(|(k, v)| KeyValueEntry {
            key: k.clone(),
            value: v.clone(),
            extra: None,
        })
        .collect()
}

fn kv_entries_to_map(entries: &[KeyValueEntry]) -> BTreeMap<String, String> {
    entries
        .iter()
        .map(|e| (e.key.clone(), e.value.clone()))
        .collect()
}
