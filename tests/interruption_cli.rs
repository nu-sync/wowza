#![cfg(all(windows, feature = "test-hooks"))]
mod common;
use common::{Installation, snapshot, success};
use std::{fs, process::Command};
use wowza::manage;

fn stop(mut command: Command, checkpoint: usize) {
    let output = command
        .env("WOWZA_TEST_STOP_AFTER", checkpoint.to_string())
        .output()
        .unwrap();
    assert_eq!(
        output.status.code(),
        Some(86),
        "checkpoint {checkpoint}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn recovery_dry_run_reports_changed_files_without_writing() {
    let game = Installation::new();
    let old = game.zip("old.zip", &[("A/A.toc", "old a"), ("B/B.toc", "old b")]);
    let new = game.zip("new.zip", &[("A/A.toc", "new a"), ("B/B.toc", "new b")]);
    game.install(&old);
    let mut update = game.command();
    update.args(["update", "A"]).arg(&new).arg("--yes");
    stop(update, 5);
    let pending = manage::pending(game.root()).unwrap();
    fs::write(
        game.root()
            .join(".wowza")
            .join(&pending[0])
            .join("old/B/B.toc"),
        "changed",
    )
    .unwrap();
    let before = snapshot(game.root());
    let output = game.run(&["recover", "--dry-run", "--json"]);
    assert!(
        !output.status.success(),
        "Recovery preview must report that the original files changed"
    );
    assert!(String::from_utf8_lossy(&output.stderr).contains("Folder B was modified"));
    assert_eq!(snapshot(game.root()), before);
}

#[test]
fn recovery_rechecks_contents_after_preview() {
    let game = Installation::new();
    let old = game.zip("old.zip", &[("A/A.toc", "old a"), ("B/B.toc", "old b")]);
    let new = game.zip("new.zip", &[("A/A.toc", "new a"), ("B/B.toc", "new b")]);
    game.install(&old);
    let mut update = game.command();
    update.args(["update", "A"]).arg(&new).arg("--yes");
    stop(update, 5);
    let pending = manage::pending(game.root()).unwrap();
    let plan = manage::prepare_recovery(game.root(), None).unwrap();
    fs::write(
        game.root()
            .join(".wowza")
            .join(&pending[0])
            .join("old/B/B.toc"),
        "changed",
    )
    .unwrap();
    let before = snapshot(game.root());
    assert!(
        manage::execute(plan)
            .unwrap_err()
            .contains("Folder B was modified")
    );
    assert_eq!(snapshot(game.root()), before);
}

#[test]
fn real_operations_recover_after_every_journal_and_move_boundary() {
    for (operation, boundaries) in [("install", 4), ("update", 6), ("remove", 4)] {
        for step in 1..=boundaries {
            let game = Installation::new();
            let old = game.zip("old.zip", &[("A/A.toc", "old a"), ("B/B.toc", "old b")]);
            let new = game.zip("new.zip", &[("A/A.toc", "new a"), ("B/B.toc", "new b")]);
            fs::create_dir(game.addons().join("Unrelated")).unwrap();
            fs::write(game.addons().join("Unrelated/keep"), "do not touch").unwrap();
            if operation != "install" {
                game.install(&old);
            }
            let before = snapshot(&game.addons());
            let mut command = game.command();
            match operation {
                "install" => {
                    command.arg("install").arg(&new);
                }
                "update" => {
                    command.args(["update", "A"]).arg(&new);
                }
                _ => {
                    command.args(["remove", "A"]);
                }
            }
            command.arg("--yes");
            stop(command, step);
            let pending = manage::pending(game.root()).unwrap();
            if step < boundaries {
                assert_eq!(pending.len(), 1, "{operation} checkpoint {step}");
                let preview = snapshot(game.root());
                let output = success(game.run(&["recover", "--dry-run", "--json"]));
                let value: serde_json::Value = serde_json::from_str(&output).unwrap();
                assert_eq!(value["folders"], serde_json::json!(["A", "B"]));
                assert_eq!(snapshot(game.root()), preview);
                assert!(success(game.input(&["recover"], b"n\n")).contains("Cancelled."));
                assert_eq!(snapshot(game.root()), preview);
                success(game.run(&["recover", "--yes"]));
                assert_eq!(
                    snapshot(&game.addons()),
                    before,
                    "{operation} checkpoint {step}"
                );
            } else {
                assert!(pending.is_empty());
                if operation == "remove" {
                    assert!(!game.addons().join("A").exists());
                } else {
                    assert_eq!(
                        fs::read_to_string(game.addons().join("A/A.toc")).unwrap(),
                        "new a"
                    );
                }
            }
            assert_eq!(
                fs::read_to_string(game.addons().join("Unrelated/keep")).unwrap(),
                "do not touch"
            );
            assert!(manage::pending(game.root()).unwrap().is_empty());
            game.preserved();
        }
    }
}

#[test]
fn recovery_itself_can_be_interrupted_and_retried() {
    for step in 1..=6 {
        let game = Installation::new();
        let old = game.zip("old.zip", &[("A/A.toc", "old a"), ("B/B.toc", "old b")]);
        let new = game.zip("new.zip", &[("A/A.toc", "new a"), ("B/B.toc", "new b")]);
        game.install(&old);
        let before = snapshot(&game.addons());
        let mut update = game.command();
        update.args(["update", "A"]).arg(&new).arg("--yes");
        stop(update, 5);
        let mut recover = game.command();
        recover.args(["recover", "--yes"]);
        stop(recover, step);
        if !manage::pending(game.root()).unwrap().is_empty() {
            success(game.run(&["recover", "--yes"]));
        }
        assert_eq!(snapshot(&game.addons()), before);
        game.preserved();
    }
}

#[test]
fn recovery_preserves_path_handles_ambiguity_and_rejects_stale_journals() {
    let first = Installation::new();
    let second = Installation::new();
    let zip = first.zip("a.zip", &[("A/A.toc", "a")]);
    let mut command = first.command();
    command.arg("install").arg(&zip).arg("--yes");
    stop(command, 1);
    let operations = manage::pending_operations(first.root()).unwrap();
    assert_eq!(operations[0].folders, ["A"]);
    assert_eq!(operations[0].action, "install");
    let suggestion = &operations[0].recovery_command;
    assert!(
        suggestion.contains("''"),
        "PowerShell apostrophe must be escaped"
    );
    assert!(!second.run(&["recover", "--yes"]).status.success());
    let plan = manage::prepare_recovery(first.root(), None).unwrap();
    let journal = first
        .root()
        .join(".wowza")
        .join(&operations[0].id)
        .join("pending.json");
    let original = fs::read(&journal).unwrap();
    let mut changed = original.clone();
    changed.push(b' ');
    fs::write(&journal, changed).unwrap();
    assert!(
        manage::execute(plan)
            .unwrap_err()
            .contains("changed after preview")
    );
    fs::write(&journal, &original).unwrap();
    let other = first.root().join(".wowza/txn-other");
    fs::create_dir(&other).unwrap();
    fs::write(other.join("pending.json"), &original).unwrap();
    let before = snapshot(first.root());
    let output = first.run(&["recover", "--yes"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("Multiple"));
    assert_eq!(snapshot(first.root()), before);
    fs::remove_dir_all(other).unwrap();
    let executable = env!("CARGO_BIN_EXE_wowza").replace('\'', "''");
    let script = format!(
        "function wowza {{ & '{executable}' @args }}\n{suggestion} --yes\nexit $LASTEXITCODE"
    );
    let output = Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .env("WOWZA_FOREVER_PATH", second.root())
        .env("WOWZA_CONFIG_FILE", second.root().join("config.json"))
        .output()
        .unwrap();
    success(output);
    assert!(manage::pending(first.root()).unwrap().is_empty());
    assert!(!first.addons().join("A").exists());
    assert!(snapshot(&second.addons()).is_empty());
    first.preserved();
    second.preserved();
}
