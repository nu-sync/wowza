use anstream::{eprintln, println};
use clap::{
    Parser, Subcommand,
    builder::styling::{AnsiColor, Style, Styles},
};
use std::{io, path::PathBuf};
use wowza::{config, manage};

const HEADING: Style = AnsiColor::Cyan.on_default().bold();
const ACCENT: Style = AnsiColor::Magenta.on_default().bold();
const SUCCESS: Style = AnsiColor::Green.on_default();
const WARNING: Style = AnsiColor::Yellow.on_default();
const ERROR: Style = AnsiColor::Red.on_default().bold();
const MUTED: Style = Style::new().dimmed();

const HELP_STYLES: Styles = Styles::styled()
    .header(HEADING)
    .usage(HEADING)
    .literal(ACCENT)
    .placeholder(AnsiColor::Cyan.on_default())
    .valid(SUCCESS)
    .invalid(WARNING)
    .error(ERROR);

#[derive(Parser)]
#[command(
    version,
    styles = HELP_STYLES,
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
    /// Show packages, problems, and the next safe action
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
    /// Search CurseForge with the Forever filter, or return its URL
    Search {
        query: String,
        /// Print only the URL without prompting or opening a browser
        #[arg(long = "url")]
        url_only: bool,
        #[arg(long, short = 'y')]
        yes: bool,
    },
    /// Open a CurseForge project or release page, or return its validated URL
    Continue {
        url: String,
        /// Print only the URL without prompting or opening a browser
        #[arg(long = "url")]
        url_only: bool,
        #[arg(long, short = 'y')]
        yes: bool,
    },
}

fn json(value: &impl serde::Serialize) -> Result<(), String> {
    std::println!(
        "{}",
        serde_json::to_string_pretty(value).map_err(|e| e.to_string())?
    );
    Ok(())
}

fn print_handoff(
    url: &url::Url,
    action: &str,
    query: Option<&str>,
    structured: bool,
) -> Result<(), String> {
    if structured {
        let mut handoff = serde_json::json!({
            "kind": "url_handoff",
            "action": action,
            "url": url.as_str(),
            "content_fetched": false,
        });
        if let Some(query) = query {
            handoff["query"] = query.trim().into();
        }
        json(&handoff)
    } else {
        std::println!("{url}");
        Ok(())
    }
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
            "{HEADING}Installation:{HEADING:#} {}\n{HEADING}Action:{HEADING:#} {ACCENT}{}{ACCENT:#}\n{HEADING}Package folders:{HEADING:#} {ACCENT}{}{ACCENT:#}",
            p.installation.display(),
            p.action,
            p.folders.join(", ")
        );
        if let Some(archive) = &p.archive {
            println!("{HEADING}Archive:{HEADING:#} {}", archive.display());
        }
        if let Some(digest) = &p.sha256 {
            println!("{MUTED}SHA-256: {digest}{MUTED:#}");
        }
        if let Some(tx) = &p.transaction {
            println!("{HEADING}Transaction:{HEADING:#} {tx}");
        }
        println!("{WARNING}{}{WARNING:#}", p.notice);
        if dry_run {
            println!("{HEADING}Preview only:{HEADING:#} nothing changed.");
            return Ok(());
        }
    }
    if !wowza::interaction::confirm(
        &mut io::stdin().lock(),
        &mut anstream::stdout(),
        &format!("{HEADING}Apply this change?{HEADING:#}"),
        yes,
    )? {
        println!("{MUTED}Cancelled.{MUTED:#}");
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
    println!("{SUCCESS}{verb} {}.{SUCCESS:#}", result.folders.join(", "));
    if let Some(path) = result.retained {
        println!(
            "{HEADING}Original files retained at{HEADING:#} {}.",
            path.display()
        );
    }
    println!(
        "{HEADING}Inspect the result:{HEADING:#} {ACCENT}{}{ACCENT:#}",
        config::command(&root, &["status"])
    );
    Ok(())
}

fn status_label(status: manage::Status) -> String {
    let (style, label) = match status {
        manage::Status::Unmanaged => (MUTED, "unmanaged"),
        manage::Status::Managed => (SUCCESS, "managed"),
        manage::Status::Modified => (WARNING, "managed, modified"),
        manage::Status::Incomplete => (ERROR, "managed, incomplete or invalid record"),
    };
    format!("{style}{label}{style:#}")
}

fn metadata(addon: &wowza::Addon, detailed: bool) {
    if addon.tocs.is_empty() {
        println!("  {WARNING}No TOC found{WARNING:#}");
    }
    for toc in &addon.tocs {
        println!("  {MUTED}{}{MUTED:#}", toc.file);
        let keys: &[&str] = if detailed {
            &["title", "version", "interface", "author", "notes"]
        } else {
            &["title", "version"]
        };
        for key in keys {
            if let Some(value) = toc.metadata.get(*key) {
                println!("    {HEADING}{key}:{HEADING:#} {value}");
            }
        }
    }
}

