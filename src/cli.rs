use clap::{Parser, Subcommand};
use std::{io, path::PathBuf};
use wowza::{config, manage};

#[derive(Parser)]
#[command(
    version,
    about = "Inspect and safely manage WoW Forever add-on packages"
)]
struct Cli {
    /// Forever installation root; overrides environment and saved configuration
    #[arg(long, global = true)]
    path: Option<PathBuf>,
    /// Emit structured output (mutations require --yes or --dry-run)
    #[arg(long, global = true)]
    json: bool,
    /// Preview a package change without writing or prompting
    #[arg(long, global = true)]
    dry_run: bool,
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Show packages, problems, and the next safe action (the default command)
    Status {
        #[arg(long, short)]
        verbose: bool,
    },
    /// Remember a validated Forever installation for future commands
    Configure { installation: PathBuf },
    /// Inspect each local add-on folder and its TOC metadata
    Scan,
    /// List local add-on folders and their titles and versions
    List,
    /// Diagnose installation metadata and interrupted operations
    Doctor,
    /// Inspect a downloaded ZIP and check local folder conflicts
    #[command(visible_alias = "inspect")]
    InspectArchive { archive: PathBuf },
    /// Install a locally supplied ZIP after preview and confirmation (Windows)
    #[command(visible_alias = "install")]
    InstallArchive {
        archive: PathBuf,
        #[arg(long, short = 'y')]
        yes: bool,
    },
    /// Update a managed package from a ZIP for the same project and Forever release
    #[command(visible_alias = "update")]
    UpdateArchive {
        folder: String,
        archive: PathBuf,
        #[arg(long, short = 'y')]
        yes: bool,
    },
    /// Remove an unchanged managed package, retaining its files for recovery
    Remove {
        folder: String,
        #[arg(long, short = 'y')]
        yes: bool,
    },
    /// Show interrupted operations with installation-specific recovery commands
    Pending,
    /// Restore an interrupted operation; omit its ID when exactly one is pending
    Recover {
        transaction: Option<String>,
        #[arg(long, short = 'y')]
        yes: bool,
    },
    /// Search CurseForge in a browser with the Forever filter
    Search {
        query: String,
        #[arg(long, short = 'y')]
        yes: bool,
    },
    /// Open a CurseForge project or release page after confirmation
    Continue {
        url: String,
        #[arg(long, short = 'y')]
        yes: bool,
    },
}

fn json(value: &impl serde::Serialize) -> Result<(), String> {
    println!(
        "{}",
        serde_json::to_string_pretty(value).map_err(|e| e.to_string())?
    );
    Ok(())
}

fn apply(
    plan: manage::OperationPlan,
    yes: bool,
    dry_run: bool,
    structured: bool,
) -> Result<(), String> {
    if structured && !dry_run && !yes {
        return Err(
            "JSON execution requires --yes; use --dry-run --json for a read-only preview.".into(),
        );
    }
    let p = plan.preview();
    if structured && dry_run {
        return json(p);
    }
    if !structured {
        println!(
            "Installation: {}\nAction: {}\nPackage folders: {}",
            p.installation.display(),
            p.action,
            p.folders.join(", ")
        );
        if let Some(archive) = &p.archive {
            println!("Archive: {}", archive.display());
        }
        if let Some(digest) = &p.sha256 {
            println!("SHA-256: {digest}");
        }
        if let Some(tx) = &p.transaction {
            println!("Transaction: {tx}");
        }
        println!("{}", p.notice);
        if dry_run {
            println!("Preview only: nothing changed.");
            return Ok(());
        }
    }
    if !wowza::interaction::confirm(
        &mut io::stdin().lock(),
        &mut io::stdout(),
        "Apply this change?",
        yes,
    )? {
        println!("Cancelled.");
        return Ok(());
    }
    let root = p.installation.clone();
    let result = manage::execute(plan)?;
    if structured {
        return json(&result);
    }
    let verb = match result.action {
        "install" => "Installed",
        "update" => "Updated",
        "remove" => "Removed",
        _ => "Recovered",
    };
    println!("{verb} {}.", result.folders.join(", "));
    if let Some(path) = result.retained {
        println!("Original files retained at {}.", path.display());
    }
    println!(
        "Inspect the result: {}",
        config::command(&root, &["status"])
    );
    Ok(())
}

