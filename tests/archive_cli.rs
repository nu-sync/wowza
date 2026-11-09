use std::{fs, io::Write};

mod common;
use common::archive;

fn run(root: &std::path::Path, zip: &std::path::Path) -> std::process::Output {
    common::command(root)
        .arg("inspect-archive")
        .arg(zip)
        .output()
        .unwrap()
}
#[test]
fn preview_and_conflict_do_not_modify_installation() {
    let temp = tempfile::tempdir().unwrap();
    let addons = temp.path().join("Interface/AddOns");
    fs::create_dir_all(&addons).unwrap();
    let zip = temp.path().join("addon.zip");
    archive(
        &zip,
        &[
            ("Example/Example.toc", "## Title: Example"),
            ("Example/main.lua", "hello"),
        ],
    );
    let out = run(temp.path(), &zip);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(String::from_utf8_lossy(&out.stdout).contains("Example: 2 files [no existing folder]"));
    assert!(!addons.join("Example").exists());
    fs::create_dir(addons.join("example")).unwrap();
    fs::write(addons.join("example/user.txt"), "keep").unwrap();
    let out = run(temp.path(), &zip);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).contains("CONFLICT"));
    assert_eq!(
        fs::read_to_string(addons.join("example/user.txt")).unwrap(),
        "keep"
    );
}
#[test]
fn rejects_unsafe_and_invalid_archives() {
    let temp = tempfile::tempdir().unwrap();
    fs::create_dir_all(temp.path().join("Interface/AddOns")).unwrap();
    let zip = temp.path().join("addon.zip");
    for entries in [
        vec![("../escape", "x")],
        vec![("Addon/file.lua", "x")],
        vec![("Addon/Addon.toc", "x"), ("addon/addon.toc", "y")],
        vec![("Addon/CON.txt", "x")],
    ] {
        archive(&zip, &entries);
        assert!(!run(temp.path(), &zip).status.success(), "{entries:?}");
    }
    fs::write(&zip, "not a zip").unwrap();
    assert!(!run(temp.path(), &zip).status.success());
}

