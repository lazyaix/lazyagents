//! Integration tests for shell-free, timeout-enforced `auto-format` execution (`la_runtime::hook_exec`).

use core::assert_matches;
use std::fs;

use la_core::FormatCommandPlan;
use la_runtime::{RuntimeError, execute_format_plan};
use tempfile::TempDir;

#[test]
fn execute_format_plan_formats_rust_file_silently_without_shell() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    let src_dir = root.join("src");
    fs::create_dir_all(&src_dir).unwrap();
    let file_path = src_dir.join("lib.rs");
    fs::write(&file_path, "pub fn add(a:i32,b:i32)->i32{a+b}\n").unwrap();

    let plan = FormatCommandPlan {
        rule_id: "rustfmt".to_owned(),
        program: "rustfmt".to_owned(),
        args: vec![
            "--edition".to_owned(),
            "2024".to_owned(),
            "src/lib.rs".to_owned(),
        ],
        target_path: "src/lib.rs".to_owned(),
        timeout_ms: 5_000,
    };

    let outcome = execute_format_plan(root, &plan).unwrap();
    assert!(outcome.succeeded, "diagnostic: {:?}", outcome.diagnostic);
    assert!(!outcome.timed_out);
    assert!(outcome.diagnostic.is_none());

    let formatted = fs::read_to_string(&file_path).unwrap();
    assert!(
        formatted.contains("pub fn add(a: i32, b: i32) -> i32"),
        "formatted: {formatted}"
    );
}

#[test]
fn execute_format_plan_rejects_path_traversal_and_handles_missing_files_and_binaries() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    // 1. Path traversal outside workspace root is rejected with RuntimeError::HookExec.
    let escape_plan = FormatCommandPlan {
        rule_id: "escape".to_owned(),
        program: "rustfmt".to_owned(),
        args: vec!["../outside.rs".to_owned()],
        target_path: "../outside.rs".to_owned(),
        timeout_ms: 1_000,
    };
    assert_matches!(
        execute_format_plan(root, &escape_plan),
        Err(RuntimeError::HookExec { .. })
    );

    // 2. Non-existent target file is treated as a clean no-op.
    let missing_plan = FormatCommandPlan {
        rule_id: "missing".to_owned(),
        program: "rustfmt".to_owned(),
        args: vec!["src/deleted.rs".to_owned()],
        target_path: "src/deleted.rs".to_owned(),
        timeout_ms: 1_000,
    };
    let missing_out = execute_format_plan(root, &missing_plan).unwrap();
    assert!(missing_out.succeeded);
    assert!(missing_out.diagnostic.is_none());

    // 3. Missing formatter binary returns a non-fatal diagnostic instead of crashing.
    let target = root.join("sample.txt");
    fs::write(&target, "hello\n").unwrap();
    let bad_bin_plan = FormatCommandPlan {
        rule_id: "missing-bin".to_owned(),
        program: "lazyagents-nonexistent-formatter-xyz".to_owned(),
        args: vec!["sample.txt".to_owned()],
        target_path: "sample.txt".to_owned(),
        timeout_ms: 1_000,
    };
    let bad_bin_out = execute_format_plan(root, &bad_bin_plan).unwrap();
    assert!(!bad_bin_out.succeeded);
    assert!(!bad_bin_out.timed_out);
    assert!(
        bad_bin_out
            .diagnostic
            .as_deref()
            .is_some_and(|d| d.contains("failed to launch"))
    );
}

#[test]
fn execute_format_plan_enforces_strict_timeout_and_kills_slow_process() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    fs::write(root.join("slow.txt"), "data\n").unwrap();

    let (program, args): (&str, Vec<String>) = cfg_select! {
        unix => ("sleep", vec!["2".to_owned()]),
        windows => ("cmd", vec!["/C".to_owned(), "ping -n 4 127.0.0.1 >NUL".to_owned()]),
    };

    let plan = FormatCommandPlan {
        rule_id: "slow-fmt".to_owned(),
        program: program.to_owned(),
        args,
        target_path: "slow.txt".to_owned(),
        timeout_ms: 80,
    };

    let outcome = execute_format_plan(root, &plan).unwrap();
    assert!(!outcome.succeeded);
    assert!(outcome.timed_out);
    assert!(
        outcome
            .diagnostic
            .as_deref()
            .is_some_and(|d| d.contains("timed out"))
    );
}
