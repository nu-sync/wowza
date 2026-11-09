mod common;
use common::{Installation, archive, snapshot, success};
use std::{fs, process::Command};
use wowza::manage::{self, execute, prepare_install, prepare_remove, prepare_update};

fn redirected_installation(preview_first: bool) {
    for component in ["Interface", "Interface/AddOns"] {
        let selected = Installation::new();
        let other = Installation::new();
        let zip = selected.zip("addon.zip", &[("A/A.toc", "addon")]);
        let plan = preview_first.then(|| prepare_install(selected.root(), &zip).unwrap());
        fs::remove_dir(selected.addons()).unwrap();
        if component == "Interface" {
            fs::remove_dir(selected.root().join("Interface")).unwrap();
        }
        common::link(
            &selected.root().join(component),
            &other.root().join(component),
        );
        let before = snapshot(other.root());
        let outcome = match plan {
            Some(plan) => execute(plan),
            None => prepare_install(selected.root(), &zip).and_then(execute),
        };
        assert!(
            outcome.is_err(),
            "{component} redirected a change into another installation"
        );
        assert_eq!(snapshot(other.root()), before);
        common::unlink(&selected.root().join(component));
    }
}

#[test]
fn refuses_redirected_installation_before_preview() {
    redirected_installation(false);
}

#[test]
fn refuses_redirected_installation_after_preview() {
    redirected_installation(true);
}

#[test]
fn inspection_groups_only_consistent_packages_and_reports_file_problems() {
    let game = Installation::new();
    let zip = game.zip(
        "maps.zip",
        &[
            ("Maps/Maps.toc", "## Title: Maps\n## Version: 1"),
            ("Maps_Config/Maps_Config.toc", "## Title: Maps settings"),
        ],
    );
    game.install(&zip);
    let before = snapshot(game.root());
    let report = manage::inspect_installation(game.root()).unwrap();
    assert_eq!(report.folder_count, 2);
    assert_eq!(report.packages.len(), 1);
    assert_eq!(report.packages[0].folders, ["Maps", "Maps_Config"]);
    assert_eq!(report.packages[0].status, manage::Status::Managed);
    for args in [
        &["--json"][..],
        &["status", "--json"],
        &["list", "--json"],
        &["scan", "--json"],
        &["doctor", "--json"],
    ] {
        let value: serde_json::Value = serde_json::from_str(&success(game.run(args))).unwrap();
        assert_eq!(value["packages"].as_array().unwrap().len(), 1);
    }
    assert_eq!(snapshot(game.root()), before);
    fs::write(game.addons().join("Maps_Config/Maps_Config.toc"), "changed").unwrap();
    let report = manage::inspect_installation(game.root()).unwrap();
    assert_eq!(report.packages[0].status, manage::Status::Modified);
    assert!(
        report.packages[0]
            .issues
            .iter()
            .any(|s| s.contains("contents changed"))
    );
    fs::remove_dir_all(game.addons().join("Maps_Config")).unwrap();
    let report = manage::inspect_installation(game.root()).unwrap();
    assert_eq!(report.packages[0].status, manage::Status::Incomplete);
    assert!(
        report.packages[0]
            .issues
            .iter()
            .any(|s| s.contains("Missing folder: Maps_Config"))
    );
    fs::create_dir(game.addons().join("Maps_Config")).unwrap();
    fs::write(
        game.addons().join("Maps_Config/Maps_Config.toc"),
        "unmanaged",
    )
    .unwrap();
    let report = manage::inspect_installation(game.root()).unwrap();
    assert_eq!(report.packages.len(), 2);
    assert_eq!(report.packages[0].folders, ["Maps"]);
    assert_eq!(report.packages[1].status, manage::Status::Unmanaged);
}

#[test]
fn previews_cancellation_and_json_never_change_the_installation() {
    let game = Installation::new();
    let zip = game.zip("a.zip", &[("A/A.toc", "old")]);
    let before = snapshot(game.root());
    let preview: serde_json::Value = serde_json::from_str(&success(
        game.command()
            .args(["install", "--dry-run", "--json"])
            .arg(&zip)
            .output()
            .unwrap(),
    ))
    .unwrap();
    assert_eq!(preview["folders"], serde_json::json!(["A"]));
    assert_eq!(preview["action"], "install");
    assert_eq!(snapshot(game.root()), before);
    let output = game.input(&["install", zip.to_str().unwrap()], b"n\n");
    assert!(success(output).contains("Cancelled."));
    assert_eq!(snapshot(game.root()), before);
    assert!(
        !game
            .run(&["install", zip.to_str().unwrap(), "--json"])
            .status
            .success()
    );
    assert_eq!(snapshot(game.root()), before);
    game.install(&zip);
    let new = game.zip("new.zip", &[("A/A.toc", "new")]);
    let before = snapshot(game.root());
    for args in [
        vec!["update", "A", new.to_str().unwrap(), "--dry-run", "--json"],
        vec!["remove", "A", "--dry-run", "--json"],
    ] {
        serde_json::from_str::<serde_json::Value>(&success(game.run(&args))).unwrap();
    }
    assert_eq!(snapshot(game.root()), before);
    let applied: serde_json::Value = serde_json::from_str(&success(game.run(&[
        "update",
        "A",
        new.to_str().unwrap(),
        "--json",
        "--yes",
    ])))
    .unwrap();
    assert_eq!(applied["action"], "update");
    assert_eq!(
        fs::read_to_string(game.addons().join("A/A.toc")).unwrap(),
        "new"
    );
    game.preserved();
}

