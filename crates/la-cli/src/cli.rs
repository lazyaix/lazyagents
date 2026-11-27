//! Command-line argument definitions (`clap` 4 derive) for `lazyagents`.

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};
use la_core::manifest::{GitOutputMode, RuleTrigger};
use la_core::{HookEvent, HookProtocol, TargetKind};
use la_runtime::ImportSource;
use la_tui::{GlyphSet, MotionLevel, ShaderPreset, ThemeId, UiLang};

/// Effective CLI version string (`LAZYAGENTS_RELEASE_VERSION` when built from a manual release
/// tag, falling back to `CARGO_PKG_VERSION`).
pub(crate) const APP_VERSION: &str = match option_env!("LAZYAGENTS_RELEASE_VERSION") {
    Some(version) => version,
    None => env!("CARGO_PKG_VERSION"),
};

/// `lazyagents` — Multi-Agent Project Configuration Studio, Engineering Governance Compiler,
/// and Hook Policy Engine.
#[derive(Debug, Parser)]
#[command(
    name = "lazyagents",
    version = APP_VERSION,
    about = "Multi-Agent Project Configuration Studio, Engineering Governance Compiler, and Hook Policy Engine"
)]
pub(crate) struct Cli {
    /// Target workspace directory (defaults to the current working directory).
    #[arg(short = 'C', long = "dir", global = true)]
    pub dir: Option<PathBuf>,

    /// Interface language override (`en` or `zh`, overriding `studio.toml` & locale hints).
    #[arg(long, value_enum, global = true)]
    pub lang: Option<CliLang>,

    /// Never launch the interactive TUI even when stdout is a TTY.
    #[arg(long, global = true)]
    pub no_tui: bool,

    /// Visual theme override (`auto`, `cyber-neon`, `tokyo-night`, `catppuccin-mocha`, `catppuccin-latte`).
    #[arg(long, value_enum, global = true)]
    pub theme: Option<CliTheme>,

    /// `TachyonFX` shader intensity preset (`cyber-overdrive`, `aurora-sweep`, `crisp-minimal`).
    #[arg(long, value_enum, global = true)]
    pub shader: Option<CliShader>,

    /// Visual motion tier for the TUI (`full`, `reduced`, `none`).
    #[arg(long, value_enum, global = true)]
    pub motion: Option<CliMotion>,

    /// Icon and box-drawing glyph fallback tier (`nerd-font`, `unicode`, `ascii`).
    #[arg(long, value_enum, global = true)]
    pub glyphs: Option<CliGlyphs>,

    /// Launch the TUI in compact inline viewport mode preserving scrollback (`--inline` or `--inline=20`).
    #[arg(
        long,
        value_name = "HEIGHT",
        num_args = 0..=1,
        require_equals = true,
        default_missing_value = "18",
        global = true
    )]
    pub inline: Option<u16>,

    /// Disable the animated TUI startup splash banner.
    #[arg(long, global = true)]
    pub no_splash: bool,

    /// Disable mouse capture inside the interactive TUI.
    #[arg(long, global = true)]
    pub no_mouse: bool,

    /// Subcommand to execute headlessly (omit in an interactive TTY to launch the TUI).
    #[command(subcommand)]
    pub command: Option<Command>,
}

/// Supported CLI/TUI interface languages.
#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub(crate) enum CliLang {
    /// English (default).
    En,
    /// Simplified Chinese.
    Zh,
}

impl From<CliLang> for UiLang {
    fn from(value: CliLang) -> Self {
        match value {
            CliLang::En => Self::En,
            CliLang::Zh => Self::Zh,
        }
    }
}

/// Built-in visual color theme selector mapped to [`ThemeId`].
#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub(crate) enum CliTheme {
    Auto,
    CyberNeon,
    TokyoNight,
    CatppuccinMocha,
    CatppuccinLatte,
}

impl From<CliTheme> for ThemeId {
    fn from(value: CliTheme) -> Self {
        match value {
            CliTheme::Auto => Self::Auto,
            CliTheme::CyberNeon => Self::CyberNeon,
            CliTheme::TokyoNight => Self::TokyoNight,
            CliTheme::CatppuccinMocha => Self::CatppuccinMocha,
            CliTheme::CatppuccinLatte => Self::CatppuccinLatte,
        }
    }
}

/// `TachyonFX` shader preset selector mapped to [`ShaderPreset`].
#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub(crate) enum CliShader {
    CyberOverdrive,
    AuroraSweep,
    CrispMinimal,
}

impl From<CliShader> for ShaderPreset {
    fn from(value: CliShader) -> Self {
        match value {
            CliShader::CyberOverdrive => Self::CyberOverdrive,
            CliShader::AuroraSweep => Self::AuroraSweep,
            CliShader::CrispMinimal => Self::CrispMinimal,
        }
    }
}