#[test]
fn install_remove_and_refuse_modified_or_unmanaged_folders() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let addons = root.join("Interface/AddOns");
    fs::create_dir_all(&addons).unwrap();
    let zip = root.join("addon.zip");
    archive(
        &zip,
        &[
            ("Example/Example.toc", "## Title: Example"),
            ("Example/main.lua", "hello"),
        ],
    );
    let call = |args: &[&str]| common::command(root).args(args).output().unwrap();
    let z = zip.to_str().unwrap();
    let out = call(&["install-archive", z, "--yes"]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        fs::read_to_string(addons.join("Example/main.lua")).unwrap(),
        "hello"
    );
    assert!(!call(&["install-archive", z, "--yes"]).status.success());
    fs::write(addons.join("Example/main.lua"), "changed").unwrap();
    assert!(!call(&["remove", "Example", "--yes"]).status.success());
    assert_eq!(
        fs::read_to_string(addons.join("Example/main.lua")).unwrap(),
        "changed"
    );
    fs::write(addons.join("Example/main.lua"), "hello").unwrap();
    fs::write(addons.join("Example/extra.txt"), "user file").unwrap();
    assert!(!call(&["remove", "Example", "--yes"]).status.success());
    fs::remove_file(addons.join("Example/extra.txt")).unwrap();
    let out = call(&["remove", "Example", "--yes"]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(!addons.join("Example").exists());
    let retained = fs::read_dir(root.join(".wowza"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| p.join("data/Example/main.lua").exists())
        .unwrap();
    assert_eq!(
        fs::read_to_string(retained.join("data/Example/main.lua")).unwrap(),
        "hello"
    );
    fs::create_dir(addons.join("Other")).unwrap();
    fs::write(addons.join("Other/file"), "untouched").unwrap();
    assert!(!call(&["remove", "Other", "--yes"]).status.success());
    assert_eq!(
        fs::read_to_string(addons.join("Other/file")).unwrap(),
        "untouched"
    );
}

#[test]
fn cancellation_and_multifolder_packages() {
    use std::process::Stdio;
    let temp = tempfile::tempdir().unwrap();
    let addons = temp.path().join("Interface/AddOns");
    fs::create_dir_all(&addons).unwrap();
    let zip = temp.path().join("test.zip");
    archive(&zip, &[("A/A.toc", "x")]);
    let mut child = common::command(temp.path())
        .arg("install-archive")
        .arg(&zip)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(b"n\n").unwrap();
    assert!(child.wait_with_output().unwrap().status.success());
    assert!(!addons.join("A").exists());
    assert!(!temp.path().join(".wowza").exists());
    archive(&zip, &[("A/A.toc", "x"), ("B/B.toc", "x")]);
    let out = common::command(temp.path())
        .arg("install-archive")
        .arg(&zip)
        .arg("--yes")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(addons.join("A/A.toc").exists());
    assert!(addons.join("B/B.toc").exists());
    let list = common::command(temp.path()).arg("list").output().unwrap();
    assert!(String::from_utf8_lossy(&list.stdout).contains("[managed]"));
    let remove = common::command(temp.path())
        .args(["remove", "A", "--yes"])
        .output()
        .unwrap();
    assert!(
        remove.status.success(),
        "{}",
        String::from_utf8_lossy(&remove.stderr)
    );
    assert!(!addons.join("A").exists());
    assert!(!addons.join("B").exists());
}

#[test]
fn multifolder_conflict_and_modified_removal_preserve_all_folders() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let addons = root.join("Interface/AddOns");
    fs::create_dir_all(&addons).unwrap();
    let zip = root.join("package.zip");
    archive(&zip, &[("A/A.toc", "a"), ("B/B.toc", "b")]);
    let run = |args: &[&str]| common::command(root).args(args).output().unwrap();
    let archive_path = zip.to_str().unwrap();
    fs::create_dir(addons.join("B")).unwrap();
    fs::write(addons.join("B/keep"), "keep").unwrap();
    assert!(
        !run(&["install-archive", archive_path, "--yes"])
            .status
            .success()
    );
    assert!(!addons.join("A").exists());
    assert_eq!(fs::read_to_string(addons.join("B/keep")).unwrap(), "keep");
    fs::remove_dir_all(addons.join("B")).unwrap();
    assert!(
        run(&["install-archive", archive_path, "--yes"])
            .status
            .success()
    );
    fs::write(addons.join("B/B.toc"), "changed").unwrap();
    assert!(!run(&["remove", "A", "--yes"]).status.success());
    assert_eq!(fs::read_to_string(addons.join("A/A.toc")).unwrap(), "a");
    assert_eq!(
        fs::read_to_string(addons.join("B/B.toc")).unwrap(),
        "changed"
    );
    let doctor = run(&["doctor"]);
    assert!(doctor.status.success());
    assert!(String::from_utf8_lossy(&doctor.stdout).contains("contents changed"));
}

#[test]
fn rejects_traversal_in_untrusted_ownership_and_journal() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let addons = root.join("Interface/AddOns");
    fs::create_dir_all(addons.join("A")).unwrap();
    let outside = root.join("Interface/outside");
    fs::create_dir_all(&outside).unwrap();
    fs::write(outside.join("keep"), "safe").unwrap();
    let package = serde_json::json!({"version":2,"folders":{"A":{},"../outside":{}}});
    let marker = serde_json::to_string(&package).unwrap();
    fs::write(addons.join("A/.wowza-manifest.json"), &marker).unwrap();
    let call = |args: &[&str]| common::command(root).args(args).output().unwrap();
    assert!(!call(&["remove", "A", "--yes"]).status.success());
    let tx = root.join(".wowza/txn-malicious");
    fs::create_dir_all(tx.join("data")).unwrap();
    let journal = serde_json::json!({"version":1,"action":"Remove","package":package,"markers":{"A":marker,"../outside":marker}});
    fs::write(
        tx.join("pending.json"),
        serde_json::to_vec(&journal).unwrap(),
    )
    .unwrap();
    assert!(
        !call(&["recover", "txn-malicious", "--yes"])
            .status
            .success()
    );
    assert_eq!(fs::read_to_string(outside.join("keep")).unwrap(), "safe");
    assert!(addons.join("A").exists());
}

