use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

#[cfg(target_os = "macos")]
const DEFAULT_ROOT: &str = "/Applications/World of Warcraft/_forever_";
#[cfg(not(target_os = "macos"))]
const DEFAULT_ROOT: &str = r"C:\battlenet\World of Warcraft\_forever_";

#[derive(Serialize, Deserialize)]
struct Settings {
    installation: PathBuf,
}

pub fn file() -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os("WOWZA_CONFIG_FILE") {
        return Ok(path.into());
    }
    default_file().ok_or_else(|| "Set WOWZA_CONFIG_FILE to choose a configuration file.".into())
}

#[cfg(windows)]
fn default_file() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(|path| PathBuf::from(path).join("Wowza/config.json"))
}

#[cfg(target_os = "macos")]
fn default_file() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .map(|home| PathBuf::from(home).join("Library/Application Support/Wowza/config.json"))
}

#[cfg(not(any(windows, target_os = "macos")))]
fn default_file() -> Option<PathBuf> {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
        .map(|dir| dir.join("wowza/config.json"))
}

pub fn validate(path: &Path) -> Result<PathBuf, String> {
    if !crate::addons_path(path).is_dir() {
        return Err(format!(
            "AddOns folder not found at {}. Pass --path with your Forever installation root or run wowza configure <installation>.",
            crate::addons_path(path).display()
        ));
    }
    let root = fs::canonicalize(path).map_err(|e| format!("Cannot resolve installation: {e}"))?;
    for directory in [root.join("Interface"), crate::addons_path(&root)] {
        let metadata = fs::symlink_metadata(&directory)
            .map_err(|e| format!("Cannot inspect {}: {e}", directory.display()))?;
        if !metadata.file_type().is_dir() || metadata.file_type().is_symlink() {
            return Err(format!(
                "{} must be a normal directory, not a link or junction. Select an installation whose Interface and AddOns folders stay inside its root.",
                directory.display()
            ));
        }
    }
    Ok(root)
}

pub fn resolve(explicit: Option<PathBuf>) -> Result<PathBuf, String> {
    if let Some(path) =
        explicit.or_else(|| std::env::var_os("WOWZA_FOREVER_PATH").map(PathBuf::from))
    {
        return validate(&path);
    }
    let saved = match file() {
        Ok(file) => match fs::read(&file) {
            Ok(bytes) => Some(serde_json::from_slice::<Settings>(&bytes)
                .map_err(|e| format!("Cannot read configuration {}: {e}. Run wowza configure <installation> to replace it.", file.display()))?.installation),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(format!("Cannot read configuration {}: {e}", file.display())),
        },
        Err(_) => None,
    };
    validate(&saved.unwrap_or_else(|| DEFAULT_ROOT.into()))
}

pub fn save(root: &Path) -> Result<PathBuf, String> {
    let installation = validate(root)?;
    let file = file()?;
    let parent = file
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let mut temp = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    serde_json::to_writer_pretty(
        &mut temp,
        &Settings {
            installation: installation.clone(),
        },
    )
    .map_err(|e| e.to_string())?;
    temp.flush().map_err(|e| e.to_string())?;
    temp.as_file().sync_all().map_err(|e| e.to_string())?;
    temp.persist(&file)
        .map_err(|e| format!("Cannot save configuration {}: {e}", file.display()))?;
    Ok(installation)
}

/// A copyable shell command (PowerShell on Windows, POSIX shell elsewhere);
/// single quotes protect spaces and metacharacters.
pub fn command(root: &Path, args: &[&str]) -> String {
    #[cfg(windows)]
    let quote = |text: &str| format!("'{}'", text.replace('\'', "''"));
    #[cfg(not(windows))]
    let quote = |text: &str| format!("'{}'", text.replace('\'', r"'\''"));
    let mut parts = vec![
        "wowza".to_string(),
        "--path".into(),
        quote(&root.to_string_lossy()),
    ];
    parts.extend(args.iter().map(|arg| quote(arg)));
    parts.join(" ")
}
