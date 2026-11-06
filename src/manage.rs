use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, HashSet},
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};

mod hooks;
mod inspection;
mod plan;
pub use inspection::{
    Inspection, PackageSummary, PendingOperation, inspect_installation, pending_operations,
};
pub use plan::{
    OperationPlan, OperationPreview, OperationResult, execute, prepare_install, prepare_recovery,
    prepare_remove, prepare_update,
};

const MARKER: &str = ".wowza-manifest.json";
const JOURNAL: &str = "pending.json";
type Files = BTreeMap<String, String>;
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
struct Package {
    version: u32,
    folders: BTreeMap<String, Files>,
}
#[derive(Deserialize)]
struct Legacy {
    version: u32,
    folder: String,
    files: Files,
}
#[derive(Serialize, Deserialize)]
struct Transaction {
    version: u32,
    action: Action,
    package: Package,
    markers: BTreeMap<String, String>,
}
#[derive(Serialize, Deserialize, PartialEq, Eq)]
enum Action {
    Install,
    Remove,
}
#[derive(Serialize, Deserialize)]
struct UpdateJournal {
    version: u32,
    old: Package,
    new: Package,
    old_markers: BTreeMap<String, String>,
    new_markers: BTreeMap<String, String>,
}
const UPDATE_JOURNAL: &str = "update-pending.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Unmanaged,
    Managed,
    Modified,
    Incomplete,
}

fn valid_name(name: &str) -> bool {
    crate::archive::valid_component(name)
}