#[test]
fn updates_package_retains_backup_and_refuses_unsafe_changes() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let addons = root.join("Interface/AddOns");
    fs::create_dir_all(&addons).unwrap();
    let old = root.join("old.zip");
    let new = root.join("new.zip");
    archive(&old, &[("A/A.toc", "v1"), ("B/B.toc", "v1")]);
    archive(&new, &[("A/A.toc", "v2"), ("B/B.toc", "v2")]);
    let call = |args: &[&str]| common::command(root).args(args).output().unwrap();
    assert!(
        call(&["install-archive", old.to_str().unwrap(), "--yes"])
            .status
            .success()
    );
    fs::write(addons.join("B/B.toc"), "edited").unwrap();
    assert!(
        !call(&["update-archive", "A", new.to_str().unwrap(), "--yes"])
            .status
            .success()
    );
    assert_eq!(fs::read_to_string(addons.join("A/A.toc")).unwrap(), "v1");
    fs::write(addons.join("B/B.toc"), "v1").unwrap();
    let out = call(&["update-archive", "A", new.to_str().unwrap(), "--yes"]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    for f in ["A/A.toc", "B/B.toc"] {
        assert_eq!(fs::read_to_string(addons.join(f)).unwrap(), "v2");
    }
    let backup = fs::read_dir(root.join(".wowza"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| p.join("updated.json").exists())
        .unwrap();
    assert_eq!(
        fs::read_to_string(backup.join("old/A/A.toc")).unwrap(),
        "v1"
    );
    let list = call(&["list"]);
    assert!(String::from_utf8_lossy(&list.stdout).contains("[managed]"));
    archive(&new, &[("A/A.toc", "v3"), ("C/C.toc", "v3")]);
    assert!(
        !call(&["update-archive", "A", new.to_str().unwrap(), "--yes"])
            .status
            .success()
    );
    assert_eq!(fs::read_to_string(addons.join("A/A.toc")).unwrap(), "v2");
    assert!(!addons.join("C").exists());
}

#[test]
fn update_rejects_a_stale_confirmed_package() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    fs::create_dir_all(root.join("Interface/AddOns")).unwrap();
    let old = root.join("old.zip");
    let next = root.join("next.zip");
    archive(&old, &[("A/A.toc", "v1")]);
    archive(&next, &[("A/A.toc", "v2")]);
    use wowza::manage::{execute, prepare_install, prepare_update};
    execute(prepare_install(root, &old).unwrap()).unwrap();
    let previous = prepare_update(root, "A", &old).unwrap();
    execute(prepare_update(root, "A", &next).unwrap()).unwrap();
    let err = execute(previous).unwrap_err();
    assert!(err.contains("changed after preview"));
    assert_eq!(
        fs::read_to_string(root.join("Interface/AddOns/A/A.toc")).unwrap(),
        "v2"
    );
}

#[test]
fn cancelling_update_leaves_installed_contents_untouched() {
    use std::process::Stdio;
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    fs::create_dir_all(root.join("Interface/AddOns")).unwrap();
    let old = root.join("old.zip");
    let next = root.join("next.zip");
    archive(&old, &[("A/A.toc", "v1")]);
    archive(&next, &[("A/A.toc", "v2")]);
    let call = common::command(root)
        .arg("install-archive")
        .arg(&old)
        .arg("--yes")
        .output()
        .unwrap();
    assert!(call.status.success());
    let mut child = common::command(root)
        .args(["update-archive", "A"])
        .arg(&next)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(b"n\n").unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).contains("Cancelled."));
    assert_eq!(
        fs::read_to_string(root.join("Interface/AddOns/A/A.toc")).unwrap(),
        "v1"
    );
    assert!(wowza::manage::pending(root).unwrap().is_empty());
}
