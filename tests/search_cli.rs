mod common;

use std::{
    fs,
    io::Write,
    process::{Command, Output, Stdio},
};
use url::Url;

#[test]
fn search_encodes_terms_and_preserves_filter() {
    let url = wowza::search_url("  bags & maps + 雪  ").unwrap();
    assert_eq!(url.host_str(), Some("www.curseforge.com"));
    assert_eq!(url.path(), "/wow/search");
    let pairs: std::collections::BTreeMap<_, _> = url.query_pairs().into_owned().collect();
    assert_eq!(pairs.get("search").unwrap(), "bags & maps + 雪");
    assert_eq!(pairs.get("gameVersionTypeId").unwrap(), "88568");
    assert_eq!(pairs.get("class").unwrap(), "addons");
    assert!(wowza::search_url("  ").is_err());
}

#[test]
fn cli_displays_destination_and_cancels_by_default() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_wowza"))
        .args(["search", "bags & maps"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(b"n\n").unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success());
    let text = String::from_utf8(out.stdout).unwrap();
    let link = text
        .lines()
        .find_map(|l| l.strip_prefix("Destination: "))
        .unwrap();
    let parsed = Url::parse(link).unwrap();
    assert!(
        parsed
            .query_pairs()
            .any(|(k, v)| k == "search" && v == "bags & maps")
    );
    assert!(
        parsed
            .query_pairs()
            .any(|(k, v)| k == "gameVersionTypeId" && v == "88568")
    );
    assert!(text.contains("Open this search? [y/N]"));
    assert!(text.contains("Search handoff cancelled."));
    assert!(!text.contains("Search handoff opened."));
}

#[test]
fn cli_rejects_blank_search() {
    let out = Command::new(env!("CARGO_BIN_EXE_wowza"))
        .args(["search", " ", "--yes"])
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("Enter a search term"));
}

fn machine_output(args: &[&str]) -> Output {
    let root = tempfile::tempdir().unwrap();
    let config = root.path().join("config.json");
    fs::write(&config, "not valid configuration JSON").unwrap();
    let before = common::snapshot(root.path());
    let output = Command::new(env!("CARGO_BIN_EXE_wowza"))
        .args(args)
        .env_remove("WOWZA_FOREVER_PATH")
        .env("WOWZA_CONFIG_FILE", config)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert_eq!(common::snapshot(root.path()), before);
    output
}

#[test]
fn search_url_mode_prints_only_the_encoded_url_without_configuration() {
    for options in [vec![], vec!["--yes"]] {
        let mut args = vec!["search", "  bags & maps + 雪  ", "--url"];
        args.extend(options);
        let output = machine_output(&args);
        assert!(output.stderr.is_empty());
        assert_eq!(
            common::success(output),
            "https://www.curseforge.com/wow/search?class=addons&page=1&pageSize=20&sortBy=relevancy&search=bags+%26+maps+%2B+%E9%9B%AA&gameVersionTypeId=88568\n"
        );
    }
}

#[test]
fn search_json_is_a_url_handoff_even_with_yes_or_url_flags() {
    for options in [vec![], vec!["--url"], vec!["--yes"], vec!["--url", "--yes"]] {
        let mut args = vec!["--json", "search", "  bags & maps + 雪  "];
        args.extend(options);
        let output = machine_output(&args);
        assert!(output.stderr.is_empty());
        let document: serde_json::Value = serde_json::from_str(&common::success(output)).unwrap();
        assert_eq!(
            document,
            serde_json::json!({
                "kind": "url_handoff",
                "action": "search",
                "query": "bags & maps + 雪",
                "url": "https://www.curseforge.com/wow/search?class=addons&page=1&pageSize=20&sortBy=relevancy&search=bags+%26+maps+%2B+%E9%9B%AA&gameVersionTypeId=88568",
                "content_fetched": false
            })
        );
    }
}

#[test]
fn continue_machine_modes_return_validated_urls_without_prompts() {
    let input = "https://WWW.CURSEFORGE.COM/wow/addons/questie";
    let normalized = "https://www.curseforge.com/wow/addons/questie";
    for options in [vec![], vec!["--yes"]] {
        let mut args = vec!["continue", input, "--url"];
        args.extend(options);
        let output = machine_output(&args);
        assert!(output.stderr.is_empty());
        assert_eq!(common::success(output), format!("{normalized}\n"));
    }

    for options in [vec![], vec!["--url"], vec!["--yes"], vec!["--url", "--yes"]] {
        let mut args = vec!["continue", input, "--json"];
        args.extend(options);
        let output = machine_output(&args);
        assert!(output.stderr.is_empty());
        let document: serde_json::Value = serde_json::from_str(&common::success(output)).unwrap();
        assert_eq!(
            document,
            serde_json::json!({
                "kind": "url_handoff",
                "action": "continue",
                "url": normalized,
                "content_fetched": false
            })
        );
    }
}

#[test]
fn machine_handoffs_do_not_resolve_or_create_an_explicit_installation() {
    let root = tempfile::tempdir().unwrap();
    let missing = root.path().join("missing-game");
    let config = root.path().join("config.json");
    fs::write(&config, "invalid settings").unwrap();
    let before = common::snapshot(root.path());
    for (command, input) in [
        ("search", "Questie"),
        ("continue", "https://www.curseforge.com/wow/addons/questie"),
    ] {
        for mode in ["--url", "--json"] {
            let output = Command::new(env!("CARGO_BIN_EXE_wowza"))
                .arg("--path")
                .arg(&missing)
                .args([command, input, mode])
                .env("WOWZA_FOREVER_PATH", &missing)
                .env("WOWZA_CONFIG_FILE", &config)
                .stdin(Stdio::null())
                .output()
                .unwrap();
            common::success(output);
            assert_eq!(common::snapshot(root.path()), before);
        }
    }
}

#[test]
fn machine_handoffs_reject_invalid_inputs_without_output() {
    for mode in ["--url", "--json"] {
        let output = machine_output(&["search", " \t ", mode]);
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8_lossy(&output.stderr).contains("Enter a search term"));

        for input in [
            "https://example.com/wow/addons/questie",
            "http://www.curseforge.com/wow/addons/questie",
            "https://user@www.curseforge.com/wow/addons/questie",
            "https://www.curseforge.com/minecraft/mc-mods/example",
        ] {
            let output = machine_output(&["continue", input, mode]);
            assert!(!output.status.success());
            assert!(output.stdout.is_empty());
            assert!(
                String::from_utf8_lossy(&output.stderr).contains("Expected an HTTPS CurseForge")
            );
        }
    }
}

#[test]
fn dry_run_is_not_a_browser_or_machine_handoff_mode() {
    for (command, input) in [
        ("search", "Questie"),
        ("continue", "https://www.curseforge.com/wow/addons/questie"),
    ] {
        for options in [vec![], vec!["--url"], vec!["--json"]] {
            let mut args = vec![command, input, "--dry-run"];
            args.extend(options);
            let output = machine_output(&args);
            assert!(!output.status.success());
            assert!(output.stdout.is_empty());
            assert!(String::from_utf8_lossy(&output.stderr).contains("--dry-run"));
        }
    }
}