fn validate(package: &Package) -> Result<(), String> {
    if package.version != 2 || package.folders.is_empty() || package.folders.len() > 10_000 {
        return Err("Invalid package ownership record.".into());
    }
    let mut seen = HashSet::new();
    for name in package.folders.keys() {
        if !valid_name(name) || !seen.insert(name.to_lowercase()) {
            return Err("Unsafe or duplicate folder in ownership record.".into());
        }
    }
    Ok(())
}
fn inventory(root: &Path) -> Result<Files, String> {
    fn walk(root: &Path, dir: &Path, out: &mut Files) -> Result<(), String> {
        for e in fs::read_dir(dir).map_err(|e| e.to_string())? {
            let e = e.map_err(|e| e.to_string())?;
            let p = e.path();
            let kind = e.file_type().map_err(|e| e.to_string())?;
            let rel = p
                .strip_prefix(root)
                .map_err(|e| e.to_string())?
                .to_str()
                .ok_or("Non-UTF-8 path")?
                .replace('\\', "/");
            if rel == MARKER {
                continue;
            }
            if kind.is_symlink() {
                return Err(format!("Symlink found: {rel}"));
            }
            if kind.is_dir() {
                out.insert(format!("{rel}/"), "directory".into());
                walk(root, &p, out)?;
            } else if kind.is_file() {
                let mut f = fs::File::open(&p).map_err(|e| e.to_string())?;
                let mut hash = Sha256::new();
                std::io::copy(&mut f, &mut hash).map_err(|e| e.to_string())?;
                out.insert(rel, format!("{:x}", hash.finalize()));
            } else {
                return Err(format!("Unsupported file type: {rel}"));
            }
        }
        Ok(())
    }
    let mut files = Files::new();
    walk(root, root, &mut files)?;
    Ok(files)
}
fn normal_dir(p: &Path) -> Result<(), String> {
    let md = fs::symlink_metadata(p).map_err(|e| format!("Cannot inspect {}: {e}", p.display()))?;
    if !md.file_type().is_dir() || md.file_type().is_symlink() {
        return Err(format!("{} must be a normal directory", p.display()));
    }
    Ok(())
}
fn state_dir(root: &Path) -> Result<PathBuf, String> {
    let p = root.join(".wowza");
    match fs::create_dir(&p) {
        Ok(()) => (),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => (),
        Err(e) => return Err(format!("Cannot create {}: {e}", p.display())),
    }
    normal_dir(&p)?;
    Ok(p)
}
fn acquire_lock(root: &Path) -> Result<fs::File, String> {
    let path = state_dir(root)?.join("lock");
    match fs::symlink_metadata(&path) {
        Ok(md) if !md.file_type().is_file() || md.file_type().is_symlink() => {
            return Err("Wowza lock must be a normal file.".into());
        }
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
            return Err(format!("Cannot inspect Wowza lock: {e}"));
        }
        _ => (),
    }
    let f = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
        .map_err(|e| format!("Cannot open Wowza lock: {e}"))?;
    f.try_lock()
        .map_err(|e| format!("Cannot acquire Wowza lock; another operation may be running: {e}"))?;
    Ok(f)
}
#[cfg(windows)]
fn move_no_replace(from: &Path, to: &Path) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn MoveFileW(from: *const u16, to: *const u16) -> i32;
    }
    let from: Vec<u16> = from.as_os_str().encode_wide().chain(Some(0)).collect();
    let to: Vec<u16> = to.as_os_str().encode_wide().chain(Some(0)).collect();
    if unsafe { MoveFileW(from.as_ptr(), to.as_ptr()) } == 0 {
        return Err(format!(
            "Cannot move folder without replacing an existing entry: {}",
            std::io::Error::last_os_error()
        ));
    }
    Ok(())
}
#[cfg(not(windows))]
fn move_no_replace(_: &Path, _: &Path) -> Result<(), String> {
    Err("Install and remove currently support Windows only.".into())
}
fn read_marker(folder: &Path) -> Result<String, String> {
    normal_dir(folder)?;
    let p = folder.join(MARKER);
    let md = fs::symlink_metadata(&p)
        .map_err(|_| "No Wowza ownership record; refusing removal.".to_string())?;
    if !md.file_type().is_file() || md.len() > 4_000_000 {
        return Err("Invalid ownership record.".into());
    }
    let mut s = String::new();
    fs::File::open(p)
        .map_err(|e| e.to_string())?
        .read_to_string(&mut s)
        .map_err(|e| e.to_string())?;
    Ok(s)
}
fn parse_marker(s: &str, expected_folder: &str) -> Result<Package, String> {
    let value: serde_json::Value =
        serde_json::from_str(s).map_err(|e| format!("Invalid ownership record: {e}"))?;
    let package = match value.get("version").and_then(|v| v.as_u64()) {
        Some(1) => {
            let old: Legacy = serde_json::from_value(value).map_err(|e| e.to_string())?;
            if old.version != 1 || old.folder != expected_folder {
                return Err("Legacy ownership record does not match folder.".into());
            }
            Package {
                version: 2,
                folders: BTreeMap::from([(old.folder, old.files)]),
            }
        }
        Some(2) => serde_json::from_value(value).map_err(|e| e.to_string())?,
        _ => return Err("Unsupported ownership record version.".into()),
    };
    validate(&package)?;
    if !package.folders.contains_key(expected_folder) {
        return Err("Ownership record does not include this folder.".into());
    }
    Ok(package)
}
fn verify_folder(path: &Path, folder: &str, package: &Package) -> Result<String, String> {
    let marker = read_marker(path)?;
    if parse_marker(&marker, folder)? != *package || inventory(path)? != package.folders[folder] {
        return Err(format!(
            "Folder {folder} was modified or its ownership record differs."
        ));
    }
    Ok(marker)
}
fn preflight(addons: &Path, package: &Package) -> Result<(), String> {
    normal_dir(addons)?;
    let existing: HashSet<String> = fs::read_dir(addons)
        .map_err(|e| e.to_string())?
        .map(|e| e.map(|e| e.file_name().to_string_lossy().to_lowercase()))
        .collect::<Result<_, _>>()
        .map_err(|e| e.to_string())?;
    for folder in package.folders.keys() {
        if existing.contains(&folder.to_lowercase()) {
            return Err(format!(
                "Folder {folder} already exists; nothing was overwritten."
            ));
        }
    }
    Ok(())
}
fn new_transaction(root: &Path) -> Result<tempfile::TempDir, String> {
    tempfile::Builder::new()
        .prefix("txn-")
        .tempdir_in(state_dir(root)?)
        .map_err(|e| e.to_string())
}
fn write_journal(tx: &Path, journal: &Transaction) -> Result<(), String> {
    let encoded = serde_json::to_vec(journal).map_err(|e| e.to_string())?;
    if encoded.len() > 16_000_000 {
        return Err("Transaction record exceeds size limit.".into());
    }
    let mut f = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(tx.join("journal.tmp"))
        .map_err(|e| e.to_string())?;
    f.write_all(&encoded).map_err(|e| e.to_string())?;
    f.sync_all().map_err(|e| e.to_string())?;
    drop(f);
    move_no_replace(&tx.join("journal.tmp"), &tx.join(JOURNAL))?;
    Ok(())
}
fn read_journal(tx: &Path) -> Result<Transaction, String> {
    normal_dir(tx)?;
    normal_dir(&tx.join("data"))?;
    let p = tx.join(JOURNAL);
    let md = fs::symlink_metadata(&p).map_err(|e| e.to_string())?;
    if !md.is_file() || md.file_type().is_symlink() || md.len() > 16_000_000 {
        return Err("Invalid transaction journal.".into());
    }
    let tx: Transaction = serde_json::from_reader(fs::File::open(p).map_err(|e| e.to_string())?)
        .map_err(|e| format!("Invalid transaction journal: {e}"))?;
    validate(&tx.package)?;
    if tx.version != 1 || tx.markers.len() != tx.package.folders.len() {
        return Err("Invalid transaction journal.".into());
    }
    for folder in tx.package.folders.keys() {
        let marker = tx.markers.get(folder).ok_or("Missing transaction marker")?;
        if parse_marker(marker, folder)? != tx.package {
            return Err("Transaction ownership records disagree.".into());
        }
    }
    Ok(tx)
}
fn tx_error(path: &Path, err: String) -> String {
    let root = path
        .parent()
        .and_then(Path::parent)
        .expect("transaction has an installation root");
    let transaction = path.file_name().unwrap().to_string_lossy();
    format!(
        "{err} Transaction retained at {}. After resolving conflicts, run: {}",
        path.display(),
        crate::config::command(root, &["recover", &transaction])
    )
}

