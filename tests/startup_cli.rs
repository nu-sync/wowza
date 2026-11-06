mod common;

use common::{Installation, snapshot, success};
use std::{
    fs,
    path::Path,
    process::{Command, Stdio},
};

fn command(config: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_wowza"));
    command
        .env("WOWZA_CONFIG_FILE", config)
        .env_remove("WOWZA_FOREVER_PATH")
        .env_remove("WOWZA_TEST_STOP_AFTER")
        .stdin(Stdio::null());
    command
}

#[test]
fn no_arguments_show_guidance_without_resolving_configuration_or_installation() {
    let temp = tempfile::tempdir().unwrap();
    let config = temp.path().join("config.json");
    fs::write(&config, "invalid configuration").unwrap();
    let missing = temp.path().join("missing-installation");
    let before = snapshot(temp.path());

    for environment in [None, Some(&missing)] {
        let mut start = command(&config);
        let mut status = command(&config);
        if let Some(path) = environment {
            start.env("WOWZA_FOREVER_PATH", path);
            status.env("WOWZA_FOREVER_PATH", path);
        }
        let output = start.output().unwrap();
        assert!(output.stderr.is_empty());
        let text = success(output);
        assert!(text.contains("wowza status"));
        assert!(text.contains("wowza --help"));
        assert!(!text.contains("Installation:"));

        let output = status.arg("status").output().unwrap();
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        let error = String::from_utf8_lossy(&output.stderr);
        assert!(error.contains(if environment.is_some() {
            "AddOns folder not found"
        } else {
            "Cannot read configuration"
        }));
        assert_eq!(snapshot(temp.path()), before);
    }
}

#[test]
fn help_and_version_work_without_configuration_or_installation() {
    let temp = tempfile::tempdir().unwrap();
    let config = temp.path().join("config.json");
    fs::write(&config, "invalid configuration").unwrap();
    let before = snapshot(temp.path());

    for args in [
        &["--help"][..],
        &["-h"],
        &["help"],
        &["help", "status"],
        &["status", "--help"],
        &["--version"],
    ] {
        let output = command(&config)
            .env(
                "WOWZA_FOREVER_PATH",
                temp.path().join("missing-installation"),
            )
            .args(args)
            .output()
            .unwrap();
        assert!(output.stderr.is_empty(), "{args:?}");
        let text = success(output);
        if args == ["--version"] {
            assert_eq!(text, format!("wowza {}\n", env!("CARGO_PKG_VERSION")));
        } else {
            assert!(text.contains("Usage: wowza"), "{args:?}: {text}");
        }
        assert_eq!(snapshot(temp.path()), before);
    }
}

#[test]
fn status_and_option_only_invocations_still_inspect_without_writing() {
    let game = Installation::new();
    fs::create_dir(game.addons().join("Example")).unwrap();
    fs::write(
        game.addons().join("Example/Example.toc"),
        "## Title: Example Addon\n## Version: 1.2\n",
    )
    .unwrap();
    let config = game.root().join("config.json");
    fs::write(&config, "invalid configuration").unwrap();
    let before = snapshot(game.root());
    let path = game.root().to_str().unwrap();

    let output = command(&config)
        .env("WOWZA_FOREVER_PATH", game.root())
        .output()
        .unwrap();
    assert!(output.stderr.is_empty());
    let text = success(output);
    assert!(text.contains("wowza status"));
    assert!(!text.contains("Example"));
    assert!(!text.contains("Installation:"));
    assert_eq!(snapshot(game.root()), before);

    for args in [
        &["status"][..],
        &["--path", path],
        &["--dry-run"],
        &["--json"],
        &["--path", path, "--json"],
    ] {
        let output = command(&config)
            .env("WOWZA_FOREVER_PATH", game.root())
            .args(args)
            .output()
            .unwrap();
        assert!(output.stderr.is_empty(), "{args:?}");
        let text = success(output);
        if args.contains(&"--json") {
            let report: serde_json::Value = serde_json::from_str(&text).unwrap();
            assert_eq!(report["folder_count"], 1);
            assert_eq!(report["packages"].as_array().unwrap().len(), 1);
            assert_eq!(report["packages"][0]["folders"][0], "Example");
            assert_eq!(report["packages"][0]["status"], "unmanaged");
        } else {
            assert!(text.contains("AddOns: 1"), "{args:?}: {text}");
            assert!(text.contains("Example [unmanaged]"), "{args:?}: {text}");
            assert!(text.contains("Example Addon 1.2"), "{args:?}: {text}");
        }
        assert_eq!(snapshot(game.root()), before);
    }
}
