# AGENTS.md

This repository contains `translate-patcher`, a terminal UI tool for embedding external translation dictionaries into visual novel game resources. The first supported backend is MTool-style JSON applied to TyranoScript games packaged as Electron ASAR archives.

The project is for Linux/Wine users who already have a translation JSON and want to patch the game resource package directly, without running MTool or a runtime hook while playing.

## Product Scope

Keep the first version narrow.

- Build a Ratatui app. The user-facing product is a TUI, not a scriptable CLI.
- Support TyranoScript packaged in `resources/app.asar`.
- Support MTool-style JSON dictionaries shaped as `{ "source text": "translated text" }`.
- Patch in place only after creating a timestamped `.bak`.
- Provide restore and backup deletion from the TUI.
- Do not promise support for image text, video subtitles, Kirikiri, Ren'Py, Unity, or arbitrary binary formats in v0.1.

The binary name is:

```text
translate-patcher
```

`--help` and `--version` are acceptable standard flags. Do not add patch/restore/scan CLI subcommands unless the product direction changes.

## User Flow

Optimize for the common path: launch from the game folder, confirm the detected files, patch.

The default flow is:

1. Start in the current working directory.
2. Detect candidate files:
   - `resources/app.asar`
   - `*.json`
   - an optional game executable for display only
3. Show the selected ASAR and JSON.
4. Let the user change either file through an in-app file picker.
5. Preview:
   - engine/backend
   - translation entry count
   - scenario file count
   - estimated matches
   - backup path
6. Ask for explicit confirmation.
7. Create backup.
8. Patch ASAR.
9. Show result:
   - modified files
   - applied entries
   - unused entries
   - backup path
10. Offer restore backup, delete backup, or quit.

The happy path should need only two confirmations after launch.

## File Picker

Use an in-TUI file picker. Do not call a system GUI file picker by default.

Selection is limited to the current working directory's parent:

```text
scan_root = cwd.parent().unwrap_or(cwd)
```

Rules:

- The user may navigate inside `scan_root`.
- The user may not navigate above `scan_root`.
- ASAR mode shows directories and `*.asar`.
- JSON mode shows directories and `*.json`.
- `Backspace` goes up one directory, capped at `scan_root`.
- `Enter` opens a directory or selects a matching file.
- `Esc` cancels and returns to the previous screen.

This keeps selection useful for normal game folders without exposing the whole filesystem.

## TUI Design

Keep the interface plain and predictable.

- Use simple panels, short labels, and clear button names.
- Prefer one primary action per screen.
- Avoid dense menus.
- Avoid decorative UI.
- Do not show implementation details unless they help the user decide or recover.
- Long paths should be shortened from the left or shown relative to `scan_root` where possible.
- Every destructive action, including deleting a backup, requires a confirmation screen.

Initial screens:

- `Select files`
- `Confirm patch`
- `Patching`
- `Patch complete`
- `Restore backup`
- `Error`

## Patch Strategy

Default mode is conservative.

For Tyrano `.ks` scenario files:

- Process files under `data/scenario/**/*.ks`.
- Replace plain scenario text.
- Replace `#character name` lines.
- Preserve Tyrano tags such as `[p]`, `[bg]`, `[jump]`, `[button]`, and their control parameters.
- Do not replace resource paths, labels, storage names, target names, image names, or variable identifiers by default.

Aggressive whole-file replacement may be added later, but it must be opt-in and clearly labeled as higher risk.

Do not rely on runtime hooks for the main patch path. The patched game should work by reading modified resources directly.

## ASAR Handling

Implement ASAR support in the core layer.

Requirements:

- Parse the ASAR header.
- Preserve non-target files byte-for-byte where possible.
- Repack with correct offsets and sizes.
- Recompute integrity metadata when present.
- Write to a temporary file first.
- Replace the target ASAR only after the new archive is fully written.
- Leave the `.bak` untouched unless the user explicitly restores or deletes it.

Never modify the selected ASAR before the backup has been created successfully.

## Architecture

Separate core logic from the TUI.

Suggested layout:

```text
crates/
  translate-patcher-core/
    asar
    mtool_json
    tyrano
    patch_plan
  translate-patcher-tui/
    app state
    screens
    widgets
    file picker
```

The core crate should be testable without a terminal. The TUI crate should call core operations and render state.

## Reporting

After patching, produce a small report file next to the patched ASAR unless the user disables it later:

```text
translate-patcher-report.txt
```

Include:

- selected ASAR
- selected JSON
- backup path
- modified files
- applied entries
- unused entries count
- timestamp
- tool version

Do not dump the full translation dictionary into the report.

## Backup and Restore

Backup name format:

```text
app.asar.YYMMDD-HHMMSS.bak
```

Restore should copy the selected backup back to the ASAR path after confirmation.

Backup deletion is optional and must be explicit. Prefer keeping backups by default.

## Release Model

The project uses CalVer tags:

```text
vYY.MM.DD
vYY.MM.DD-1
```

Examples:

```text
v26.07.01
v26.07.01-1
```

Releases are manually tagged. GitHub Actions builds release binaries from tags and uploads artifacts to GitHub Releases.

The install script installs the latest GitHub Release, not an arbitrary CI artifact.

Initial artifacts:

- `x86_64-unknown-linux-musl`
- `aarch64-unknown-linux-musl`
- `x86_64-pc-windows-msvc` if practical
- `checksums.txt`

The one-line installer should install to `~/.local/bin` on Linux and verify checksums.

## CI

MVP may start with only the release workflow.

When normal CI is added, keep it lightweight:

```text
cargo fmt --check
cargo clippy -- -D warnings
cargo test
```

Do not add slow integration jobs until there are real fixtures and a reason to run them on every push.

## Safety Rules

- Do not silently delete user files.
- Do not modify files outside the selected ASAR, report path, and backup path.
- Do not traverse above the configured picker root in the TUI.
- Do not download translation data.
- Do not call external translation APIs.
- Do not upload game files, dictionaries, reports, or paths.
- Treat game resources and translation files as local user data.

## Development Preferences

- Use Rust stable.
- Prefer small, explicit modules over framework-heavy abstractions.
- Keep parsing code deterministic and covered by focused tests.
- Add fixtures for ASAR and Tyrano parsing as soon as core behavior stabilizes.
- Use `ratatui` and `crossterm` unless there is a concrete reason to change.
- Keep dependencies conservative, especially for archive parsing and terminal UI.

## Non-Goals for v0.1

- Machine translation.
- Live text hook.
- OCR.
- GUI file dialogs.
- Package manager distribution.
- Multi-engine support beyond TyranoScript ASAR.
- Editing image assets.
- Perfect translation coverage.

