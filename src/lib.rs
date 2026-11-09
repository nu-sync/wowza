use std::{
    collections::BTreeMap,
    fs, io,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, serde::Serialize)]
pub struct Addon {
    pub folder: String,
    pub tocs: Vec<Toc>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct Toc {
    pub file: String,
    pub metadata: BTreeMap<String, String>,
}

impl Addon {
    /// The TOC the Forever client loads: `<Folder>_Forever.toc`, then `<Folder>.toc`.
    pub fn primary_toc(&self) -> Option<&Toc> {
        let named = |suffix: &str| {
            let name = format!("{}{suffix}.toc", self.folder);
            self.tocs
                .iter()
                .find(|t| t.file.eq_ignore_ascii_case(&name))
        };
        named("_Forever")
            .or_else(|| named(""))
            .or_else(|| self.tocs.first())
    }
}

/// Remove WoW UI escape sequences such as `|cffRRGGBB` and `|r` from display text.
pub fn strip_ui_escapes(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '|' {
            out.push(c);
            continue;
        }
        match chars.peek() {
            Some('c' | 'C') => {
                chars.next();
                for _ in 0..8 {
                    if chars.next_if(char::is_ascii_hexdigit).is_none() {
                        break;
                    }
                }
            }
            Some('r' | 'R') => {
                chars.next();
            }
            Some('|') => {
                chars.next();
                out.push('|');
            }
            _ => out.push('|'),
        }
    }
    out.trim().to_string()
}

pub fn parse_toc(text: &str) -> BTreeMap<String, String> {
    let mut fields = BTreeMap::new();
    for line in text.lines() {
        let line = line.trim().trim_start_matches('\u{feff}');
        if let Some((key, value)) = line.strip_prefix("##").and_then(|s| s.split_once(':')) {
            let key = key.trim().to_ascii_lowercase();
            if !key.is_empty() {
                fields.insert(key, value.trim().to_string());
            }
        }
    }
    fields
}

pub fn addons_path(root: &Path) -> PathBuf {
    root.join("Interface").join("AddOns")
}

pub fn scan(root: &Path) -> io::Result<Vec<Addon>> {
    let mut addons = Vec::new();
    for entry in fs::read_dir(addons_path(root))? {
        let entry = entry?;
        // Do not follow directory symlinks outside the installation.
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let mut tocs = Vec::new();
        for file in fs::read_dir(entry.path())? {
            let file = file?;
            if file.file_type()?.is_file()
                && file
                    .path()
                    .extension()
                    .is_some_and(|e| e.eq_ignore_ascii_case("toc"))
            {
                let bytes = fs::read(file.path())?;
                tocs.push(Toc {
                    file: file.file_name().to_string_lossy().into_owned(),
                    metadata: parse_toc(&String::from_utf8_lossy(&bytes)),
                });
            }
        }
        tocs.sort_by(|a, b| a.file.cmp(&b.file));
        addons.push(Addon {
            folder: entry.file_name().to_string_lossy().into_owned(),
            tocs,
        });
    }
    addons.sort_by_key(|a| a.folder.to_lowercase());
    Ok(addons)
}

pub fn project_url(input: &str) -> Result<url::Url, String> {
    let url = url::Url::parse(input).map_err(|e| format!("Invalid URL: {e}"))?;
    let parts: Vec<_> = url
        .path_segments()
        .map(|p| p.filter(|s| !s.is_empty()).collect())
        .unwrap_or_default();
    if url.scheme() != "https"
        || url.host_str() != Some("www.curseforge.com") && url.host_str() != Some("curseforge.com")
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some()
        || parts.len() < 3
        || parts[0] != "wow"
        || parts[1] != "addons"
    {
        return Err("Expected an HTTPS CurseForge WoW add-on project URL, e.g. https://www.curseforge.com/wow/addons/example".into());
    }
    Ok(url)
}

/// Build a link to CurseForge's public WoW search page with the Forever filter.
pub fn search_url(query: &str) -> Result<url::Url, String> {
    let query = query.trim();
    if query.is_empty() {
        return Err("Enter a search term.".into());
    }
    let mut url =
        url::Url::parse("https://www.curseforge.com/wow/search").expect("static URL is valid");
    url.query_pairs_mut()
        .append_pair("class", "addons")
        .append_pair("page", "1")
        .append_pair("pageSize", "20")
        .append_pair("sortBy", "relevancy")
        .append_pair("search", query)
        .append_pair("gameVersionTypeId", "88568");
    Ok(url)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_metadata_and_bom() {
        let m = parse_toc(
            "\u{feff}## Title: My Addon\n## Interface: 12345\n## X-Test: a: b\nfile.lua\n",
        );
        assert_eq!(m["title"], "My Addon");
        assert_eq!(m["x-test"], "a: b");
        assert_eq!(m.len(), 3);
    }
    #[test]
    fn accepts_only_curseforge_project_urls() {
        assert!(project_url("https://www.curseforge.com/wow/addons/example/files/1").is_ok());
        for url in [
            "https://curseforge.com.evil.test/wow/addons/x",
            "http://curseforge.com/wow/addons/x",
            "https://curseforge.com/wow/addons",
            "https://user@curseforge.com/wow/addons/x",
        ] {
            assert!(project_url(url).is_err(), "{url}");
        }
    }
    #[test]
    fn primary_toc_prefers_forever_then_folder_name() {
        let toc = |file: &str| Toc {
            file: file.into(),
            metadata: BTreeMap::new(),
        };
        let mut addon = Addon {
            folder: "Questie".into(),
            tocs: vec![
                toc("Questie.toc"),
                toc("Questie_Classic.toc"),
                toc("questie_forever.TOC"),
            ],
        };
        assert_eq!(addon.primary_toc().unwrap().file, "questie_forever.TOC");
        addon.tocs.pop();
        assert_eq!(addon.primary_toc().unwrap().file, "Questie.toc");
        addon.tocs.remove(0);
        assert_eq!(addon.primary_toc().unwrap().file, "Questie_Classic.toc");
        addon.tocs.clear();
        assert!(addon.primary_toc().is_none());
    }

    #[test]
    fn strips_ui_escapes() {
        assert_eq!(strip_ui_escapes("|cffe6b85cOpen|rMap"), "OpenMap");
        assert_eq!(
            strip_ui_escapes("Questie|cFFFF0000 game client not supported|r "),
            "Questie game client not supported"
        );
        assert_eq!(strip_ui_escapes("A || B |x"), "A | B |x");
    }

    #[test]
    fn scans_folders_and_multiple_tocs() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = addons_path(tmp.path()).join("Example");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("Example.toc"), "## Title: Example\n## Version: 1").unwrap();
        fs::write(dir.join("Example_Classic.toc"), "## Interface: 123").unwrap();
        let found = scan(tmp.path()).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].tocs.len(), 2);
        assert_eq!(found[0].tocs[0].metadata["title"], "Example");
    }
}

pub mod archive;

pub mod config;
pub mod interaction;
pub mod manage;
