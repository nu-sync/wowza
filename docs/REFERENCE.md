# Command reference

Wowza is an experimental Windows and macOS CLI. Its filesystem workflows are tested against temporary installations; actual in-game compatibility and player usability still need validation. Release selection remains the user's responsibility.

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
4. `C:\battlenet\World of Warcraft\_forever_` on Windows, or `/Applications/World of Warcraft/_forever_` on macOS

Settings live at `%APPDATA%\Wowza\config.json` on Windows and `~/Library/Application Support/Wowza/config.json` on macOS. `WOWZA_CONFIG_FILE` selects another settings file, useful for isolated setups and tests. Explicit paths and environment overrides work even when saved settings are invalid.

`Interface` and `Interface/AddOns` must be normal directories. Symbolic links and Windows junctions are rejected during preview and checked again before execution, so they cannot redirect a prepared change into another installation.

## Inspection and browser handoffs

`wowza` shows a short getting-started guide without reading configuration or scanning the installation. `wowza --help` lists all commands and options.

`wowza status` shows packages, problems, and suggested actions. Each package line shows the title and version from `<Folder>_Forever.toc`, falling back to `<Folder>.toc` and then the first TOC, with WoW color codes removed. `status --verbose` includes all TOC metadata. Consistent ownership records group several folders into one package; conflicting records remain separate. For existing scripts, supplying only global options (`--path`, `--json`, or `--dry-run`) still runs status; prefer an explicit `status` command in new scripts.

`list` and `scan` retain their folder-oriented display. `doctor` reports missing TOCs, changed packages, and pending operations. These commands share one inspection result and do not modify the installation. TOCs are read as UTF-8 with replacement for invalid bytes.

`search "Leatrix"` opens CurseForge search in the default browser after confirmation. Results appear on the website; check that Forever is selected and verify the release. `continue <url>` opens an HTTPS CurseForge WoW add-on project or release page. Both accept `--yes` to skip Wowza's prompt. Opening a page does not install an add-on or select a game installation.

### Keyless agent handoffs

```powershell
wowza search "Questie" --url
wowza search "Questie" --json
wowza continue https://www.curseforge.com/wow/addons/questie --url
```

`--url` prints one validated URL. `--json` returns a structured handoff, for example:

```json
{
  "kind": "url_handoff",
  "action": "search",
  "query": "Questie",
  "url": "https://www.curseforge.com/wow/search?class=addons&page=1&pageSize=20&sortBy=relevancy&search=Questie&gameVersionTypeId=88568",
  "content_fetched": false
}
```

For `continue --json`, `action` is `continue` and `query` is omitted. Both modes work without a game installation or API key and never fetch pages, prompt, or open a browser, including when `--yes` is supplied. If both `--url` and `--json` are present, JSON takes precedence. Invalid queries or URLs fail with a nonzero exit status and an error on stderr.

An agent passes `url` to its own browsing tool and reads the returned content. This output is a link, not search results or a compatibility claim. Page access can still be blocked or require user interaction; the browsing tool must report that rather than invent results. `--dry-run` is not supported for these commands; use `--url` or `--json` instead.

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

## Terminal colors

Interactive output uses cyan headings, magenta package names and suggested commands, green success messages, yellow warnings, and red errors. Text labels carry the same meaning without color. Help uses the same palette.

Colors adapt to the terminal and are disabled when output is redirected. Set `NO_COLOR=1` or `CLICOLOR=0` for plain text, or `CLICOLOR_FORCE=1` to force colors, including in redirected output. `NO_COLOR` takes precedence. JSON and URL-only output remain plain even when colors are forced.

```powershell
$env:NO_COLOR = '1'
wowza status
Remove-Item Env:NO_COLOR
```

## Previews and automation

Install, update, remove, and recover prompt by default. `--dry-run` previews without prompting, locking, staging, or writing installation files. Execution rechecks the preview's archive, package, or journal under the installation lock and refuses changed inputs.

`--json` supports inspection, pending operations, configuration, previews, operation results, and the keyless URL handoffs above. JSON package mutations require `--yes`; use `--dry-run --json` to obtain a read-only preview. Errors go to stderr with a nonzero exit status.

## Recovery and limits

`pending` lists interrupted operations and installation-specific recovery commands, quoted for PowerShell on Windows and for a POSIX shell such as zsh on macOS. `recover` selects the sole pending operation and asks for confirmation. With several pending operations, pass an explicit ID: `recover <transaction-id>`. `recover --dry-run` checks the journal, folder contents, and destination conflicts without writing. Execution repeats those checks under the installation lock.

Recovery attempts to restore the prior state. It refuses conflicting destinations, changed files, and changed journals, and retains transaction data for inspection. Completed updates and removals retain original files under `.wowza`; automatic rollback of completed operations is not supported. Do not overwrite existing folders when restoring retained data. Legacy version 1 ownership records remain readable.

Keep the game and other add-on managers closed during changes. Wowza's lock coordinates only Wowza processes. Keep backups: power-loss durability and automatic recovery are not guaranteed.

Wowza does not download ZIPs, identify CurseForge projects, choose compatible releases, or check for newer releases automatically. See [development notes](https://github.com/nu-sync/wowza/blob/main/DEVELOPMENT.md) for integration research and [lifecycle models](https://github.com/nu-sync/wowza/blob/main/models/README.md) for verification limits.

## Building and contributing

Use Rust 1.91 or newer. `cargo build --locked --release` builds `target/release/wowza.exe`. Run `.\scripts\check.ps1` (or `pwsh scripts/check.ps1` on macOS) for formatting, Clippy, default tests, interruption tests, and a release build. `cargo test --locked --features test-hooks --test interruption_cli` runs the process-interruption scenarios. Omit `test-hooks` from release builds.

Shared fixtures live in `tests/common/mod.rs`. The library exposes installation inspection and prepared install/update/remove/recovery plans. Callers explicitly authorize execution by consuming a plan with `execute`; JSON previews are informational, not executable input. See [Repository Guidelines](../AGENTS.md) for contributor conventions.

Report bugs in [GitHub Issues](https://github.com/nu-sync/wowza/issues) with the Wowza version, operating system version, command, and error. Remove usernames, personal paths, account information, and saved settings from reports. The project is licensed under [MIT](../LICENSE).
