use super::*;

#[derive(Debug, Serialize)]
pub struct PendingOperation {
    pub id: String,
    pub action: String,
    pub folders: Vec<String>,
    pub recovery_command: String,
    pub issue: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PackageSummary {
    pub folders: Vec<String>,
    pub status: Status,
    pub addons: Vec<crate::Addon>,
    pub issues: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct Inspection {
    pub installation: PathBuf,
    pub folder_count: usize,
    pub packages: Vec<PackageSummary>,
    pub pending: Vec<PendingOperation>,
    pub next_actions: Vec<String>,
}

pub fn pending_operations(root: &Path) -> Result<Vec<PendingOperation>, String> {
    pending(root)?
        .into_iter()
        .map(|id| {
            let tx = root.join(".wowza").join(&id);
            let details = if tx.join(UPDATE_JOURNAL).exists() {
                read_update_journal(&tx).map(|j| ("update", j.old.folders.into_keys().collect()))
            } else {
                read_journal(&tx).map(|j| {
                    (
                        if j.action == Action::Install {
                            "install"
                        } else {
                            "remove"
                        },
                        j.package.folders.into_keys().collect(),
                    )
                })
            };
            let (action, folders, issue) = match details {
                Ok((action, folders)) => (action.into(), folders, None),
                Err(e) => ("unknown".into(), vec![], Some(e)),
            };
            let recovery_command = crate::config::command(root, &["recover", &id]);
            Ok(PendingOperation {
                id,
                action,
                folders,
                recovery_command,
                issue,
            })
        })
        .collect()
}

pub fn inspect_installation(root: &Path) -> Result<Inspection, String> {
    let installation = crate::config::validate(root)?;
    let addons = crate::scan(&installation).map_err(|e| format!("Cannot read AddOns: {e}"))?;
    let base = crate::addons_path(&installation);
    let pending = pending_operations(&installation)?;
    let records: BTreeMap<_, _> = addons
        .iter()
        .map(|a| {
            let path = base.join(&a.folder);
            let record = match fs::symlink_metadata(path.join(MARKER)) {
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
                Err(e) => Err(e.to_string()),
                Ok(_) => read_marker(&path)
                    .and_then(|s| parse_marker(&s, &a.folder))
                    .map(Some),
            };
            (a.folder.clone(), record)
        })
        .collect();
    let mut seen = HashSet::new();
    let mut packages = Vec::new();
    for addon in &addons {
        if seen.contains(&addon.folder) {
            continue;
        }
        let mut issues = Vec::new();
        let (folders, status) = match &records[&addon.folder] {
            Ok(None) => (vec![addon.folder.clone()], Status::Unmanaged),
            Err(e) => {
                issues.push(format!("Invalid ownership record: {e}"));
                (vec![addon.folder.clone()], Status::Modified)
            }
            Ok(Some(package)) => {
                let agrees = package.folders.keys().all(|f| match records.get(f) {
                    Some(Ok(Some(other))) => other == package,
                    None => !base.join(f).exists(),
                    _ => false,
                });
                if !agrees {
                    issues.push("Ownership records disagree; folders are shown separately. Inspect the records before making changes.".into());
                    (vec![addon.folder.clone()], Status::Incomplete)
                } else {
                    let folders: Vec<_> = package.folders.keys().cloned().collect();
                    let mut status = Status::Managed;
                    for f in &folders {
                        if !records.contains_key(f) {
                            status = Status::Incomplete;
                            issues.push(format!("Missing folder: {f}"));
                        } else if inventory(&base.join(f)).as_ref() != Ok(&package.folders[f]) {
                            if status != Status::Incomplete {
                                status = Status::Modified;
                            }
                            issues.push(format!("{f}: managed package contents changed; update and removal are refused. Restore the original files before retrying."));
                        }
                    }
                    if pending
                        .iter()
                        .any(|p| p.folders.iter().any(|f| folders.contains(f)))
                    {
                        status = Status::Incomplete;
                        issues.push("An interrupted operation must be recovered first.".into());
                    }
                    (folders, status)
                }
            }
        };
        let members: Vec<_> = addons
            .iter()
            .filter(|a| folders.contains(&a.folder))
            .cloned()
            .collect();
        for a in &members {
            seen.insert(a.folder.clone());
            if a.tocs.is_empty() {
                issues.push(format!(
                    "{}: no TOC; check whether this folder is an add-on or supporting data.",
                    a.folder
                ));
            }
        }
        packages.push(PackageSummary {
            folders,
            status,
            addons: members,
            issues,
        });
    }
    let mut next_actions: Vec<_> = pending.iter().map(|p| p.recovery_command.clone()).collect();
    if next_actions.is_empty() && packages.iter().any(|p| !p.issues.is_empty()) {
        next_actions.push("Resolve the reported file or ownership problems before updating or removing affected packages.".into());
    }
    Ok(Inspection {
        installation,
        folder_count: addons.len(),
        packages,
        pending,
        next_actions,
    })
}