/// Glyph set fallback tier mapped to [`GlyphSet`].
#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub(crate) enum CliGlyphs {
    NerdFont,
    Unicode,
    Ascii,
}

impl From<CliGlyphs> for GlyphSet {
    fn from(value: CliGlyphs) -> Self {
        match value {
            CliGlyphs::NerdFont => Self::NerdFont,
            CliGlyphs::Unicode => Self::Unicode,
            CliGlyphs::Ascii => Self::Ascii,
        }
    }
}

/// Motion accessibility tier mapped to [`MotionLevel`].
#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub(crate) enum CliMotion {
    Full,
    Reduced,
    None,
}

impl From<CliMotion> for MotionLevel {
    fn from(value: CliMotion) -> Self {
        match value {
            CliMotion::Full => Self::Full,
            CliMotion::Reduced => Self::Reduced,
            CliMotion::None => Self::None,
        }
    }
}

/// Target filter flag accepted by `generate`, `diff`, and `check`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub(crate) enum CliTarget {
    All,
    Antigravity,
    ClaudeCode,
    Codex,
    Cursor,
    GithubCopilot,
}

impl CliTarget {
    pub(crate) const fn as_filter(self) -> Option<&'static [TargetKind]> {
        match self {
            Self::All => None,
            Self::Antigravity => Some(&[TargetKind::Antigravity]),
            Self::ClaudeCode => Some(&[TargetKind::ClaudeCode]),
            Self::Codex => Some(&[TargetKind::Codex]),
            Self::Cursor => Some(&[TargetKind::Cursor]),
            Self::GithubCopilot => Some(&[TargetKind::GitHubCopilot]),
        }
    }
}

/// Output format for `check`, `inspect`, and `github audit`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub(crate) enum OutputFormat {
    Text,
    Json,
    Github,
}

/// Git ownership mode selected during `lazyagents init`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub(crate) enum CliGitMode {
    Commit,
    Ignore,
}

impl From<CliGitMode> for GitOutputMode {
    fn from(value: CliGitMode) -> Self {
        match value {
            CliGitMode::Commit => Self::Commit,
            CliGitMode::Ignore => Self::Ignore,
        }
    }
}

/// Source format selector for `lazyagents import`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub(crate) enum CliImportSource {
    Auto,
    Antigravity,
    Claude,
    Codex,
}

impl From<CliImportSource> for ImportSource {
    fn from(value: CliImportSource) -> Self {
        match value {
            CliImportSource::Auto => Self::Auto,
            CliImportSource::Antigravity => Self::Antigravity,
            CliImportSource::Claude => Self::ClaudeCode,
            CliImportSource::Codex => Self::Codex,
        }
    }
}

/// Resource kind managed by `add`, `remove`, `enable`, and `disable`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub(crate) enum ResourceKind {
    Section,
    Rule,
    Skill,
    Agent,
    Hook,
    Mcp,
    Plugin,
    Target,
}

/// Rule trigger flag for `lazyagents add rule`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub(crate) enum CliRuleTrigger {
    AlwaysOn,
    Glob,
    ModelDecision,
    Manual,
}

impl From<CliRuleTrigger> for RuleTrigger {
    fn from(value: CliRuleTrigger) -> Self {
        match value {
            CliRuleTrigger::AlwaysOn => Self::AlwaysOn,
            CliRuleTrigger::Glob => Self::Glob,
            CliRuleTrigger::ModelDecision => Self::ModelDecision,
            CliRuleTrigger::Manual => Self::Manual,
        }
    }
}

/// Lifecycle hook event flag for `lazyagents add hook`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub(crate) enum CliHookEvent {
    SessionStart,
    UserPromptSubmit,
    PreInvocation,
    PreToolUse,
    PostToolUse,
    PostInvocation,
    PreCompact,
    Stop,
    SessionEnd,
}

impl From<CliHookEvent> for HookEvent {
    fn from(value: CliHookEvent) -> Self {
        match value {
            CliHookEvent::SessionStart => Self::SessionStart,
            CliHookEvent::UserPromptSubmit => Self::UserPromptSubmit,
            CliHookEvent::PreInvocation => Self::PreInvocation,
            CliHookEvent::PreToolUse => Self::PreToolUse,
            CliHookEvent::PostToolUse => Self::PostToolUse,
            CliHookEvent::PostInvocation => Self::PostInvocation,
            CliHookEvent::PreCompact => Self::PreCompact,
            CliHookEvent::Stop => Self::Stop,
            CliHookEvent::SessionEnd => Self::SessionEnd,
        }
    }
}

/// Wire protocol flag for `lazyagents hook eval`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub(crate) enum CliHookProtocol {
    Antigravity,
    ClaudeCode,
    Codex,
    Cursor,
    GithubCopilot,
}

