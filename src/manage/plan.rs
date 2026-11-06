use super::*;

#[derive(Debug, Serialize)]
pub struct OperationPreview {
    pub installation: PathBuf,
    pub action: &'static str,
    pub folders: Vec<String>,
    pub archive: Option<PathBuf>,
    pub sha256: Option<String>,
    pub transaction: Option<String>,
    pub notice: &'static str,
}

#[derive(Debug)]
enum Operation {
    Install,
    Update {
        folder: String,
        package_digest: String,
    },
    Remove {
        folder: String,
        package_digest: String,
    },
    Recover {
        journal_digest: String,
    },
}

/// A read-only preview bound to the exact installation and inputs to execute.
/// Execution revalidates these preconditions while holding the installation lock.
#[derive(Debug)]
pub struct OperationPlan {
    preview: OperationPreview,
    operation: Operation,
}

impl OperationPlan {
    pub fn preview(&self) -> &OperationPreview {
        &self.preview
    }
}

#[derive(Debug, Serialize)]
pub struct OperationResult {
    pub installation: PathBuf,
    pub action: &'static str,
    pub folders: Vec<String>,
    pub retained: Option<PathBuf>,
    pub transaction: Option<String>,
}

fn verified_package(root: &Path, folder: &str) -> Result<Package, String> {
    if !valid_name(folder) {
        return Err("Provide a valid add-on folder name.".into());
    }
    ensure_no_pending(root)?;
    let addons = crate::addons_path(root);
    normal_dir(&addons)?;
    let package = parse_marker(&read_marker(&addons.join(folder))?, folder)?;
    for name in package.folders.keys() {
        verify_folder(&addons.join(name), name, &package)?;
    }
    Ok(package)
}

fn archive_preview(
    root: &Path,
    archive: &Path,
    action: &'static str,
) -> Result<OperationPreview, String> {
    let installation = crate::config::validate(root)?;
    ensure_no_pending(&installation)?;
    let archive = fs::canonicalize(archive).map_err(|e| format!("Cannot open archive: {e}"))?;
    let sha256 = archive_digest(&archive)?;
    let preview = crate::archive::inspect(&archive)?;
    if archive_digest(&archive)? != sha256 {
        return Err("Archive changed during preview; try again.".into());
    }
    Ok(OperationPreview {
        installation,
        action,
        folders: preview.folders.into_keys().collect(),
        archive: Some(archive),
        sha256: Some(sha256),
        transaction: None,
        notice: "Verify the release is intended for Forever; local metadata cannot establish compatibility.",
    })
}

pub fn prepare_install(root: &Path, archive: &Path) -> Result<OperationPlan, String> {
    let preview = archive_preview(root, archive, "install")?;
    let package = Package {
        version: 2,
        folders: preview
            .folders
            .iter()
            .map(|f| (f.clone(), Files::new()))
            .collect(),
    };
    preflight(&crate::addons_path(&preview.installation), &package)?;
    Ok(OperationPlan {
        preview,
        operation: Operation::Install,
    })
}

pub fn prepare_update(root: &Path, folder: &str, archive: &Path) -> Result<OperationPlan, String> {
    let mut preview = archive_preview(root, archive, "update")?;
    preview.notice = "Verify this is the same project and intended for Forever. Original files will be retained under .wowza; saved settings in WTF are preserved.";
    let package = verified_package(&preview.installation, folder)?;
    if package.folders.keys().ne(preview.folders.iter()) {
        return Err("Update archive must contain exactly the existing package folders; added, removed, or renamed folders are not supported yet.".into());
    }
    let operation = Operation::Update {
        folder: folder.into(),
        package_digest: package_hash(&package)?,
    };
    Ok(OperationPlan { preview, operation })
}

