use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs::File,
    io::{self, Read, Seek},
    path::Path,
};

const MAX_ENTRIES: usize = 10_000;
const MAX_BYTES: u64 = 512 * 1024 * 1024;
const RESERVED_MANIFEST: &str = ".wowza-manifest.json";

#[derive(Debug)]
pub struct ArchivePreview {
    pub folders: BTreeMap<String, usize>,
    pub bytes: u64,
}

/// Inspect archive structure and fully read every file to validate its CRC. Nothing is extracted.
pub fn inspect(path: &Path) -> Result<ArchivePreview, String> {
    let file = File::open(path).map_err(|e| format!("Cannot open archive: {e}"))?;
    inspect_reader(file, MAX_BYTES)
}

fn inspect_reader<R: Read + Seek>(reader: R, max_bytes: u64) -> Result<ArchivePreview, String> {
    let mut zip = zip::ZipArchive::new(reader).map_err(|e| format!("Invalid ZIP: {e}"))?;
    if zip.len() > MAX_ENTRIES {
        return Err("Archive has too many entries (limit 10,000).".into());
    }
    let mut folders = BTreeMap::new();
    let mut explicit = HashSet::new();
    // Track implicit directories too, so a file cannot also be an ancestor.
    let mut nodes: HashMap<String, (String, bool)> = HashMap::new();
    let mut bytes = 0u64;
    let mut tocs = HashSet::new();
    let mut roots = HashSet::new();
    for i in 0..zip.len() {
        let mut entry = zip
            .by_index(i)
            .map_err(|e| format!("Cannot read ZIP entry: {e}"))?;
        let name = std::str::from_utf8(entry.name_raw())
            .map_err(|_| "Archive entry names must be UTF-8.")?
            .to_owned();
        // Using the ZIP library's decoded name for extraction must not change the name we validate.
        if entry.name() != name {
            return Err(format!("Unsupported archive path encoding: {name}"));
        }
        let trimmed = name.strip_suffix('/').unwrap_or(&name);
        let parts: Vec<_> = trimmed.split('/').collect();
        let mode = entry.unix_mode().unwrap_or(0) & 0o170000;
        let mode_ok = mode == 0
            || (entry.is_dir() && mode == 0o040000)
            || (!entry.is_dir() && mode == 0o100000);
        if parts
            .iter()
            .any(|p| !valid_component(p) || p.eq_ignore_ascii_case(RESERVED_MANIFEST))
            || !mode_ok
        {
            return Err(format!("Unsafe archive path or file type: {name}"));
        }
        if name.encode_utf16().count() > 32_000 {
            return Err(format!("Archive path too long: {name}"));
        }
        if parts.len() == 1 && !entry.is_dir() {
            return Err(format!("File is outside an add-on folder: {name}"));
        }
        let normalized = trimmed.to_lowercase();
        if !explicit.insert(normalized) {
            return Err(format!("Duplicate archive path: {name}"));
        }
        for n in 1..=parts.len() {
            let path = parts[..n].join("/");
            let key = path.to_lowercase();
            let is_file = n == parts.len() && !entry.is_dir();
            if let Some((old, old_file)) = nodes.get(&key) {
                if old != &path {
                    return Err(format!("Inconsistent path casing: {name}"));
                }
                if *old_file != is_file {
                    return Err(format!("File and directory conflict: {name}"));
                }
            } else {
                nodes.insert(key, (path, is_file));
            }
        }
        roots.insert(parts[0].to_string());
        if entry.is_dir() {
            continue;
        }
        let declared = entry.size();
        let remaining = max_bytes
            .checked_sub(bytes)
            .ok_or("Archive size overflow")?;
        if declared > remaining {
            return Err(format!(
                "Archive exceeds the {max_bytes} byte uncompressed limit."
            ));
        }
        let read = io::copy(
            &mut entry.by_ref().take(remaining.saturating_add(1)),
            &mut io::sink(),
        )
        .map_err(|e| format!("Cannot read {name}: {e}"))?;
        if read != declared || read > remaining {
            return Err(format!("Invalid uncompressed size for {name}"));
        }
        bytes += read;
        *folders.entry(parts[0].to_string()).or_insert(0) += 1;
        if parts.len() == 2 && parts[1].to_ascii_lowercase().ends_with(".toc") {
            tocs.insert(parts[0].to_string());
        }
    }
    if folders.is_empty() {
        return Err("Archive contains no add-on files.".into());
    }
    for folder in roots {
        if !folders.contains_key(&folder) || !tocs.contains(&folder) {
            return Err(format!("Folder {folder} has no top-level TOC file."));
        }
    }
    Ok(ArchivePreview { folders, bytes })
}