impl From<CliHookProtocol> for HookProtocol {
    fn from(value: CliHookProtocol) -> Self {
        match value {
            CliHookProtocol::Antigravity => Self::Antigravity,
            CliHookProtocol::ClaudeCode => Self::ClaudeCode,
            CliHookProtocol::Codex => Self::Codex,
            CliHookProtocol::Cursor => Self::Cursor,
            CliHookProtocol::GithubCopilot => Self::GitHubCopilot,
        }
    }
}

/// Top-level CLI subcommands.
#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    /// Detect project stack and bootstrap `.lazyagents/lazyagents.toml`.
    Init(InitArgs),
    /// Ingest existing repository `AGENTS.md` and `.agents/**` into `.lazyagents/`.
    Import(ImportArgs),
    /// Deterministically compile `.lazyagents/` to target outputs and update `lock.json`.
    #[command(visible_alias = "sync")]
    Generate(GenerateArgs),
    /// Preview unified diff between compiled output and workspace files without writing.
    Diff(DiffArgs),
    /// Run CI compliance gate (schema, Antigravity rules, secrets, budgets, Actions, drift).
    Check(CheckArgs),
    /// Print machine-readable probed facts, budgets, enabled targets, and scope provenance.
    Inspect(InspectArgs),
    /// Add or scaffold a managed resource (`section|rule|skill|agent|hook|mcp|plugin`).
    Add(AddArgs),
    /// Remove a managed resource from `.lazyagents/`.
    Remove(RemoveArgs),
    /// Enable a managed resource or target.
    Enable(ToggleArgs),
    /// Disable a managed resource or target.
    Disable(ToggleArgs),
    /// Evaluate or dry-run test built-in Rust engineering guardrail hooks.
    Hook {
        /// Hook operation to perform.
        #[command(subcommand)]
        command: HookCommand,
    },
    /// Read-only live GitHub governance operations via local `gh` CLI.
    Github {
        /// GitHub operation to perform.
        #[command(subcommand)]
        command: GithubCommand,
    },
}

/// Arguments for `lazyagents init`.
#[derive(Debug, Args)]
pub(crate) struct InitArgs {
    /// Optional built-in `AGENTS.md` section preset ID to append.
    #[arg(long)]
    pub preset: Option<String>,
    /// Import existing `AGENTS.md` and `.agents/**` while initializing.
    #[arg(long)]
    pub import: bool,
    /// Git policy for generated target files (`commit` or `ignore`).
    #[arg(long, value_enum, default_value_t = CliGitMode::Commit)]
    pub git_mode: CliGitMode,
    /// Overwrite an existing `.lazyagents/lazyagents.toml` non-interactively.
    #[arg(long, short = 'y')]
    pub yes: bool,
}

/// Arguments for `lazyagents import`.
#[derive(Debug, Args)]
pub(crate) struct ImportArgs {
    /// Source agent configuration layout to import.
    #[arg(long, value_enum, default_value_t = CliImportSource::Auto)]
    pub from: CliImportSource,
    /// Print what would be imported without writing `.lazyagents/`.
    #[arg(long)]
    pub dry_run: bool,
}

/// Arguments for `lazyagents generate` (`sync`).
#[derive(Debug, Args)]
pub(crate) struct GenerateArgs {
    /// Compilation target filter (`all`, `antigravity`, …).
    #[arg(long, value_enum, default_value_t = CliTarget::All)]
    pub target: CliTarget,
    /// Preview planned changes without writing files or updating `lock.json`.
    #[arg(long)]
    pub dry_run: bool,
    /// Overwrite externally modified managed files instead of blocking.
    #[arg(long)]
    pub force: bool,
    /// Merge optional gitignored `.lazyagents/local.toml` overrides.
    #[arg(long)]
    pub include_local: bool,
}

/// Arguments for `lazyagents diff`.
#[derive(Debug, Args)]
pub(crate) struct DiffArgs {
    /// Compilation target filter.
    #[arg(long, value_enum, default_value_t = CliTarget::All)]
    pub target: CliTarget,
    /// Exit with code `1` when any file differs from compiled output.
    #[arg(long)]
    pub exit_code: bool,
    /// Merge optional gitignored `.lazyagents/local.toml` overrides.
    #[arg(long)]
    pub include_local: bool,
}

/// Arguments for `lazyagents check`.
#[derive(Debug, Args)]
pub(crate) struct CheckArgs {
    /// Compilation target filter.
    #[arg(long, value_enum, default_value_t = CliTarget::All)]
    pub target: CliTarget,
    /// Treat warnings as failures (exit code `1`).
    #[arg(long)]
    pub strict: bool,
    /// Output format (`text`, `json`, or `github` workflow annotations).
    #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
    pub format: OutputFormat,
    /// Merge optional gitignored `.lazyagents/local.toml` overrides.
    #[arg(long)]
    pub include_local: bool,
}

