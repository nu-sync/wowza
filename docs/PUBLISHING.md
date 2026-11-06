# Publishing Wowza

Wowza is an experimental Windows CLI. Source publication and a downloadable release are separate steps; a passing build does not establish in-game compatibility or player usability.

## Verify a change

Run `./scripts/check.ps1` in PowerShell. GitHub's **Windows checks** workflow runs the same formatting, Clippy, default tests, interruption tests, and release build, then packages a Windows x64 executable. Release builds exclude interruption hooks.

Model verification is independent of Codex. Follow [the model instructions](../models/README.md) to run all seven checks with Java and the TLA+ tools JAR.

## Prepare a Windows download

```powershell
.\scripts\package-release.ps1
```

This creates `target/dist/wowza-<version>-windows-x86_64.zip` and `target/dist/SHA256SUMS`. It reads the version from Cargo metadata, builds locked dependencies without test hooks, removes local source-path prefixes from compiler output, and includes documentation and dependency license notices. Existing output directories are refused; use `-OutputDirectory <new-directory>` to repeat a build.

Successful GitHub checks upload these files as the `wowza-windows-x86_64` artifact, retained for 14 days. These are test builds, not a published release. Download artifacts from the run for the exact commit being reviewed.

Before a public release, extract the ZIP on a fresh Windows machine, verify its checksum, and exercise the documented setup/install/update/remove flows against a temporary installation. Separately confirm a compatible add-on loads in game with the player's consent and backups. Record the Windows version, result, and any limitations without sharing personal paths or game account data.

## Publish a prerelease

Create a GitHub release targeting the verified commit, with a tag matching Cargo's version (for example, `v0.1.0`). Attach the ZIP and `SHA256SUMS`; mark the release as a prerelease. Explain the manual ZIP workflow and known limits: unmanaged add-ons cannot be adopted, folder sets must match for updates, and recovery applies to interrupted operations rather than completed updates.

## Keep personal configuration private

Only commit source, documentation, tests, and sanitized verification evidence. Build artifacts belong in ignored `target/`. Root `.env*`, `config.json`, `.wowza/`, and `local/` are ignored; normal saved settings live outside this repository. Do not copy game installations, saved variables, credentials, personal paths, or unredacted diagnostics into issues or Git history.

Before pushing, inspect staged files and reachable history with a secret scanner and check for personal path/name/email markers. Use a project identity or a GitHub no-reply address for commits. Redact diagnostics without changing reported outcomes. Ignore rules do not remove data already committed.
