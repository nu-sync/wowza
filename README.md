# Wowza

An experimental Windows CLI for managing World of Warcraft: Forever add-ons from downloaded ZIPs. Preview changes, manage related folders together, and recover interrupted operations. No API key is needed.

## Set up once

Install Git and Rust 1.91 or newer with Cargo, then run:

```powershell
git clone https://github.com/nu-sync/wowza.git
cd wowza
cargo install --locked --path .
wowza configure 'C:\battlenet\World of Warcraft\_forever_'
```

Use your game installation root, not its `Interface\AddOns` folder. Wowza remembers it. Use `--path '<installation>'` to override it for one command.

## See what is installed

```powershell
wowza
wowza status --verbose
```

The default view shows packages, problems, and suggested next steps. Existing add-ons installed outside Wowza remain unmanaged; there is no import command yet.

## Find and install an add-on

```powershell
wowza search "Leatrix"
```

This opens CurseForge in your browser. Download a ZIP and verify that its release targets Forever. Wowza does not download releases or establish compatibility. Preview and install it:

```powershell
wowza install 'C:\Downloads\addon.zip' --dry-run
wowza install 'C:\Downloads\addon.zip'
```

Wowza shows the affected folders and asks before applying a change.

## Update or remove a package

Use a folder name from a package installed through Wowza:

```powershell
wowza update MyAddon 'C:\Downloads\new-release.zip'
wowza remove MyAddon
```

For updates, choose a ZIP from the same project with the same folder names. Both commands affect the whole package, preserve saved settings, and retain the original files. Modified packages are refused.

## Recover an interrupted change

```powershell
wowza pending
wowza recover
```

Recovery previews the interrupted operation and asks for confirmation. If several operations are pending, use the command printed by `pending`. Recovery does not undo completed updates or removals.

Keep the game and other add-on managers closed during changes, and keep backups.

Add `--dry-run` to preview a change, `--yes` to skip its confirmation, or `--json` for structured output. JSON package changes require `--yes` or `--dry-run`.

Run `wowza --help` for all commands. See the [command reference](docs/REFERENCE.md) for options and limits, or [Repository Guidelines](AGENTS.md) to contribute. Report problems in [GitHub Issues](https://github.com/nu-sync/wowza/issues). Licensed under [MIT](LICENSE).
