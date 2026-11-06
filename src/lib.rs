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
    addons.sort_by(|a, b| a.folder.to_lowercase().cmp(&b.folder.to_lowercase()));
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
