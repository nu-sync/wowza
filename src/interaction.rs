use std::io::{BufRead, Write};

pub fn confirm(
    input: &mut impl BufRead,
    output: &mut impl Write,
    question: &str,
    yes: bool,
) -> Result<bool, String> {
    if yes {
        return Ok(true);
    }
    write!(output, "{question} [y/N] ").map_err(|e| e.to_string())?;
    output.flush().map_err(|e| e.to_string())?;
    let mut answer = String::new();
    input.read_line(&mut answer).map_err(|e| e.to_string())?;
    Ok(matches!(
        answer.trim().to_ascii_lowercase().as_str(),
        "y" | "yes"
    ))
}

pub fn handoff(
    input: &mut impl BufRead,
    output: &mut impl Write,
    url: &url::Url,
    yes: bool,
    search: bool,
    open: impl FnOnce(&str) -> Result<(), String>,
) -> Result<(), String> {
    writeln!(output, "Destination: {url}").map_err(|e| e.to_string())?;
    if search {
        writeln!(output, "CurseForge will display the results. Check that its Forever filter is selected; the website may change.").map_err(|e| e.to_string())?;
    }
    let (question, cancelled, opened) = if search {
        (
            "Open this search?",
            "Search handoff cancelled.",
            "Search handoff opened. Choose a project and verify its Forever release on CurseForge.",
        )
    } else {
        (
            "Open this page?",
            "Handoff cancelled.",
            "Handoff opened. Complete any steps in CurseForge, selecting Forever if necessary, then scan your installation.",
        )
    };
    if !confirm(input, output, question, yes)? {
        return writeln!(output, "{cancelled}").map_err(|e| e.to_string());
    }
    open(url.as_str()).map_err(|e| format!("Could not open browser: {e}"))?;
    writeln!(output, "{opened}").map_err(|e| e.to_string())
}