const MAX_ARCHIVE_BYTES: u64 = 512 * 1024 * 1024;
fn archive_digest(path: &Path) -> Result<String, String> {
    let file = fs::File::open(path).map_err(|e| format!("Cannot open archive: {e}"))?;
    let mut limited = file.take(MAX_ARCHIVE_BYTES + 1);
    let mut hash = Sha256::new();
    let bytes = std::io::copy(&mut limited, &mut hash).map_err(|e| e.to_string())?;
    if bytes > MAX_ARCHIVE_BYTES {
        return Err("Compressed archive exceeds 512 MiB limit.".into());
    }
    Ok(format!("{:x}", hash.finalize()))
}
fn ensure_no_pending(root: &Path) -> Result<(), String> {
    let txs = pending(root)?;
    if !txs.is_empty() {
        return Err(format!(
            "Pending Wowza transaction(s): {}. Recover them before another operation.",
            txs.join(", ")
        ));
    }
    Ok(())
}
fn stage_archive(
    stage: &Path,
    archive: &Path,
    expected_sha256: &str,
) -> Result<(Package, BTreeMap<String, String>), String> {
    let zip_path = stage.join("input.zip");
    let input = fs::File::open(archive).map_err(|e| format!("Cannot open archive: {e}"))?;
    let mut output = fs::File::create(&zip_path).map_err(|e| e.to_string())?;
    let copied = std::io::copy(&mut input.take(MAX_ARCHIVE_BYTES + 1), &mut output)
        .map_err(|e| format!("Cannot copy archive: {e}"))?;
    drop(output);
    if copied > MAX_ARCHIVE_BYTES {
        return Err("Compressed archive exceeds 512 MiB limit.".into());
    }
    if archive_digest(&zip_path)? != expected_sha256 {
        return Err("Archive changed after preview; inspect it again.".into());
    }
    let preview = crate::archive::inspect(&zip_path)?;
    let mut package = Package {
        version: 2,
        folders: preview
            .folders
            .keys()
            .map(|n| (n.clone(), Files::new()))
            .collect(),
    };
    validate(&package)?;
    let data = stage.join("data");
    fs::create_dir(&data).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipArchive::new(fs::File::open(zip_path).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).map_err(|e| e.to_string())?;
        let name = entry.name().to_string();
        if name.split('/').any(|p| p.eq_ignore_ascii_case(MARKER)) {
            return Err("Archive contains reserved Wowza manifest name.".into());
        }
        let dest = data.join(&name);
        if entry.is_dir() {
            fs::create_dir_all(dest).map_err(|e| e.to_string())?;
        } else {
            fs::create_dir_all(dest.parent().ok_or("Invalid archive path")?)
                .map_err(|e| e.to_string())?;
            let mut out = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(dest)
                .map_err(|e| e.to_string())?;
            std::io::copy(&mut entry, &mut out).map_err(|e| e.to_string())?;
            out.flush().map_err(|e| e.to_string())?;
        }
    }
    drop(zip);
    for (folder, files) in &mut package.folders {
        *files = inventory(&data.join(folder))?;
    }
    let marker = serde_json::to_string_pretty(&package).map_err(|e| e.to_string())?;
    if marker.len() > 4_000_000
        || marker
            .len()
            .checked_mul(package.folders.len())
            .is_none_or(|n| n > 12_000_000)
    {
        return Err("Package ownership records exceed size limits.".into());
    }
    let mut markers = BTreeMap::new();
    for folder in package.folders.keys() {
        let mut f = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(data.join(folder).join(MARKER))
            .map_err(|e| e.to_string())?;
        f.write_all(marker.as_bytes()).map_err(|e| e.to_string())?;
        f.sync_all().map_err(|e| e.to_string())?;
        markers.insert(folder.clone(), marker.clone());
    }
    Ok((package, markers))
}
fn install(root: &Path, archive: &Path, expected_sha256: &str) -> Result<Vec<String>, String> {
    let _lock = acquire_lock(root)?;
    ensure_no_pending(root)?;
    let tmp = new_transaction(root)?;
    let (package, markers) = stage_archive(tmp.path(), archive, expected_sha256)?;
    let addons = crate::addons_path(root);
    preflight(&addons, &package)?;
    let journal = Transaction {
        version: 1,
        action: Action::Install,
        package: package.clone(),
        markers,
    };
    write_journal(tmp.path(), &journal)?;
    let tx = tmp.keep();
    hooks::checkpoint();
    for folder in package.folders.keys() {
        move_no_replace(&tx.join("data").join(folder), &addons.join(folder))
            .map_err(|e| tx_error(&tx, e))?;
        hooks::checkpoint();
    }
    fs::remove_file(tx.join(JOURNAL)).map_err(|e| tx_error(&tx, e.to_string()))?;
    hooks::checkpoint();
    let _ = fs::remove_dir_all(&tx);
    Ok(package.folders.keys().cloned().collect())
}
fn read_update_journal(tx: &Path) -> Result<UpdateJournal, String> {
    normal_dir(tx)?;
    normal_dir(&tx.join("data"))?;
    normal_dir(&tx.join("old"))?;
    let path = tx.join(UPDATE_JOURNAL);
    let md = fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
    if !md.file_type().is_file() || md.len() > 16_000_000 {
        return Err("Invalid update journal.".into());
    }
    let journal: UpdateJournal =
        serde_json::from_reader(fs::File::open(path).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    validate(&journal.old)?;
    validate(&journal.new)?;
    if journal.version != 1
        || journal.old.folders.keys().ne(journal.new.folders.keys())
        || journal.old_markers.len() != journal.old.folders.len()
        || journal.new_markers.len() != journal.new.folders.len()
    {
        return Err("Invalid update journal.".into());
    }
    for folder in journal.old.folders.keys() {
        if parse_marker(
            journal
                .old_markers
                .get(folder)
                .ok_or("Missing old marker")?,
            folder,
        )? != journal.old
            || parse_marker(
                journal
                    .new_markers
                    .get(folder)
                    .ok_or("Missing new marker")?,
                folder,
            )? != journal.new
        {
            return Err("Update ownership records disagree.".into());
        }
    }
    Ok(journal)
}
fn exists(path: &Path) -> Result<bool, String> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e.to_string()),
    }
}
fn update_recovery_moves(root: &Path, tx: &Path) -> Result<Vec<(PathBuf, PathBuf)>, String> {
    let journal = read_update_journal(tx)?;
    let addons = crate::addons_path(root);
    normal_dir(&addons)?;
    let mut remove_new = Vec::new();
    let mut restore_old = Vec::new();
    for name in journal.old.folders.keys() {
        let game = addons.join(name);
        let old = tx.join("old").join(name);
        let new = tx.join("data").join(name);
        match (exists(&game)?, exists(&old)?, exists(&new)?) {
            (true, false, true) => {
                if verify_folder(&game, name, &journal.old)? != journal.old_markers[name] {
                    return Err(format!("Original {name} changed."));
                }
                if verify_folder(&new, name, &journal.new)? != journal.new_markers[name] {
                    return Err(format!("Replacement {name} changed."));
                }
            }
            (false, true, true) => {
                if verify_folder(&old, name, &journal.old)? != journal.old_markers[name]
                    || verify_folder(&new, name, &journal.new)? != journal.new_markers[name]
                {
                    return Err(format!("Update data for {name} changed."));
                }
                restore_old.push((old, game));
            }
            (true, true, false) => {
                if verify_folder(&old, name, &journal.old)? != journal.old_markers[name]
                    || verify_folder(&game, name, &journal.new)? != journal.new_markers[name]
                {
                    return Err(format!("Update data for {name} changed."));
                }
                remove_new.push((game.clone(), new));
                restore_old.push((old, game));
            }
            _ => {
                return Err(format!(
                    "Cannot safely recover {name}: files are missing or a destination is occupied."
                ));
            }
        }
    }
    Ok(remove_new.into_iter().chain(restore_old).collect())
}
/// Update an existing managed package using an explicitly supplied archive.
fn update(
    root: &Path,
    folder: &str,
    archive: &Path,
    expected_sha256: &str,
    expected_folders: &[String],
    expected_package_digest: &str,
) -> Result<PathBuf, String> {
    let _lock = acquire_lock(root)?;
    ensure_no_pending(root)?;
    if !valid_name(folder) {
        return Err("Provide a valid add-on folder name.".into());
    }
    let addons = crate::addons_path(root);
    normal_dir(&addons)?;
    let old = parse_marker(&read_marker(&addons.join(folder))?, folder)?;
    if package_hash(&old)? != expected_package_digest {
        return Err("Package changed after preview; inspect and confirm again.".into());
    }
    if old
        .folders
        .keys()
        .map(String::as_str)
        .ne(expected_folders.iter().map(String::as_str))
    {
        return Err("Package folders changed after confirmation.".into());
    }
    let mut old_markers = BTreeMap::new();
    for name in old.folders.keys() {
        old_markers.insert(name.clone(), verify_folder(&addons.join(name), name, &old)?);
    }
    let tmp = new_transaction(root)?;
    let (new, new_markers) = stage_archive(tmp.path(), archive, expected_sha256)?;
    if old.folders.keys().ne(new.folders.keys()) {
        return Err("Update archive must contain exactly the existing package folders; added, removed, or renamed folders are not supported yet.".into());
    }
    fs::create_dir(tmp.path().join("old")).map_err(|e| e.to_string())?;
    let journal = UpdateJournal {
        version: 1,
        old,
        new,
        old_markers,
        new_markers,
    };
    let bytes = serde_json::to_vec(&journal).map_err(|e| e.to_string())?;
    if bytes.len() > 16_000_000 {
        return Err("Update journal exceeds size limit.".into());
    }
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(tmp.path().join("update.tmp"))
        .map_err(|e| e.to_string())?;
    file.write_all(&bytes).map_err(|e| e.to_string())?;
    file.sync_all().map_err(|e| e.to_string())?;
    drop(file);
    move_no_replace(
        &tmp.path().join("update.tmp"),
        &tmp.path().join(UPDATE_JOURNAL),
    )?;
    let tx = tmp.keep();
    hooks::checkpoint();
    for name in journal.old.folders.keys() {
        move_no_replace(&addons.join(name), &tx.join("old").join(name))
            .map_err(|e| tx_error(&tx, e))?;
        hooks::checkpoint();
    }
    for name in journal.new.folders.keys() {
        move_no_replace(&tx.join("data").join(name), &addons.join(name))
            .map_err(|e| tx_error(&tx, e))?;
        hooks::checkpoint();
    }
    move_no_replace(&tx.join(UPDATE_JOURNAL), &tx.join("updated.json"))
        .map_err(|e| tx_error(&tx, e))?;
    hooks::checkpoint();
    Ok(tx.join("old"))
}
fn remove(
    root: &Path,
    folder: &str,
    expected_folders: &[String],
    expected_digest: &str,
) -> Result<PathBuf, String> {
    let _lock = acquire_lock(root)?;
    ensure_no_pending(root)?;
    if !valid_name(folder) {
        return Err("Provide a valid single add-on folder name.".into());
    }
    let addons = crate::addons_path(root);
    normal_dir(&addons)?;
    let package = parse_marker(&read_marker(&addons.join(folder))?, folder)?;
    if package_hash(&package)? != expected_digest {
        return Err("Package changed after preview; inspect and confirm again.".into());
    }
    if package
        .folders
        .keys()
        .map(String::as_str)
        .ne(expected_folders.iter().map(String::as_str))
    {
        return Err(
            "Package folders changed after confirmation; inspect and confirm again.".into(),
        );
    }
    let mut markers = BTreeMap::new();
    let mut marker_bytes = 0usize;
    for name in package.folders.keys() {
        let marker = verify_folder(&addons.join(name), name, &package)?;
        marker_bytes = marker_bytes
            .checked_add(marker.len())
            .ok_or("Package ownership records exceed size limits.")?;
        if marker_bytes > 12_000_000 {
            return Err("Package ownership records exceed size limits.".into());
        }
        markers.insert(name.clone(), marker);
    }
    let tmp = new_transaction(root)?;
    fs::create_dir(tmp.path().join("data")).map_err(|e| e.to_string())?;
    write_journal(
        tmp.path(),
        &Transaction {
            version: 1,
            action: Action::Remove,
            package: package.clone(),
            markers,
        },
    )?;
    let tx = tmp.keep();
    hooks::checkpoint();
    for name in package.folders.keys() {
        move_no_replace(&addons.join(name), &tx.join("data").join(name))
            .map_err(|e| tx_error(&tx, e))?;
        hooks::checkpoint();
    }
    move_no_replace(&tx.join(JOURNAL), &tx.join("removed.json")).map_err(|e| tx_error(&tx, e))?;
    hooks::checkpoint();
    Ok(tx.join("data"))
}
pub fn pending(root: &Path) -> Result<Vec<String>, String> {
    let state = root.join(".wowza");
    if !state.exists() {
        return Ok(vec![]);
    }
    normal_dir(&state)?;
    let mut found = Vec::new();
    for e in fs::read_dir(state).map_err(|e| e.to_string())? {
        let e = e.map_err(|e| e.to_string())?;
        let name = e.file_name().to_string_lossy().to_string();
        if name.starts_with("txn-")
            && (e.path().join(JOURNAL).exists() || e.path().join(UPDATE_JOURNAL).exists())
        {
            normal_dir(&e.path())?;
            found.push(name);
        }
    }
    found.sort();
    Ok(found)
}
fn recover_checked(root: &Path, transaction: &str, expected_digest: &str) -> Result<(), String> {
    let _lock = acquire_lock(root)?;
    if plan::journal_digest(root, transaction)? != expected_digest {
        return Err("Recovery journal changed after preview; inspect and confirm again.".into());
    }
    if !transaction.starts_with("txn-") || !valid_name(transaction) {
        return Err("Invalid transaction name.".into());
    }
    let state = root.join(".wowza");
    normal_dir(&state)?;
    let txpath = state.join(transaction);
    let moves = recovery_moves(root, &txpath)?;
    let journal = if txpath.join(UPDATE_JOURNAL).exists() {
        UPDATE_JOURNAL
    } else {
        JOURNAL
    };
    hooks::checkpoint();
    for (from, to) in moves {
        move_no_replace(&from, &to).map_err(|e| tx_error(&txpath, e))?;
        hooks::checkpoint();
    }
    fs::remove_file(txpath.join(journal)).map_err(|e| tx_error(&txpath, e.to_string()))?;
    hooks::checkpoint();
    Ok(())
}

