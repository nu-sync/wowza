use std::{
    io::Write,
    process::{Command, Stdio},
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
