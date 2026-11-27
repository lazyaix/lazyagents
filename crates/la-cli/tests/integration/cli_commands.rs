//! End-to-end subprocess tests exercising the compiled `lazyagents` binary across
//! `init`, `generate`, `diff`, `check`, `inspect`, `add/remove/enable/disable`, `import`,
//! and `hook eval` / `hook test`.

use std::fs;
use std::io::{self, Write as _};
use std::path::Path;
use std::process::{Command, Output, Stdio};

use tempfile::TempDir;

fn run_cli(root: &Path, args: &[&str]) -> io::Result<Output> {
    Command::new(env!("CARGO_BIN_EXE_lazyagents"))
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
}

fn run_cli_with_stdin(root: &Path, args: &[&str], stdin_data: &str) -> io::Result<Output> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_lazyagents"))
        .arg("-C")
        .arg(root)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(stdin_data.as_bytes())?;
    }
    child.wait_with_output()
}

#[test]
fn full_headless_ci_lifecycle_init_generate_check_diff_and_manage() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"sample-app\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .unwrap();

    // 1. init
    let init = run_cli(root, &["init"]).unwrap();
    assert!(
        init.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&init.stderr)
    );
    assert!(root.join(".lazyagents/lazyagents.toml").is_file());

    // 2. add / disable / enable resources via CLI
    let add_skill = run_cli(
        root,
        &[
            "add",
            "skill",
            "db-migrate",
            "--description",
            "Safe database migration runbook.",
        ],
    )
    .unwrap();
    assert!(add_skill.status.success());

    let add_mcp = run_cli(
        root,
        &[
            "add",
            "mcp",
            "docs-api",
            "--url",
            "https://mcp.example.com/mcp",
        ],
    )
    .unwrap();
    assert!(add_mcp.status.success());

    let disable_mcp = run_cli(root, &["disable", "mcp", "docs-api"]).unwrap();
    assert!(disable_mcp.status.success());

    let enable_mcp = run_cli(root, &["enable", "mcp", "docs-api"]).unwrap();
    assert!(enable_mcp.status.success());

    // 3. generate
    let generate = run_cli(root, &["generate"]).unwrap();
    assert!(
        generate.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&generate.stderr)
    );
    assert!(root.join("AGENTS.md").is_file());
    assert!(root.join(".agents/skills/db-migrate/SKILL.md").is_file());
    assert!(root.join(".agents/mcp_config.json").is_file());
    assert!(root.join(".agents/hooks.json").is_file());

    // 4. check (json) + diff --exit-code + inspect --format json
    let check_json = run_cli(root, &["check", "--format", "json"]).unwrap();
    assert!(check_json.status.success());
    let parsed: serde_json::Value = serde_json::from_slice(&check_json.stdout).unwrap();
    assert_eq!(parsed["diagnostics"].as_array().unwrap().len(), 0);

    let diff_clean = run_cli(root, &["diff", "--exit-code"]).unwrap();
    assert!(diff_clean.status.success());

    let inspect_json = run_cli(root, &["inspect", "--format", "json"]).unwrap();
    assert!(inspect_json.status.success());
    let inspected: serde_json::Value = serde_json::from_slice(&inspect_json.stdout).unwrap();
    assert_eq!(inspected["project"]["name"], "sample-app");
    assert_eq!(inspected["has_drift"], false);

    // 5. Introduce external drift -> check fails with exit code 1, diff --exit-code exits 1
    fs::write(root.join("AGENTS.md"), "# manual drift\n").unwrap();
    let check_drift = run_cli(root, &["check", "--format", "github"]).unwrap();
    assert_eq!(check_drift.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&check_drift.stdout).contains("title=LA013"));

    let diff_dirty = run_cli(root, &["diff", "--exit-code"]).unwrap();
    assert_eq!(diff_dirty.status.code(), Some(1));

    // 6. remove resource
    let rm = run_cli(root, &["remove", "skill", "db-migrate", "--purge-files"]).unwrap();
    assert!(rm.status.success());
    assert!(!root.join(".lazyagents/skills/db-migrate").exists());
}

#[test]
fn hook_eval_and_hook_test_enforce_guardrails_over_stdio() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    let eval_out = run_cli_with_stdin(
        root,
        &["hook", "eval", "git-guard"],
        r#"{"toolCall":{"args":{"CommandLine":"git push --force origin main"}}}"#,
    )
    .unwrap();
    assert!(eval_out.status.success());
    let stdout = String::from_utf8_lossy(&eval_out.stdout);
    assert!(stdout.contains("\"decision\":\"deny\""), "stdout: {stdout}");

    let test_out = run_cli(
        root,
        &[
            "hook",
            "test",
            "git-guard",
            "--command",
            "git reset --hard HEAD~1",
        ],
    )
    .unwrap();
    assert!(test_out.status.success());
    assert!(String::from_utf8_lossy(&test_out.stdout).contains("\"decision\":\"deny\""));

    let bad_cli = run_cli(root, &["--unknown-flag-xyz"]).unwrap();
    assert_eq!(bad_cli.status.code(), Some(2));
}
