# Command reference

Wowza is an experimental Windows CLI. Its filesystem workflows are tested against temporary installations; actual in-game compatibility and player usability still need validation. Release selection remains the user's responsibility.

## Portable Windows package

When a portable package is available in [Releases](https://github.com/nu-sync/wowza/releases), download the Windows x64 ZIP and `SHA256SUMS`. A successful [Windows checks run](https://github.com/nu-sync/wowza/actions) also provides a `wowza-windows-x86_64` artifact containing both files, retained for 14 days; downloading Actions artifacts requires a GitHub account. Source installation is documented in the [README](../README.md).

For version 0.1.0, compare the ZIP's SHA-256 hash with its entry in `SHA256SUMS`:

```powershell
Get-Content .\SHA256SUMS
Get-FileHash .\wowza-0.1.0-windows-x86_64.zip -Algorithm SHA256
```

Then extract the ZIP and check the executable:

```powershell
Expand-Archive .\wowza-0.1.0-windows-x86_64.zip -DestinationPath .\wowza
cd wowza
.\wowza.exe --version
```

Use `.\wowza.exe` in place of `wowza` in the examples below while running from that folder. The portable executable does not require Cargo. Keep the included documentation and license files with it.

## Installation selection

`wowza configure <installation>` validates and remembers a Forever installation. Use the installation root, not `Interface/AddOns`. `configure <installation> --dry-run` validates without saving.

Path precedence is:

1. `--path <installation>`
2. `WOWZA_FOREVER_PATH`
3. The saved installation
4. `C:\battlenet\World of Warcraft\_forever_`

Settings live at `%APPDATA%\Wowza\config.json`. `WOWZA_CONFIG_FILE` selects another settings file, useful for isolated setups and tests. Explicit paths and environment overrides work even when saved settings are invalid.

`Interface` and `Interface/AddOns` must be normal directories. Linked folders and Windows junctions are rejected during preview and checked again before execution, so they cannot redirect a prepared change into another installation.

## Inspection and browser handoffs

`wowza` and `wowza status` show packages, problems, and suggested actions. `status --verbose` includes TOC metadata. Consistent ownership records group several folders into one package; conflicting records remain separate.

`list` and `scan` retain their folder-oriented display. `doctor` reports missing TOCs, changed packages, and pending operations. These commands share one inspection result and do not modify the installation. TOCs are read as UTF-8 with replacement for invalid bytes.

`search "Leatrix"` opens CurseForge search in the default browser after confirmation. Results appear on the website; check that Forever is selected and verify the release. `continue <url>` opens an HTTPS CurseForge WoW add-on project or release page. Both accept `--yes` to skip Wowza's prompt. Opening a page does not install an add-on or select a game installation.

## Package changes

| Command | Behavior |
| --- | --- |
| `inspect <zip>` | Validate a ZIP and report existing-folder conflicts. |
| `install <zip>` | Preview and install a local ZIP without overwriting existing folders. |
| `update <folder> <zip>` | Replace a managed package, retaining its original files. |
| `remove <folder>` | Move an unchanged managed package into retained storage. |

`inspect`, `install`, and `update` are aliases for `inspect-archive`, `install-archive`, and `update-archive`. Use any owned folder name to select a multi-folder package. Update and removal affect the whole package.

Add-ons installed outside Wowza can be inspected but remain unmanaged. There is no import or adoption command. Installation refuses occupied folders, and updates/removals require Wowza's ownership record.

ZIPs must contain a top-level TOC in every add-on folder. Validation rejects unsafe Windows paths, ambiguous names, special files, and corrupted data. Entry names must be UTF-8. Limits are 10,000 entries and 512 MiB uncompressed; installation also limits compressed archives to 512 MiB.

Updates require exactly the existing folder names. Verify independently that the ZIP is from the same project and targets Forever. Wowza does not establish project identity, release compatibility, or whether a replacement is newer. Changed, missing, or extra files prevent updates and removals. Saved settings in `WTF` are preserved.

## Previews and automation

Install, update, remove, and recover prompt by default. `--dry-run` previews without prompting, locking, staging, or writing installation files. Execution rechecks the preview's archive, package, or journal under the installation lock and refuses changed inputs.

`--json` supports inspection, pending operations, configuration, previews, and operation results. JSON package mutations require `--yes`; use `--dry-run --json` to obtain a read-only preview. Errors go to stderr with a nonzero exit status. Browser handoffs do not support `--json` or `--dry-run`.

## Recovery and limits

`pending` lists interrupted operations and installation-specific PowerShell recovery commands. `recover` selects the sole pending operation and asks for confirmation. With several pending operations, pass an explicit ID: `recover <transaction-id>`. `recover --dry-run` checks the journal, folder contents, and destination conflicts without writing. Execution repeats those checks under the installation lock.

Recovery attempts to restore the prior state. It refuses conflicting destinations, changed files, and changed journals, and retains transaction data for inspection. Completed updates and removals retain original files under `.wowza`; automatic rollback of completed operations is not supported. Do not overwrite existing folders when restoring retained data. Legacy version 1 ownership records remain readable.

Keep the game and other add-on managers closed during changes. Wowza's lock coordinates only Wowza processes. Keep backups: power-loss durability and automatic recovery are not guaranteed.

Wowza does not download ZIPs, identify CurseForge projects, choose compatible releases, or check for newer releases automatically. See [development notes](https://github.com/nu-sync/wowza/blob/main/DEVELOPMENT.md) for integration research and [lifecycle models](https://github.com/nu-sync/wowza/blob/main/models/README.md) for verification limits.

## Building and contributing

Use Rust 1.91 or newer. `cargo build --locked --release` builds `target/release/wowza.exe`. Run `.\scripts\check.ps1` on Windows for formatting, Clippy, default tests, interruption tests, and a release build. `cargo test --locked --features test-hooks --test interruption_cli` runs the process-interruption scenarios. Omit `test-hooks` from release builds.

Shared fixtures live in `tests/common/mod.rs`. The library exposes installation inspection and prepared install/update/remove/recovery plans. Callers explicitly authorize execution by consuming a plan with `execute`; JSON previews are informational, not executable input. See [Repository Guidelines](../AGENTS.md) for contributor conventions.

Report bugs in [GitHub Issues](https://github.com/nu-sync/wowza/issues) with the Wowza version, Windows version, command, and error. Remove usernames, personal paths, account information, and saved settings from reports. The project is licensed under [MIT](../LICENSE).