fn recovery_moves(root: &Path, txpath: &Path) -> Result<Vec<(PathBuf, PathBuf)>, String> {
    if txpath.join(UPDATE_JOURNAL).exists() {
        return update_recovery_moves(root, txpath);
    }
    let journal = read_journal(txpath)?;
    let addons = crate::addons_path(root);
    normal_dir(&addons)?;
    // Validate the complete recovery plan before moving anything.
    let mut moves = Vec::new();
    for folder in journal.package.folders.keys() {
        let game = addons.join(folder);
        let saved = txpath.join("data").join(folder);
        let (before, after) = match journal.action {
            Action::Install => (&saved, &game),
            Action::Remove => (&game, &saved),
        };
        let before_exists = exists(before)?;
        let after_exists = exists(after)?;
        match (before_exists, after_exists) {
            (true, false) => {
                if verify_folder(before, folder, &journal.package)? != journal.markers[folder] {
                    return Err(format!("Ownership marker changed for {folder}."));
                }
            }
            (false, true) => {
                if verify_folder(after, folder, &journal.package)? != journal.markers[folder] {
                    return Err(format!("Ownership marker changed for {folder}."));
                }
                moves.push((after.to_path_buf(), before.to_path_buf()));
            }
            _ => {
                return Err(format!(
                    "Cannot safely recover {folder}: destination occupied or both copies missing."
                ));
            }
        }
    }
    Ok(moves)
}
fn package_hash(package: &Package) -> Result<String, String> {
    let bytes = serde_json::to_vec(package).map_err(|e| e.to_string())?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}