fn show_inspection(report: &manage::Inspection, command: &Command) {
    println!(
        "{HEADING}Installation:{HEADING:#} {}\n{HEADING}AddOns:{HEADING:#} {}",
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
                println!(
                    "\n{ACCENT}{}{ACCENT:#} [{}]",
                    addon.folder,
                    status_label(status)
                );
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
            let health = if missing == 0 { SUCCESS } else { WARNING };
            println!(
                "{SUCCESS}AddOns folder is readable.{SUCCESS:#} {health}{missing} folder(s) have no readable TOC file.{health:#}"
            );
            for package in &report.packages {
                for issue in &package.issues {
                    println!("  {WARNING}{issue}{WARNING:#}");
                }
            }
        }
        _ => {
            println!("{HEADING}Packages:{HEADING:#} {}", report.packages.len());
            for package in &report.packages {
                println!(
                    "\n{ACCENT}{}{ACCENT:#} [{}]",
                    package.folders.join(" + "),
                    status_label(package.status)
                );
                for issue in &package.issues {
                    println!("  {WARNING}{issue}{WARNING:#}");
                }
                for addon in &package.addons {
                    if matches!(command, Command::Status { verbose: true }) {
                        metadata(addon, true);
                    } else if let Some(toc) = addon.tocs.first() {
                        if let Some(title) = toc.metadata.get("title") {
                            println!(
                                "  {title} {MUTED}{}{MUTED:#}",
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
            "{WARNING}Interrupted {}: {} ({}){WARNING:#}",
            operation.action,
            operation.id,
            operation.folders.join(", ")
        );
        if let Some(issue) = &operation.issue {
            println!("  {ERROR}Cannot validate recovery data:{ERROR:#} {issue}");
        }
    }
    for action in &report.next_actions {
        println!("{HEADING}Next:{HEADING:#} {ACCENT}{action}{ACCENT:#}");
    }
    println!(
        "\n{MUTED}Local metadata does not establish project identity or Forever compatibility.{MUTED:#}"
    );
}

fn run(cli: Cli) -> Result<(), String> {
    if cli.command.is_none() && cli.path.is_none() && !cli.json && !cli.dry_run {
        println!(
            "{HEADING}Manage your WoW Forever add-ons from downloaded ZIPs.{HEADING:#}\n\n\
             {HEADING}Start with:{HEADING:#}\n  \
             {ACCENT}wowza configure <installation>{ACCENT:#}  Remember your game installation\n  \
             {ACCENT}wowza status{ACCENT:#}                    See installed add-ons and problems\n  \
             {ACCENT}wowza search \"Questie\"{ACCENT:#}          Find an add-on on CurseForge\n  \
             {ACCENT}wowza install <addon.zip>{ACCENT:#}       Preview and confirm an installation\n\n\
             Use {ACCENT}wowza --help{ACCENT:#} for all commands."
        );
        return Ok(());
    }
    let command = cli.command.unwrap_or(Command::Status { verbose: false });
    match command {
        Command::Search {
            query,
            url_only,
            yes,
        } => {
            if cli.dry_run {
                return Err("Search does not support --dry-run; use --url or --json to return a link without opening a browser.".into());
            }
            let url = wowza::search_url(&query)?;
            if url_only || cli.json {
                return print_handoff(&url, "search", Some(&query), cli.json);
            }
            println!("{HEADING}CurseForge search{HEADING:#}");
            wowza::interaction::handoff(
                &mut io::stdin().lock(),
                &mut io::stdout(),
                &url,
                yes,
                true,
                |url| open::that(url).map_err(|e| e.to_string()),
            )
        }
        Command::Continue { url, url_only, yes } => {
            if cli.dry_run {
                return Err("Continue does not support --dry-run; use --url or --json to return a link without opening a browser.".into());
            }
            let url = wowza::project_url(&url)?;
            if url_only || cli.json {
                return print_handoff(&url, "continue", None, cli.json);
            }
            println!("{HEADING}CurseForge project or release{HEADING:#}");
            wowza::interaction::handoff(
                &mut io::stdin().lock(),
                &mut io::stdout(),
                &url,
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
                    "{SUCCESS}{} installation:{SUCCESS:#} {}",
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
                        println!("{SUCCESS}No interrupted Wowza operations found.{SUCCESS:#}");
                    }
                    for p in pending {
                        println!(
                            "{WARNING}{}: {} {}{WARNING:#}\n  {ACCENT}{}{ACCENT:#}",
                            p.id,
                            p.action,
                            p.folders.join(", "),
                            p.recovery_command
                        );
                        if let Some(issue) = p.issue {
                            println!("  {ERROR}Cannot validate recovery data:{ERROR:#} {issue}");
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
                            "{HEADING}Archive:{HEADING:#} {}\n{HEADING}Uncompressed bytes:{HEADING:#} {}",
                            archive.display(),
                            preview.bytes
                        );
                        for (folder, count) in &preview.folders {
                            let (style, availability) = if existing.contains(&folder.to_lowercase())
                            {
                                (ERROR, "CONFLICT: already exists")
                            } else {
                                (SUCCESS, "no existing folder")
                            };
                            println!(
                                "  {ACCENT}{folder}{ACCENT:#}: {count} files {style}[{availability}]{style:#}"
                            );
                        }
                        println!(
                            "{HEADING}Preview only:{HEADING:#} nothing installed. {WARNING}Verify the release is intended for Forever.{WARNING:#}"
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
        eprintln!("{ERROR}Error:{ERROR:#} {error}");
        std::process::exit(1);
    }
}