/// Arguments for `lazyagents inspect`.
#[derive(Debug, Args)]
pub(crate) struct InspectArgs {
    /// Output format (`text` or `json`).
    #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
    pub format: OutputFormat,
    /// Merge optional gitignored `.lazyagents/local.toml` overrides.
    #[arg(long)]
    pub include_local: bool,
}

/// Arguments for `lazyagents add <kind> <name>`.
#[derive(Debug, Args)]
pub(crate) struct AddArgs {
    /// Kind of resource to add.
    #[arg(value_enum)]
    pub kind: ResourceKind,
    /// Resource identifier or kebab-case name.
    pub name: String,
    /// Optional built-in preset ID (for `section` or `skill`).
    #[arg(long)]
    pub preset: Option<String>,
    /// Rule activation trigger (for `add rule`).
    #[arg(long, value_enum, default_value_t = CliRuleTrigger::Glob)]
    pub trigger: CliRuleTrigger,
    /// File globs for `add rule --trigger glob` (repeatable).
    #[arg(long = "glob")]
    pub globs: Vec<String>,
    /// Concise description for `rule`, `skill`, `agent`, or `plugin`.
    #[arg(long)]
    pub description: Option<String>,
    /// Lifecycle event for `add hook`.
    #[arg(long, value_enum, default_value_t = CliHookEvent::PreToolUse)]
    pub event: CliHookEvent,
    /// Command executed by `add hook` or `add mcp` (stdio).
    #[arg(long)]
    pub command: Option<String>,
    /// Streamable HTTP URL for `add mcp`.
    #[arg(long)]
    pub url: Option<String>,
    /// Write the addition into gitignored `.lazyagents/local.toml`.
    #[arg(long)]
    pub local: bool,
}

/// Arguments for `lazyagents remove <kind> <name>`.
#[derive(Debug, Args)]
pub(crate) struct RemoveArgs {
    /// Kind of resource to remove.
    #[arg(value_enum)]
    pub kind: ResourceKind,
    /// Resource identifier or name to remove.
    pub name: String,
    /// Also delete backing source files under `.lazyagents/`.
    #[arg(long)]
    pub purge_files: bool,
    /// Modify `.lazyagents/local.toml` instead of `.lazyagents/lazyagents.toml`.
    #[arg(long)]
    pub local: bool,
}

/// Arguments for `lazyagents enable` and `lazyagents disable`.
#[derive(Debug, Args)]
pub(crate) struct ToggleArgs {
    /// Kind of resource (or `target`) to toggle.
    #[arg(value_enum)]
    pub kind: ResourceKind,
    /// Resource identifier or target name (`antigravity`, `claude_code`, `codex`, `cursor`, `github_copilot`).
    pub name: String,
    /// Modify `.lazyagents/local.toml` instead of `.lazyagents/lazyagents.toml`.
    #[arg(long)]
    pub local: bool,
}

/// Subcommands under `lazyagents hook`.
#[derive(Debug, Subcommand)]
pub(crate) enum HookCommand {
    /// Evaluate a built-in guardrail hook against JSON on stdin and emit JSON on stdout.
    Eval {
        /// Guard ID (`git-guard`, `protected-paths`, `secret-guard`, `verify-gate`).
        guard: String,
        /// Hook wire protocol (`antigravity`).
        #[arg(long, value_enum, default_value_t = CliHookProtocol::Antigravity)]
        protocol: CliHookProtocol,
    },
    /// Dry-run test a built-in guardrail hook with a JSON fixture or synthetic arguments.
    Test {
        /// Guard ID (`git-guard`, `protected-paths`, `secret-guard`, `verify-gate`).
        guard: String,
        /// Optional JSON fixture file path (`-` reads stdin).
        #[arg(long)]
        input: Option<String>,
        /// Synthetic shell command line (for `git-guard`).
        #[arg(long)]
        command: Option<String>,
        /// Synthetic target file path (for `protected-paths`).
        #[arg(long)]
        path: Option<String>,
        /// Synthetic outgoing file content (for `secret-guard`).
        #[arg(long)]
        content: Option<String>,
    },
}

/// Subcommands under `lazyagents github`.
#[derive(Debug, Subcommand)]
pub(crate) enum GithubCommand {
    /// Read-only live GitHub Rulesets, merge strategy, and Immutable Releases audit via `gh`.
    Audit {
        /// Explicit repository slug (`owner/name`), overriding manifest and Git remote probing.
        #[arg(long)]
        repo: Option<String>,
        /// Output format (`text` or `json`).
        #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
        format: OutputFormat,
    },
}
