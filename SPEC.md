# Wowza: High-Level Specification

## Purpose

Wowza is a Rust command-line tool for managing World of Warcraft: Forever add-ons through CurseForge. It should make it easy to discover compatible add-ons, understand what is installed, and keep an installation up to date without managing folders by hand.

The project is Forever-first: it targets the live Forever client and selects releases intended for that game version. Its core capabilities should be accessible through a consistent management API and exposed as command-line commands. The API described here is a set of operations; this specification does not require a network service.

## Intended experience

A player points Wowza at their Forever installation, scans existing add-ons, identifies the CurseForge projects they want to manage, and uses simple commands to search, install, inspect, update, and remove them. Wowza should explain what it finds and what it plans to change, especially when existing add-ons cannot be identified confidently.

A single CurseForge project may install several add-on folders. Wowza should understand and manage those folders as one package while respecting other add-ons and the player's saved settings.

## Continue with CurseForge

Wowza may hand installation or updates off to CurseForge by opening the relevant project or release page, where the player can continue through the website or CurseForge app. By default, Wowza should show the intended handoff and ask for confirmation before opening it.

An explicit promptless option should skip Wowza's confirmation and open the handoff immediately, making the operation convenient for scripts and repeat use. Promptless mode does not bypass any browser, operating system, or CurseForge prompts, and it does not imply that installation has completed. Wowza should report that the handoff was opened and offer a subsequent scan to check the local installation. The player remains responsible for selecting the correct Forever installation and completing any required steps in CurseForge.

The exact option name and whether a particular CurseForge link can target a specific release or game installation are implementation details to validate.
## Core operations

- **Create / install:** Find a CurseForge project and install an appropriate Forever release, recording its identity, version, and installed folders.
- **Read / inspect:** List installed and managed add-ons and show details such as project, installed release, local folders, compatibility, and management status.
- **Update:** Check for compatible releases and update one or more managed add-ons, with a clear preview of intended changes.
- **Delete / remove:** Remove a managed add-on and its owned folders. Preserve saved settings by default; removing settings should be a separate, explicit action.
- **Search:** Find CurseForge projects and inspect their available Forever releases.
- **Scan:** Inspect the local AddOns directory, identify installed folders and metadata, and propose matches to CurseForge projects. Distinguish confirmed matches, uncertain matches, and unmanaged add-ons; do not silently take ownership of uncertain matches.
- **Doctor:** Diagnose common problems, such as a missing or incorrect game path, incompatible or missing release metadata, conflicting folder ownership, incomplete installations, and unavailable API access or downloads. Explain suggested remedies.

These operations should be usable individually and compose naturally in automation. Human-readable output is the default; structured output is desirable for scripts and other integrations.

## Compatibility and access

Wowza should use CurseForge's documented API and respect its access requirements and project download restrictions. API access may require an approved key; the tool should make configuration and failures understandable and keep credentials out of logs and project files.

Release selection must explicitly account for Forever compatibility rather than assuming that the newest Retail or Classic file will work. The exact metadata and identification rules should be validated against the live CurseForge API. When compatibility or download availability cannot be established, Wowza should say so instead of guessing.

## Protecting the player's installation

Wowza should make changes predictable and recoverable. It should stage and validate downloads, avoid unsafe archive paths, detect conflicts with folders it does not manage, and preserve a backup of replaced files so a failed update can be recovered or rolled back. It should not modify the game client or unrelated folders. Add-on settings, normally stored separately under `WTF`, should remain intact during routine add-on management.

Existing local modifications and add-ons installed by another manager should be visible rather than silently overwritten. The player should be able to see what Wowza manages and what it leaves unmanaged.

## Scope and direction

The initial focus is a dependable Windows command-line experience for a single Forever installation and CurseForge as the source. A useful first milestone is a read-only scan and diagnosis, followed by CurseForge search and confirmed matching, then safe installation, updates, removal, and rollback.

A graphical interface, additional game versions, other add-on sources, and advanced multi-install management are possible future extensions, not requirements for the initial version. Implementation details, command syntax, storage format, and the precise public API can be refined as the project is designed and tested.