pub fn prepare_remove(root: &Path, folder: &str) -> Result<OperationPlan, String> {
    let installation = crate::config::validate(root)?;
    let package = verified_package(&installation, folder)?;
    let operation = Operation::Remove {
        folder: folder.into(),
        package_digest: package_hash(&package)?,
    };
    let preview = OperationPreview {
        installation,
        action: "remove",
        folders: package.folders.into_keys().collect(),
        archive: None,
        sha256: None,
        transaction: None,
        notice: "Saved settings in WTF are preserved. Original files are retained under .wowza.",
    };
    Ok(OperationPlan { preview, operation })
}

pub(super) fn journal_digest(root: &Path, transaction: &str) -> Result<String, String> {
    if !transaction.starts_with("txn-") || !valid_name(transaction) {
        return Err("Invalid transaction name.".into());
    }
    let tx = root.join(".wowza").join(transaction);
    normal_dir(&tx)?;
    let filename = if tx.join(UPDATE_JOURNAL).exists() {
        UPDATE_JOURNAL
    } else {
        JOURNAL
    };
    let path = tx.join(filename);
    let md = fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
    if !md.is_file() || md.file_type().is_symlink() || md.len() > 16_000_000 {
        return Err("Invalid transaction journal.".into());
    }
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(16_000_001)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 16_000_000 {
        return Err("Transaction journal exceeds size limit.".into());
    }
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

pub fn prepare_recovery(root: &Path, transaction: Option<&str>) -> Result<OperationPlan, String> {
    let installation = crate::config::validate(root)?;
    let transactions = pending(&installation)?;
    let selected = match transaction {
        Some(id) if transactions.iter().any(|t| t == id) => id.to_string(),
        Some(_) => return Err("No such pending operation in this installation.".into()),
        None if transactions.is_empty() => {
            return Err("No interrupted Wowza operations found.".into());
        }
        None if transactions.len() == 1 => transactions[0].clone(),
        None => {
            return Err(format!(
                "Multiple interrupted operations found. Choose one using: {}",
                crate::config::command(&installation, &["pending"])
            ));
        }
    };
    let digest = journal_digest(&installation, &selected)?;
    let tx = installation.join(".wowza").join(&selected);
    let folders = if tx.join(UPDATE_JOURNAL).exists() {
        read_update_journal(&tx)?.old.folders.into_keys().collect()
    } else {
        read_journal(&tx)?.package.folders.into_keys().collect()
    };
    recovery_moves(&installation, &tx)?;
    if journal_digest(&installation, &selected)? != digest {
        return Err("Recovery journal changed during preview; try again.".into());
    }
    Ok(OperationPlan {
        preview: OperationPreview {
            installation,
            action: "recover",
            folders,
            archive: None,
            sha256: None,
            transaction: Some(selected),
            notice: "Recovery restores the state before this operation and refuses conflicting or changed files.",
        },
        operation: Operation::Recover {
            journal_digest: digest,
        },
    })
}

pub fn execute(plan: OperationPlan) -> Result<OperationResult, String> {
    let p = plan.preview;
    let root = &p.installation;
    if crate::config::validate(root)? != *root {
        return Err("Installation changed after preview; inspect and confirm again.".into());
    }
    let retained = match plan.operation {
        Operation::Install => {
            install(
                root,
                p.archive.as_ref().unwrap(),
                p.sha256.as_ref().unwrap(),
            )?;
            None
        }
        Operation::Update {
            folder,
            package_digest,
        } => Some(update(
            root,
            &folder,
            p.archive.as_ref().unwrap(),
            p.sha256.as_ref().unwrap(),
            &p.folders,
            &package_digest,
        )?),
        Operation::Remove {
            folder,
            package_digest,
        } => Some(remove(root, &folder, &p.folders, &package_digest)?),
        Operation::Recover { journal_digest } => {
            recover_checked(root, p.transaction.as_ref().unwrap(), &journal_digest)?;
            None
        }
    };
    Ok(OperationResult {
        installation: p.installation,
        action: p.action,
        folders: p.folders,
        retained,
        transaction: p.transaction,
    })
}
