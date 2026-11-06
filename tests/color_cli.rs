mod common;

use common::{Installation, success};
use std::{fs, process::Command, process::Stdio};

fn command(game: &Installation) -> Command {
    let mut command = game.command();
    command
        .env_remove("NO_COLOR")
        .env_remove("CLICOLOR")
        .env_remove("CLICOLOR_FORCE")
        .env_remove("CI")
        .env("TERM", "xterm")
        .stdin(Stdio::null());
    command
}

fn without_style(text: &str) -> String {
    let mut parts = text.split("\x1b[");
    let mut plain = parts.next().unwrap().to_owned();
    for part in parts {
        plain.push_str(part.split_once('m').expect("complete color sequence").1);
    }
    plain
}

fn has_color(text: &str, color: &str) -> bool {
    text.split("\x1b[").skip(1).any(|part| {
        part.split_once('m')
            .is_some_and(|(codes, _)| codes.split(';').any(|code| code == color))
    })
}

fn sample_installation() -> Installation {
    let game = Installation::new();
    fs::create_dir(game.addons().join("Example")).unwrap();
    fs::write(
        game.addons().join("Example/Example.toc"),
        "## Title: Example addon\n## Version: 1.0",
    )
    .unwrap();
    fs::create_dir(game.addons().join("MissingMetadata")).unwrap();
    game
}

#[test]
fn redirected_human_output_and_errors_are_plain_by_default() {
    let game = sample_installation();
    for args in [
        &["status", "--verbose"][..],
        &["--help"],
        &["not-a-command"],
        &["search", " "],
    ] {
        let output = command(&game).args(args).output().unwrap();
        assert!(!output.stdout.contains(&0x1b), "stdout for {args:?}");
        assert!(!output.stderr.contains(&0x1b), "stderr for {args:?}");
    }
    let text = success(
        command(&game)
            .args(["status", "--verbose"])
            .output()
            .unwrap(),
    );
    assert!(text.contains("Example [unmanaged]"));
    assert!(text.contains("No TOC found"));
}

#[test]
fn forced_color_highlights_status_without_changing_its_text() {
    let game = sample_installation();
    let plain = success(
        command(&game)
            .args(["status", "--verbose"])
            .output()
            .unwrap(),
    );
    let styled = success(
        command(&game)
            .args(["status", "--verbose"])
            .env("CLICOLOR_FORCE", "1")
            .output()
            .unwrap(),
    );
    assert_eq!(without_style(&styled), plain);
    assert!(has_color(&styled, "36"), "cyan labels: {styled:?}");
    assert!(has_color(&styled, "35"), "magenta packages: {styled:?}");
    assert!(has_color(&styled, "33"), "yellow warnings: {styled:?}");
}

#[test]
fn forced_color_distinguishes_safe_archives_from_conflicts() {
    let game = Installation::new();
    let archive = game.zip(
        "example.zip",
        &[("Example/Example.toc", "## Title: Example")],
    );
    let preview = success(
        command(&game)
            .arg("inspect")
            .arg(&archive)
            .env("CLICOLOR_FORCE", "1")
            .output()
            .unwrap(),
    );
    assert!(
        has_color(&preview, "32"),
        "green available folder: {preview:?}"
    );
    assert!(without_style(&preview).contains("Example: 1 files [no existing folder]"));
    fs::create_dir(game.addons().join("Example")).unwrap();
    let conflict = command(&game)
        .arg("inspect")
        .arg(&archive)
        .env("CLICOLOR_FORCE", "1")
        .output()
        .unwrap();
    assert!(!conflict.status.success());
    let stdout = String::from_utf8(conflict.stdout).unwrap();
    let stderr = String::from_utf8(conflict.stderr).unwrap();
    assert!(has_color(&stdout, "31"), "red conflict: {stdout:?}");
    assert!(has_color(&stderr, "31"), "red error: {stderr:?}");
    assert!(without_style(&stdout).contains("[CONFLICT: already exists]"));
    assert!(without_style(&stderr).contains("Archive conflicts with existing AddOns entries."));
}

#[test]
fn color_preferences_apply_to_output_help_and_errors() {
    let game = sample_installation();
    for args in [
        &["status"][..],
        &["--help"],
        &["not-a-command"],
        &["search", " "],
        &["search", "maps"],
    ] {
        let plain = command(&game).args(args).output().unwrap();
        let forced = command(&game)
            .args(args)
            .env("CLICOLOR_FORCE", "1")
            .output()
            .unwrap();
        assert_eq!(forced.status.code(), plain.status.code());
        assert!(
            forced.stdout.contains(&0x1b) || forced.stderr.contains(&0x1b),
            "forced colors for {args:?}"
        );
        for (actual, expected) in [
            (&forced.stdout, &plain.stdout),
            (&forced.stderr, &plain.stderr),
        ] {
            assert_eq!(
                without_style(std::str::from_utf8(actual).unwrap()),
                std::str::from_utf8(expected).unwrap(),
                "{args:?}"
            );
        }
        let disabled = command(&game)
            .args(args)
            .env("CLICOLOR_FORCE", "1")
            .env("NO_COLOR", "1")
            .output()
            .unwrap();
        assert_eq!(disabled.stdout, plain.stdout, "NO_COLOR stdout: {args:?}");
        assert_eq!(disabled.stderr, plain.stderr, "NO_COLOR stderr: {args:?}");
        let disabled = command(&game)
            .args(args)
            .env("CLICOLOR", "0")
            .output()
            .unwrap();
        assert_eq!(disabled.stdout, plain.stdout, "CLICOLOR stdout: {args:?}");
        assert_eq!(disabled.stderr, plain.stderr, "CLICOLOR stderr: {args:?}");
    }
}

#[test]
fn forced_color_keeps_json_and_raw_urls_machine_readable() {
    let game = sample_installation();
    for args in [
        &["status", "--json"][..],
        &["search", "maps", "--json"],
        &[
            "continue",
            "https://www.curseforge.com/wow/addons/questie",
            "--json",
        ],
    ] {
        let output = command(&game)
            .args(args)
            .env("CLICOLOR_FORCE", "1")
            .output()
            .unwrap();
        assert!(output.stderr.is_empty(), "{args:?}");
        let text = success(output);
        assert!(!text.contains('\x1b'), "{args:?}");
        serde_json::from_str::<serde_json::Value>(&text).unwrap();
    }
    for (args, expected) in [
        (
            &["search", "maps", "--url"][..],
            wowza::search_url("maps").unwrap().to_string(),
        ),
        (
            &[
                "continue",
                "https://www.curseforge.com/wow/addons/questie",
                "--url",
            ][..],
            "https://www.curseforge.com/wow/addons/questie".to_owned(),
        ),
    ] {
        let output = command(&game)
            .args(args)
            .env("CLICOLOR_FORCE", "1")
            .output()
            .unwrap();
        assert!(output.stderr.is_empty());
        assert_eq!(success(output), format!("{expected}\n"));
    }
}