pub(crate) fn valid_component(s: &str) -> bool {
    !s.contains('/') && !invalid_component(s)
}

fn invalid_component(s: &str) -> bool {
    let stem = s
        .split('.')
        .next()
        .unwrap_or("")
        .trim_end_matches(' ')
        .to_ascii_uppercase();
    let is_device = matches!(
        stem.as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "CLOCK$" | "CONIN$" | "CONOUT$"
    ) || ["COM", "LPT"].iter().any(|prefix| {
        stem.strip_prefix(prefix).is_some_and(|suffix| {
            matches!(
                suffix,
                "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
            )
        })
    });
    s.is_empty()
        || s == "."
        || s == ".."
        || s.ends_with(['.', ' '])
        || s.encode_utf16().count() > 255
        || s.chars()
            .any(|c| c.is_control() || "\\:*?\"<>|".contains(c))
        || is_device
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Cursor, Write};
    use zip::{ZipWriter, write::SimpleFileOptions};

    fn make_zip(entries: &[(&str, &[u8])]) -> Cursor<Vec<u8>> {
        let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
        for (name, content) in entries {
            zip.start_file(
                *name,
                SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored),
            )
            .unwrap();
            zip.write_all(content).unwrap();
        }
        zip.finish().unwrap()
    }

    #[test]
    fn validates_multiple_folders_and_actual_size() {
        let zip = make_zip(&[
            ("A/A.toc", b"abc"),
            ("A/sub/file.lua", b"data"),
            ("B/B.toc", b"z"),
        ]);
        let preview = inspect_reader(zip, 8).unwrap();
        assert_eq!(preview.bytes, 8);
        assert_eq!(preview.folders.get("A"), Some(&2));
        assert_eq!(preview.folders.get("B"), Some(&1));
        let zip = make_zip(&[("A/A.toc", b"abc")]);
        assert!(inspect_reader(zip, 2).unwrap_err().contains("limit"));
    }

    #[test]
    fn rejects_ambiguous_and_unsafe_paths() {
        for name in [
            "../escape",
            "/absolute",
            "A/../file",
            "A/x\\file",
            "A/CON .txt",
            "A/COM¹.txt",
            "A/.wowza-manifest.json",
            "A/file:stream",
            "A/name. ",
        ] {
            let zip = make_zip(&[("A/A.toc", b"x"), (name, b"x")]);
            assert!(inspect_reader(zip, 100).is_err(), "{name}");
        }
        for entries in [
            vec![("A/A.toc", b"x".as_slice()), ("a/b.lua", b"x".as_slice())],
            vec![
                ("A/A.toc", b"x".as_slice()),
                ("A/file", b"x".as_slice()),
                ("A/file/child", b"x".as_slice()),
            ],
            vec![("A/A.toc", b"x".as_slice()), ("A/A.TOC", b"x".as_slice())],
        ] {
            assert!(
                inspect_reader(make_zip(&entries), 100).is_err(),
                "{entries:?}"
            );
        }
    }

    #[test]
    fn component_validator_rejects_path_separators() {
        assert!(!valid_component("../outside"));
        assert!(!valid_component("a/b"));
        assert!(!valid_component(r"a\b"));
        assert!(valid_component("Safe_Addon"));
    }

    #[test]
    fn detects_corrupt_file_data() {
        let zip = make_zip(&[("A/A.toc", b"unique payload to corrupt")]);
        let mut data = zip.into_inner();
        let needle = b"unique payload to corrupt";
        let pos = data
            .windows(needle.len())
            .position(|w| w == needle)
            .unwrap();
        data[pos] ^= 1;
        assert!(inspect_reader(Cursor::new(data), 100).is_err());
    }
}
