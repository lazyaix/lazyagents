//! Pure Sans-I/O canonical configuration IR, target compilers, compliance/budget linters,
//! hook policy evaluator, and secret-redaction primitives for `lazyagents`.
//!
//! This crate intentionally performs zero network, filesystem, PTY, or terminal I/O so
//! both `la-runtime` and `la-tui` can share deterministic domain logic.

mod agent;
pub mod budget;
pub mod catalog;
pub mod compiler;
pub mod diff;
pub mod engineering;
mod error;
pub mod hook_eval;
mod id;
pub mod lint;
pub mod lock;
pub mod manifest;
pub mod probe;
pub mod scope;
mod secret;
mod session;
pub mod template;

pub use crate::agent::AgentId;
pub use crate::budget::{BudgetReport, BudgetStatus, FileBudget, analyze_budgets};
pub use crate::compiler::{
    CompatibilityDiagnostic, CompatibilitySeverity, CompileInputs, CompileOutput, GeneratedFile,
    compile_all,
};
pub use crate::diff::{FileAction, FilePlan, WritePlan, plan_writes};
pub use crate::engineering::EngineeringPolicy;
pub use crate::error::CoreError;
pub use crate::hook_eval::{
    HookEvalContext, HookGuardKind, HookProtocol, PreToolUseOutput, StopHookOutput,
    evaluate_hook_json,
};
pub use crate::id::{HookEvent, KebabName, PolicyDecision, ResourceId, TargetKind, ToolCapability};
pub use crate::lint::{
    Diagnostic, DiagnosticLocation, DiagnosticSeverity, LintCode, LintInputs, LintReport,
    lint_project,
};
pub use crate::lock::{LockEntry, Lockfile, ManagedRegionStyle, OwnershipMode, sha256_hex};
pub use crate::manifest::{CURRENT_SCHEMA_VERSION, ProjectManifest};
pub use crate::probe::{CiWorkflowFact, CommandHints, DetectedLanguage, ProjectFacts};
pub use crate::scope::{ConfigScope, MergePolicy, ResolvedConfig, ScopeResolver, ScopedLayer};
pub use crate::secret::{
    RedactedSecret, SecretFinding, SecretKind, redact_ghs_tokens, scan_secrets,
};
pub use crate::session::SessionStatus;
pub use crate::template::{TemplateContext, TemplateEngine};