#[test]
fn prepared_changes_reject_stale_archives_packages_and_new_conflicts() {
    let game = Installation::new();
    let zip = game.zip("a.zip", &[("A/A.toc", "old")]);
    let plan = prepare_install(game.root(), &zip).unwrap();
    archive(&zip, &[("A/A.toc", "changed")]);
    assert!(execute(plan).unwrap_err().contains("changed after preview"));
    assert!(!game.addons().join("A").exists());
    let plan = prepare_install(game.root(), &zip).unwrap();
    fs::create_dir(game.addons().join("A")).unwrap();
    fs::write(game.addons().join("A/keep"), "unrelated").unwrap();
    assert!(execute(plan).is_err());
    assert_eq!(
        fs::read_to_string(game.addons().join("A/keep")).unwrap(),
        "unrelated"
    );
    fs::remove_dir_all(game.addons().join("A")).unwrap();
    game.install(&zip);
    let remove = prepare_remove(game.root(), "A").unwrap();
    let next = game.zip("next.zip", &[("A/A.toc", "next")]);
    execute(prepare_update(game.root(), "A", &next).unwrap()).unwrap();
    assert!(
        execute(remove)
            .unwrap_err()
            .contains("changed after preview")
    );
    let update = prepare_update(game.root(), "A", &zip).unwrap();
    fs::write(game.addons().join("A/A.toc"), "player edit").unwrap();
    assert!(execute(update).is_err());
    assert_eq!(
        fs::read_to_string(game.addons().join("A/A.toc")).unwrap(),
        "player edit"
    );
    game.preserved();
}

#[test]
fn configuration_precedence_and_relocation_are_explicit() {
    let first = Installation::new();
    let second = Installation::new();
    let settings = first.root().join("config/settings.json");
    let call = |args: &[&str], environment: Option<&std::path::Path>| {
        let mut command = Command::new(env!("CARGO_BIN_EXE_wowza"));
        command
            .env("WOWZA_CONFIG_FILE", &settings)
            .env_remove("WOWZA_FOREVER_PATH")
            .env_remove("WOWZA_TEST_STOP_AFTER")
            .args(args);
        if let Some(path) = environment {
            command.env("WOWZA_FOREVER_PATH", path);
        }
        command.output().unwrap()
    };
    success(call(
        &["configure", first.root().to_str().unwrap(), "--dry-run"],
        None,
    ));
    assert!(!settings.exists());
    success(call(&["configure", first.root().to_str().unwrap()], None));
    let reported = |args: &[&str], env| {
        let value: serde_json::Value = serde_json::from_str(&success(call(args, env))).unwrap();
        std::path::PathBuf::from(value["installation"].as_str().unwrap())
    };
    assert_eq!(
        reported(&["--json"], None),
        fs::canonicalize(first.root()).unwrap()
    );
    assert_eq!(
        reported(&["status", "--json"], Some(second.root())),
        fs::canonicalize(second.root()).unwrap()
    );
    assert_eq!(
        reported(
            &["--path", first.root().to_str().unwrap(), "--json"],
            Some(second.root())
        ),
        fs::canonicalize(first.root()).unwrap()
    );
    success(call(&["configure", second.root().to_str().unwrap()], None));
    assert_eq!(
        reported(&["--json"], None),
        fs::canonicalize(second.root()).unwrap()
    );
    fs::write(&settings, "broken").unwrap();
    assert!(!call(&["status"], None).status.success());
    assert_eq!(
        reported(&["--path", first.root().to_str().unwrap(), "--json"], None),
        fs::canonicalize(first.root()).unwrap()
    );
    success(call(&["configure", first.root().to_str().unwrap()], None));
    fs::remove_dir(first.addons()).unwrap();
    let error = call(&["status"], None);
    assert!(!error.status.success());
    assert!(String::from_utf8_lossy(&error.stderr).contains("configure"));
}