#[cfg(all(test, windows))]
fn status(root: &Path, folder: &str) -> Result<Status, String> {
    inspect_installation(root)?
        .packages
        .into_iter()
        .find(|p| p.folders.iter().any(|f| f == folder))
        .map(|p| p.status)
        .ok_or_else(|| "Add-on folder not found.".into())
}
#[cfg(all(test, windows))]
mod tests {
    use super::*;
    fn fixture() -> (tempfile::TempDir, Package, String) {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(crate::addons_path(root.path())).unwrap();
        let files = BTreeMap::from([("main.lua".into(), format!("{:x}", Sha256::digest(b"data")))]);
        let package = Package {
            version: 2,
            folders: BTreeMap::from([("Alpha".into(), files.clone()), ("Beta".into(), files)]),
        };
        let marker = serde_json::to_string(&package).unwrap();
        (root, package, marker)
    }
    fn folder(root: &Path, name: &str, marker: &str) {
        let dir = root.join(name);
        fs::create_dir(&dir).unwrap();
        fs::write(dir.join("main.lua"), "data").unwrap();
        fs::write(dir.join(MARKER), marker).unwrap();
    }
    fn journal(root: &Path, action: Action, package: &Package, marker: &str) -> PathBuf {
        let dir = new_transaction(root).unwrap();
        fs::create_dir(dir.path().join("data")).unwrap();
        let markers = package
            .folders
            .keys()
            .map(|n| (n.clone(), marker.to_owned()))
            .collect();
        write_journal(
            dir.path(),
            &Transaction {
                version: 1,
                action,
                package: package.clone(),
                markers,
            },
        )
        .unwrap();
        dir.keep()
    }
    #[test]
    fn recover_interrupted_install_without_overwriting() {
        let (root, package, marker) = fixture();
        let addons = crate::addons_path(root.path());
        let tx = journal(root.path(), Action::Install, &package, &marker);
        folder(&addons, "Alpha", &marker);
        folder(&tx.join("data"), "Beta", &marker);
        let name = tx.file_name().unwrap().to_str().unwrap();
        assert_eq!(status(root.path(), "Alpha").unwrap(), Status::Incomplete);
        assert!(ensure_no_pending(root.path()).is_err());
        fs::create_dir(tx.join("data").join("Alpha")).unwrap();
        assert!(
            prepare_recovery(root.path(), Some(name))
                .and_then(execute)
                .is_err()
        );
        assert!(addons.join("Alpha").exists());
        fs::remove_dir(tx.join("data").join("Alpha")).unwrap();
        execute(prepare_recovery(root.path(), Some(name)).unwrap()).unwrap();
        assert!(!addons.join("Alpha").exists());
        assert!(!addons.join("Beta").exists());
        assert!(tx.join("data/Alpha/main.lua").exists());
        assert!(pending(root.path()).unwrap().is_empty());
    }
    #[test]
    fn recover_interrupted_removal_preserves_both_folders() {
        let (root, package, marker) = fixture();
        let addons = crate::addons_path(root.path());
        let tx = journal(root.path(), Action::Remove, &package, &marker);
        folder(&tx.join("data"), "Alpha", &marker);
        folder(&addons, "Beta", &marker);
        let name = tx.file_name().unwrap().to_str().unwrap();
        execute(prepare_recovery(root.path(), Some(name)).unwrap()).unwrap();
        assert_eq!(status(root.path(), "Alpha").unwrap(), Status::Managed);
        assert_eq!(status(root.path(), "Beta").unwrap(), Status::Managed);
        fs::write(addons.join("Beta/main.lua"), "changed").unwrap();
        assert_eq!(status(root.path(), "Alpha").unwrap(), Status::Modified);
        assert!(
            remove(
                root.path(),
                "Alpha",
                &["Alpha".into(), "Beta".into()],
                &package_hash(&package).unwrap()
            )
            .is_err()
        );
        assert!(addons.join("Alpha").exists());
        assert!(addons.join("Beta").exists());
    }
    #[test]
    fn lock_excludes_other_operations_and_releases_on_drop() {
        let (root, _, _) = fixture();
        let held = acquire_lock(root.path()).unwrap();
        assert!(acquire_lock(root.path()).is_err());
        assert!(
            recover_checked(root.path(), "txn-test", "")
                .unwrap_err()
                .contains("lock")
        );
        drop(held);
        assert!(acquire_lock(root.path()).is_ok());
    }
    #[test]
    fn refuses_removal_if_confirmed_folder_list_changed() {
        let (root, package, marker) = fixture();
        let addons = crate::addons_path(root.path());
        folder(&addons, "Alpha", &marker);
        folder(&addons, "Beta", &marker);
        assert_eq!(
            prepare_remove(root.path(), "Alpha")
                .unwrap()
                .preview()
                .folders,
            vec!["Alpha", "Beta"]
        );
        assert!(
            remove(
                root.path(),
                "Alpha",
                &["Alpha".into()],
                &package_hash(&package).unwrap()
            )
            .is_err()
        );
        assert!(addons.join("Alpha").exists());
        assert!(addons.join("Beta").exists());
        assert_eq!(package.folders.len(), 2);
    }
}

