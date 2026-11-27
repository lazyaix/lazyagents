//! Deterministic `minijinja`-based template rendering and conditional `when`
//! predicate evaluation for dynamic `AGENTS.md` sections and scaffolds.

use minijinja::{Environment, Value, value::Serde};
use serde::Serialize;

use crate::engineering::EngineeringPolicy;
use crate::error::CoreError;
use crate::probe::ProjectFacts;

/// Context payload passed to all deterministic template renders.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct TemplateContext<'a> {
    /// Probed repository facts (`{{ project.* }}`).
    pub project: &'a ProjectFacts,
    /// Declarative engineering governance policy (`{{ engineering.* }}`).
    pub engineering: &'a EngineeringPolicy,
}

/// Pure Sans-I/O deterministic template renderer with LF line-ending normalization.
#[derive(Clone, Copy, Debug, Default)]
pub struct TemplateEngine;

impl TemplateEngine {
    /// Renders a Jinja-compatible template string against [`TemplateContext`].
    ///
    /// Line endings are normalized to Unix LF (`\n`) so output is byte-identical across
    /// macOS, Linux, and Windows.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::TemplateRender`] if syntax parsing or evaluation fails.
    pub fn render(
        template_name: &str,
        source: &str,
        ctx: &TemplateContext<'_>,
    ) -> Result<String, CoreError> {
        let mut env = Environment::new();
        env.add_template(template_name, source)
            .map_err(|err| CoreError::TemplateRender {
                template: template_name.to_owned(),
                message: err.to_string(),
            })?;
        let tpl = env
            .get_template(template_name)
            .map_err(|err| CoreError::TemplateRender {
                template: template_name.to_owned(),
                message: err.to_string(),
            })?;
        let rendered =
            tpl.render(Value::from(Serde(ctx)))
                .map_err(|err| CoreError::TemplateRender {
                    template: template_name.to_owned(),
                    message: err.to_string(),
                })?;
        let mut normalized = normalize_lf(&rendered);
        if (source.ends_with('\n') || source.ends_with('\r')) && !normalized.ends_with('\n') {
            normalized.push('\n');
        }
        Ok(normalized)
    }

    /// Evaluates a boolean `when` predicate expression (e.g., `"project.has_rust"` or
    /// `"engineering.git.stacked_prs and project.has_github_actions"`).
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::TemplateRender`] if the expression cannot be compiled or evaluated.
    pub fn eval_condition(expr: &str, ctx: &TemplateContext<'_>) -> Result<bool, CoreError> {
        let env = Environment::new();
        let compiled = env
            .compile_expression(expr)
            .map_err(|err| CoreError::TemplateRender {
                template: format!("when({expr})"),
                message: err.to_string(),
            })?;
        let value: Value =
            compiled
                .eval(Value::from(Serde(ctx)))
                .map_err(|err| CoreError::TemplateRender {
                    template: format!("when({expr})"),
                    message: err.to_string(),
                })?;
        Ok(value.is_true())
    }
}

/// Normalizes CRLF (`\r\n`) and stray CR (`\r`) line endings to Unix LF (`\n`).
#[must_use]
pub fn normalize_lf(input: &str) -> String {
    input.replace("\r\n", "\n").replace('\r', "\n")
}

#[cfg(test)]
mod tests {
    use core::assert_matches;

    use crate::probe::DetectedLanguage;

    use super::*;

    #[test]
    fn renders_project_and_engineering_variables_with_lf_normalization() {
        let mut facts = ProjectFacts {
            name: "lazyagents".to_owned(),
            languages: vec![DetectedLanguage::Rust],
            toolchain_version: Some("1.100.0".to_owned()),
            ..ProjectFacts::default()
        };
        facts.refresh_flags();
        let eng = EngineeringPolicy::default();
        let ctx = TemplateContext {
            project: &facts,
            engineering: &eng,
        };

        let out = TemplateEngine::render(
            "test",
            "# {{ project.name }}\r\nBranch: {{ engineering.git.default_branch }}\r\n",
            &ctx,
        )
        .unwrap();
        assert_eq!(out, "# lazyagents\nBranch: main\n");
    }

    #[test]
    fn evaluates_boolean_when_predicates_and_reports_syntax_errors() {
        let mut facts = ProjectFacts {
            name: "lazyagents".to_owned(),
            languages: vec![DetectedLanguage::Rust],
            ..ProjectFacts::default()
        };
        facts.refresh_flags();
        let eng = EngineeringPolicy::default();
        let ctx = TemplateContext {
            project: &facts,
            engineering: &eng,
        };

        assert!(TemplateEngine::eval_condition("project.has_rust", &ctx).unwrap());
        assert!(!TemplateEngine::eval_condition("project.has_python", &ctx).unwrap());
        assert_matches!(
            TemplateEngine::eval_condition("project.has_rust and (", &ctx),
            Err(CoreError::TemplateRender { .. })
        );
    }
}
