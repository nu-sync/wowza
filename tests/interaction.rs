use std::{cell::Cell, io::Cursor};
use wowza::interaction::{confirm, handoff};

#[test]
fn confirmation_is_explicit_and_eof_cancels() {
    for (answer, expected) in [
        ("", false),
        ("\n", false),
        ("n\n", false),
        ("yes\n", true),
        ("Y\n", true),
        ("anything\n", false),
    ] {
        assert_eq!(
            confirm(&mut Cursor::new(answer), &mut Vec::new(), "Apply?", false).unwrap(),
            expected
        );
    }
    let mut output = Vec::new();
    assert!(confirm(&mut Cursor::new(""), &mut output, "Apply?", true).unwrap());
    assert!(output.is_empty());
}

#[test]
fn handoff_opens_only_when_confirmed_and_never_reports_failed_opening_as_success() {
    let url = wowza::search_url("maps").unwrap();
    for search in [false, true] {
        let called = Cell::new(false);
        let mut output = Vec::new();
        handoff(
            &mut Cursor::new("n\n"),
            &mut output,
            &url,
            false,
            search,
            |_| {
                called.set(true);
                Ok(())
            },
        )
        .unwrap();
        assert!(!called.get());
        assert!(String::from_utf8(output).unwrap().contains("cancelled"));
        let mut output = Vec::new();
        handoff(
            &mut Cursor::new(""),
            &mut output,
            &url,
            true,
            search,
            |actual| {
                assert_eq!(actual, url.as_str());
                called.set(true);
                Ok(())
            },
        )
        .unwrap();
        assert!(called.get());
        assert!(String::from_utf8(output).unwrap().contains("opened"));
        let mut output = Vec::new();
        let result = handoff(
            &mut Cursor::new("yes\n"),
            &mut output,
            &url,
            false,
            search,
            |_| Err("browser unavailable".into()),
        );
        assert!(result.unwrap_err().contains("browser unavailable"));
        assert!(!String::from_utf8(output).unwrap().contains("opened"));
    }
}