#[cfg(all(test, windows))]
mod update_tests {
    use super::*;
    fn hash(s: &str) -> String {
        format!("{:x}", Sha256::digest(s.as_bytes()))
    }
    fn write_folder(base: &Path, name: &str, contents: &str, marker: &str) {
        let path = base.join(name);
        fs::create_dir_all(&path).unwrap();
        fs::write(path.join("file"), contents).unwrap();
        fs::write(path.join(MARKER), marker).unwrap();
    }
    #[test]
    fn recovers_every_two_folder_interruption_state() {
        for a in 0..3 {
            for b in 0..3 {
                let root = tempfile::tempdir().unwrap();
                let addons = crate::addons_path(root.path());
                fs::create_dir_all(&addons).unwrap();
                let old = Package {
                    version: 2,
                    folders: ["A", "B"]
                        .into_iter()
                        .map(|s| (s.into(), BTreeMap::from([("file".into(), hash("old"))])))
                        .collect(),
                };
                let new = Package {
                    version: 2,
                    folders: ["A", "B"]
                        .into_iter()
                        .map(|s| (s.into(), BTreeMap::from([("file".into(), hash("new"))])))
                        .collect(),
                };
                let om = serde_json::to_string(&old).unwrap();
                let nm = serde_json::to_string(&new).unwrap();
                let tx = new_transaction(root.path()).unwrap().keep();
                fs::create_dir(tx.join("old")).unwrap();
                fs::create_dir(tx.join("data")).unwrap();
                for (name, state) in [("A", a), ("B", b)] {
                    match state {
                        0 => {
                            write_folder(&addons, name, "old", &om);
                            write_folder(&tx.join("data"), name, "new", &nm);
                        }
                        1 => {
                            write_folder(&tx.join("old"), name, "old", &om);
                            write_folder(&tx.join("data"), name, "new", &nm);
                        }
                        2 => {
                            write_folder(&tx.join("old"), name, "old", &om);
                            write_folder(&addons, name, "new", &nm);
                        }
                        _ => unreachable!(),
                    }
                }
                let j = UpdateJournal {
                    version: 1,
                    old,
                    new,
                    old_markers: ["A", "B"]
                        .into_iter()
                        .map(|n| (n.into(), om.clone()))
                        .collect(),
                    new_markers: ["A", "B"]
                        .into_iter()
                        .map(|n| (n.into(), nm.clone()))
                        .collect(),
                };
                fs::write(tx.join(UPDATE_JOURNAL), serde_json::to_vec(&j).unwrap()).unwrap();
                assert_eq!(pending(root.path()).unwrap().len(), 1);
                let id = tx.file_name().unwrap().to_str().unwrap();
                if a == 2 && b == 2 {
                    fs::write(tx.join("old/B/file"), "changed").unwrap();
                    assert!(
                        prepare_recovery(root.path(), Some(id))
                            .and_then(execute)
                            .is_err()
                    );
                    assert_eq!(fs::read_to_string(addons.join("A/file")).unwrap(), "new");
                    assert_eq!(fs::read_to_string(addons.join("B/file")).unwrap(), "new");
                    fs::write(tx.join("old/B/file"), "old").unwrap();
                }
                execute(prepare_recovery(root.path(), Some(id)).unwrap()).unwrap();
                for name in ["A", "B"] {
                    assert_eq!(
                        fs::read_to_string(addons.join(name).join("file")).unwrap(),
                        "old"
                    );
                }
                assert!(pending(root.path()).unwrap().is_empty());
            }
        }
    }
}