fn status_label(status: manage::Status) -> &'static str {
    match status {
        manage::Status::Unmanaged => "unmanaged",
        manage::Status::Managed => "managed",
        manage::Status::Modified => "managed, modified",
        manage::Status::Incomplete => "managed, incomplete or invalid record",
    }
}

fn metadata(addon: &wowza::Addon, detailed: bool) {
    if addon.tocs.is_empty() {
        println!("  No TOC found");
    }
    for toc in &addon.tocs {
        println!("  {}", toc.file);
        let keys: &[&str] = if detailed {
            &["title", "version", "interface", "author", "notes"]
        } else {
            &["title", "version"]
        };
        for key in keys {
            if let Some(value) = toc.metadata.get(*key) {
                println!("    {key}: {value}");
            }
        }
    }
}

fn show_inspection(report: &manage::Inspection, command: &Command) {
    println!(
        "Installation: {}\nAddOns: {}",
        report.installation.display(),
        report.folder_count
    );
    match command {
        Command::List | Command::Scan => {
            let mut rows: Vec<_> = report
                .packages
                .iter()
                .flat_map(|p| p.addons.iter().map(move |a| (a, p.status)))
                .collect();
            rows.sort_by_key(|(a, _)| a.folder.to_lowercase());
            for (addon, status) in rows {
                println!("\n{} [{}]", addon.folder, status_label(status));
                metadata(addon, matches!(command, Command::Scan));
            }
        }
        Command::Doctor => {
            let missing = report
                .packages
                .iter()
                .flat_map(|p| &p.addons)
                .filter(|a| a.tocs.is_empty())
                .count();
            println!("AddOns folder is readable. {missing} folder(s) have no readable TOC file.");
            for package in &report.packages {
                for issue in &package.issues {
                    println!("  {issue}");
                }
            }
        }
        _ => {
            println!("Packages: {}", report.packages.len());
            for package in &report.packages {
                println!(
                    "\n{} [{}]",
                    package.folders.join(" + "),
                    status_label(package.status)
                );
                for issue in &package.issues {
                    println!("  {issue}");
                }
                for addon in &package.addons {
                    if matches!(command, Command::Status { verbose: true }) {
                        metadata(addon, true);
                    } else if let Some(toc) = addon.tocs.first() {
                        if let Some(title) = toc.metadata.get("title") {
                            println!(
                                "  {title} {}",
                                toc.metadata
                                    .get("version")
                                    .map(String::as_str)
                                    .unwrap_or("")
                            );
                        }
                        break;
                    }
                }
            }
        }
    }
    for operation in &report.pending {
        println!(
            "Interrupted {}: {} ({})",
            operation.action,
            operation.id,
            operation.folders.join(", ")
        );
        if let Some(issue) = &operation.issue {
            println!("  Cannot validate recovery data: {issue}");
        }
    }
    for action in &report.next_actions {
        println!("Next: {action}");
    }
    println!("\nLocal metadata does not establish project identity or Forever compatibility.");
}

