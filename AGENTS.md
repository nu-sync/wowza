# Repository Guidelines

## Project Structure & Module Organization

Wowza is a Rust 2024 CLI for WoW Forever add-ons and local ZIP management on Windows and macOS.

- `src/main.rs`, `src/cli.rs`: entry point, commands, and output.
- `src/lib.rs`: TOC parsing, installation scanning, and CurseForge URLs.
- `src/archive.rs`: ZIP inspection and portable (Windows-safe) path validation.
- `src/manage.rs`, `src/manage/`: ownership, locks, recovery, inspection, and prepared changes.
- `src/config.rs`, `src/interaction.rs`: installation selection and injectable prompts/browser handoffs.
- `tests/`: CLI tests and shared temporary-installation fixtures in `common/`.
- `models/`: TLA+ lifecycle models and reproducible TLC instructions.
- `README.md`, `DEVELOPMENT.md`, and `SPEC.md`: current usage, design decisions, and intended scope. Build output belongs in ignored `target/`.

## Build, Test, and Development Commands

Use Rust 1.91 or newer. Run from the root:

- `cargo build --locked --release`: build `target/release/wowza.exe` on Windows.
- `cargo run -- --help`: inspect CLI commands.
- `cargo test --locked`: run unit and integration tests; lifecycle coverage runs on Windows and macOS.
- `.\scripts\check.ps1`: run all checks, interruption scenarios, and a release build.
- `cargo fmt --check`: check formatting; use `cargo fmt` to apply it.
- `cargo clippy --all-targets -- -D warnings`: lint all targets, treating warnings as errors.

## Coding Style & Naming Conventions

Follow rustfmt defaults, including four-space indentation. Use `snake_case` for functions and tests, `PascalCase` for types, and `SCREAMING_SNAKE_CASE` for constants. Keep presentation in `cli.rs` and reusable behavior in library modules. Return actionable `Result` errors and preserve platform guards; every platform-specific code path needs tests that run on that platform in CI.

## Testing Guidelines

Use `#[test]` and `tempfile`. Name tests after observable behavior, such as `preview_and_conflict_do_not_modify_installation`. Cover cancellation, unsafe archives, conflicts, modified files, and recovery when changing those paths. `cargo test --features test-hooks` exercises abrupt child-process exits; omit this feature from release builds. No numeric coverage threshold is configured. Review affected TLA+ models; see `models/README.md` for reproduction and limitations.

## Commit & Pull Request Guidelines

Use concise imperative subjects, such as `Fix recovery conflict handling`. Keep each commit focused on one coherent change. PRs should explain behavior changes, link relevant issues, list validation commands and results, and include CLI output examples when output changes. Keep README usage examples brief; place detailed options and limits in `docs/REFERENCE.md`.

## Configuration & Installation Safety

Set `--path` or `WOWZA_FOREVER_PATH` to the installation root, not `Interface/AddOns`. Test mutations only against temporary installations. Preserve no-overwrite checks, locking, recovery records, and `WTF` settings. Never commit credentials, personal paths, or user data. No CurseForge API key is needed; local TOCs do not prove release compatibility.
