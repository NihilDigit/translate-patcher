# translate-patcher

`translate-patcher` is a terminal UI patcher for embedding external translation dictionaries into visual novel game resources.

The first supported target is MTool-style JSON applied to TyranoScript games packaged as Electron ASAR archives. The goal is simple: Linux/Wine users can patch the game resource package once and play without running MTool or a live text hook.

## Status

This repository is in early development. The v0.1 scope is intentionally narrow:

- Ratatui-based TUI
- TyranoScript in `resources/app.asar`
- MTool-style JSON dictionaries
- in-place patching with a timestamped `.bak`
- restore from backup

Image text, video subtitles, Kirikiri, Ren'Py, Unity, and arbitrary binary formats are out of scope for v0.1.

## Install

After the first release is published:

```bash
curl -fsSL https://raw.githubusercontent.com/NihilDigit/translate-patcher/main/install.sh | bash
```

The installer downloads the latest GitHub Release artifact for the current platform, verifies checksums, and installs `translate-patcher` to `~/.local/bin`.

## Build

```bash
cargo build
```

Run the TUI:

```bash
cargo run -p translate-patcher
```

## Release Versioning

`translate-patcher` uses CalVer tags:

```text
vYY.MM.DD
vYY.MM.DD-1
```

Release binaries are built by GitHub Actions from manually pushed tags.

## License

GPL-3.0-or-later.

