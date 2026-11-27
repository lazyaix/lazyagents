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
    assert!(inspected["budget"]["push_pull"]["pull_ratio_percent"].is_u64());
    assert!(inspected["lethal_trifecta"]["overall_risk"].is_string());

    let agbom_json = run_cli(root, &["inspect", "--agbom", "--format", "json"]).unwrap();
    assert!(agbom_json.status.success());
    let agbom: serde_json::Value = serde_json::from_slice(&agbom_json.stdout).unwrap();
    assert_eq!(agbom["schema_version"], 1);
    assert_eq!(agbom["project"], "sample-app");
    assert!(
        agbom["components"]
            .as_array()
            .is_some_and(|items| items.iter().any(|c| c["id"] == "db-migrate"))
    );

    let agbom_text = run_cli(root, &["inspect", "--agbom"]).unwrap();
    assert!(agbom_text.status.success());
    assert!(String::from_utf8_lossy(&agbom_text.stdout).contains("AgBOM v1"));

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

#[test]
fn hook_eval_and_check_discover_workspace_root_from_agents_and_nested_subdirs() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"hook-cwd-demo\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .unwrap();
    assert!(run_cli(root, &["init"]).unwrap().status.success());

    // Configure a custom protected path policy inside `.lazyagents/lazyagents.toml`.
    let manifest_path = root.join(".lazyagents/lazyagents.toml");
    let raw = fs::read_to_string(&manifest_path).unwrap();
    let updated = raw.replacen(
        "codeowners_sync = false\nentries = []",
        "codeowners_sync = false\nentries = [{ glob = \"secret-docs/**\", owners = [\"@sec-team\"], decision = \"deny\" }]",
        1,
    );
    assert_ne!(raw, updated, "protected_paths block should be updated");
    fs::write(&manifest_path, updated).unwrap();
    let gen_out = run_cli(root, &["generate"]).unwrap();
    assert!(
        gen_out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&gen_out.stderr)
    );

    let agents_dir = root.join(".agents");
    assert!(agents_dir.is_dir());

    // 1. Simulate Antigravity running `.agents/hooks.json` with CWD = `<repo>/.agents/`
    //    and passing repo root via stdin `cwd` (no `-C` flag).
    let payload_with_cwd = serde_json::json!({
        "cwd": root.to_string_lossy(),
        "toolCall": {
            "name": "edit_file",
            "args": {
                "Path": root.join("secret-docs/keys.md").to_string_lossy()
            }
        }
    })
    .to_string();
    let mut child = Command::new(env!("CARGO_BIN_EXE_lazyagents"))
        .current_dir(&agents_dir)
        .args(["hook", "eval", "protected-paths"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(payload_with_cwd.as_bytes())
        .unwrap();
    let out1 = child.wait_with_output().unwrap();
    assert!(out1.status.success());
    let stdout1 = String::from_utf8_lossy(&out1.stdout);
    assert!(
        stdout1.contains("\"decision\":\"deny\""),
        "stdout: {stdout1}"
    );
    assert!(stdout1.contains("@sec-team"), "stdout: {stdout1}");

    // 2. Even without stdin `cwd`, ancestor discovery from `<repo>/.agents/` loads project policy.
    let payload_no_cwd =
        r#"{"toolCall":{"name":"write_file","args":{"Path":"secret-docs/private.md"}}}"#;
    let mut child2 = Command::new(env!("CARGO_BIN_EXE_lazyagents"))
        .current_dir(&agents_dir)
        .args(["hook", "eval", "protected-paths"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child2
        .stdin
        .take()
        .unwrap()
        .write_all(payload_no_cwd.as_bytes())
        .unwrap();
    let out2 = child2.wait_with_output().unwrap();
    assert!(out2.status.success());
    let stdout2 = String::from_utf8_lossy(&out2.stdout);
    assert!(
        stdout2.contains("\"decision\":\"deny\""),
        "stdout: {stdout2}"
    );

    // 3. Running `lazyagents check` from a nested subdir without `-C` discovers workspace root.
    let nested = root.join("crates/app/src");
    fs::create_dir_all(&nested).unwrap();
    let check_from_subdir = Command::new(env!("CARGO_BIN_EXE_lazyagents"))
        .current_dir(&nested)
        .args(["check", "--format", "json"])
        .output()
        .unwrap();
    assert!(
        check_from_subdir.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&check_from_subdir.stderr)
    );
}

#[test]
fn tui_cli_flags_and_studio_prefs_resolve_deterministically_in_no_tui_mode() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    let cfg_home = root.join("xdg-config");
    fs::create_dir_all(cfg_home.join("lazyagents")).unwrap();
    fs::write(
        cfg_home.join("lazyagents/studio.toml"),
        "theme = \"catppuccin-mocha\"\nshader_preset = \"crisp-minimal\"\nmotion = \"reduced\"\nlang = \"zh\"\n",
    )
    .unwrap();

    let run_with_clean_env = |args: &[&str], extra_env: Option<(&str, &str)>| {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_lazyagents"));
        cmd.arg("-C")
            .arg(root)
            .args(args)
            .env_remove("NO_COLOR")
            .env_remove("SSH_CONNECTION")
            .env_remove("SSH_TTY")
            .env("TERM", "xterm-256color")
            .env("COLORTERM", "truecolor")
            .env("XDG_CONFIG_HOME", &cfg_home)
            .env("APPDATA", &cfg_home);
        if let Some((k, v)) = extra_env {
            cmd.env(k, v);
        }
        cmd.output()
    };

    // 1. studio.toml preferences apply when CLI flags are omitted.
    let from_prefs = run_with_clean_env(&["--no-tui", "--inline"], None).unwrap();
    assert!(
        from_prefs.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&from_prefs.stderr)
    );
    let prefs_text = String::from_utf8_lossy(&from_prefs.stdout);
    assert!(
        prefs_text.contains("theme=catppuccin-mocha"),
        "stdout: {prefs_text}"
    );
    assert!(
        prefs_text.contains("shader=crisp-minimal"),
        "stdout: {prefs_text}"
    );
    assert!(
        prefs_text.contains("Inline { height: 18 }"),
        "stdout: {prefs_text}"
    );
    assert!(
        prefs_text.contains("motion=reduced"),
        "stdout: {prefs_text}"
    );
    assert!(
        prefs_text.contains("L4 赛博朋克配置工作室"),
        "stdout: {prefs_text}"
    );

    // 2. Explicit CLI flags override studio.toml.
    let overridden = run_with_clean_env(
        &[
            "--no-tui",
            "--theme",
            "tokyo-night",
            "--shader",
            "aurora-sweep",
            "--motion",
            "full",
            "--glyphs",
            "unicode",
            "--inline=22",
            "--no-splash",
            "--no-mouse",
            "--lang",
            "en",
        ],
        None,
    )
    .unwrap();
    assert!(
        overridden.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&overridden.stderr)
    );
    let en_text = String::from_utf8_lossy(&overridden.stdout);
    assert!(en_text.contains("theme=tokyo-night"), "stdout: {en_text}");
    assert!(en_text.contains("shader=aurora-sweep"), "stdout: {en_text}");
    assert!(
        en_text.contains("Inline { height: 22 }"),
        "stdout: {en_text}"
    );
    assert!(en_text.contains("motion=full"), "stdout: {en_text}");
    assert!(en_text.contains("Headless mode"), "stdout: {en_text}");

    // 3. TERM=dumb degrades motion cleanly to `none`, and SSH degrades `full` to `reduced`.
    let dumb_term =
        run_with_clean_env(&["--no-tui", "--motion", "full"], Some(("TERM", "dumb"))).unwrap();
    assert!(dumb_term.status.success());
    let dumb_text = String::from_utf8_lossy(&dumb_term.stdout);
    assert!(dumb_text.contains("motion=none"), "stdout: {dumb_text}");

    let ssh_term = run_with_clean_env(
        &["--no-tui", "--motion", "full"],
        Some(("SSH_CONNECTION", "10.0.0.1 22 10.0.0.2 54321")),
    )
    .unwrap();
    assert!(ssh_term.status.success());
    let ssh_text = String::from_utf8_lossy(&ssh_term.stdout);
    assert!(ssh_text.contains("motion=reduced"), "stdout: {ssh_text}");
}