fn run(cli: Cli) -> Result<(), String> {
    let command = cli.command.unwrap_or(Command::Status { verbose: false });
    match command {
        Command::Search { query, yes } => {
            if cli.json || cli.dry_run {
                return Err("Search opens a browser; --json and --dry-run apply to local inspection and package changes.".into());
            }
            wowza::interaction::handoff(
                &mut io::stdin().lock(),
                &mut io::stdout(),
                &wowza::search_url(&query)?,
                yes,
                true,
                |url| open::that(url).map_err(|e| e.to_string()),
            )
        }
        Command::Continue { url, yes } => {
            if cli.json || cli.dry_run {
                return Err("Continue opens a browser; --json and --dry-run apply to local inspection and package changes.".into());
            }
            wowza::interaction::handoff(
                &mut io::stdin().lock(),
                &mut io::stdout(),
                &wowza::project_url(&url)?,
                yes,
                false,
                |url| open::that(url).map_err(|e| e.to_string()),
            )
        }
        Command::Configure { installation } => {
            if cli.path.is_some() {
                return Err(
                    "Configure takes the installation as its argument; omit --path.".into(),
                );
            }
            let path = if cli.dry_run {
                config::validate(&installation)?
            } else {
                config::save(&installation)?
            };
            if cli.json {
                json(&serde_json::json!({"installation": path, "saved": !cli.dry_run}))
            } else {
                println!(
                    "{} installation: {}",
                    if cli.dry_run { "Validated" } else { "Saved" },
                    path.display()
                );
                Ok(())
            }
        }
        command => {
            let root = config::resolve(cli.path)?;
            match command {
                Command::InstallArchive { archive, yes } => apply(
                    manage::prepare_install(&root, &archive)?,
                    yes,
                    cli.dry_run,
                    cli.json,
                ),
                Command::UpdateArchive {
                    folder,
                    archive,
                    yes,
                } => apply(
                    manage::prepare_update(&root, &folder, &archive)?,
                    yes,
                    cli.dry_run,
                    cli.json,
                ),
                Command::Remove { folder, yes } => apply(
                    manage::prepare_remove(&root, &folder)?,
                    yes,
                    cli.dry_run,
                    cli.json,
                ),
                Command::Recover { transaction, yes } => apply(
                    manage::prepare_recovery(&root, transaction.as_deref())?,
                    yes,
                    cli.dry_run,
                    cli.json,
                ),
                Command::Pending => {
                    let pending = manage::pending_operations(&root)?;
                    if cli.json {
                        return json(&pending);
                    }
                    if pending.is_empty() {
                        println!("No interrupted Wowza operations found.");
                    }
                    for p in pending {
                        println!(
                            "{}: {} {}\n  {}",
                            p.id,
                            p.action,
                            p.folders.join(", "),
                            p.recovery_command
                        );
                        if let Some(issue) = p.issue {
                            println!("  Cannot validate recovery data: {issue}");
                        }
                    }
                    Ok(())
                }
                Command::InspectArchive { archive } => {
                    let preview = wowza::archive::inspect(&archive)?;
                    let existing: std::collections::HashSet<_> =
                        std::fs::read_dir(wowza::addons_path(&root))
                            .map_err(|e| e.to_string())?
                            .map(|e| e.map(|e| e.file_name().to_string_lossy().to_lowercase()))
                            .collect::<Result<_, _>>()
                            .map_err(|e| e.to_string())?;
                    let conflicts: Vec<_> = preview
                        .folders
                        .keys()
                        .filter(|f| existing.contains(&f.to_lowercase()))
                        .collect();
                    if cli.json {
                        json(
                            &serde_json::json!({"archive": archive, "installation": root, "folders": preview.folders, "uncompressed_bytes": preview.bytes, "conflicts": conflicts}),
                        )?;
                    } else {
                        println!(
                            "Archive: {}\nUncompressed bytes: {}",
                            archive.display(),
                            preview.bytes
                        );
                        for (folder, count) in &preview.folders {
                            println!(
                                "  {folder}: {count} files{}",
                                if existing.contains(&folder.to_lowercase()) {
                                    " [CONFLICT: already exists]"
                                } else {
                                    " [no existing folder]"
                                }
                            );
                        }
                        println!(
                            "Preview only: nothing installed. Verify the release is intended for Forever."
                        );
                    }
                    if !conflicts.is_empty() {
                        return Err("Archive conflicts with existing AddOns entries.".into());
                    }
                    Ok(())
                }
                Command::Status { .. } | Command::List | Command::Scan | Command::Doctor => {
                    let report = manage::inspect_installation(&root)?;
                    if cli.json {
                        json(&report)
                    } else {
                        show_inspection(&report, &command);
                        Ok(())
                    }
                }
                _ => unreachable!("handled before installation resolution"),
            }
        }
    }
}

pub fn main() {
    if let Err(error) = run(Cli::parse()) {
        eprintln!("Error: {error}");
        std::process::exit(1);
    }
}
