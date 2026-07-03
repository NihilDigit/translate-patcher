# translate-patcher

`translate-patcher` is a terminal UI patcher for embedding external translation dictionaries into visual novel game resources.

The first supported target is MTool-style JSON applied to TyranoScript games packaged as Electron ASAR archives. The goal is simple: Linux/Wine users can patch the game resource package once and play without running MTool or a live text hook.

## Status

This repository is in early development. The current MVP can:

- scan the launch folder for `resources/app.asar` and `*.json`
- let the user choose specific ASAR and JSON files inside the launch folder's parent
- preview Tyrano scenario count and estimated translation matches
- create a timestamped `.bak`
- patch `data/scenario/**/*.ks` inside the ASAR
- write `translate-patcher-report.txt`

Image text, video subtitles, Kirikiri, Ren'Py, Unity, and arbitrary binary formats are out of scope for v0.1.

## Usage

Start the app from a game folder:

```bash
translate-patcher
```

The TUI shows detected files and asks for confirmation before modifying anything. By default it uses a conservative TyranoScript strategy:

- replace plain scenario text
- replace `#character name` lines
- keep Tyrano tags and resource paths intact

The patch is written in place after a backup is created:

```text
resources/app.asar.YYMMDD-HHMMSS.bak
```

## Install

```bash
curl -fsSL https://raw.githubusercontent.com/NihilDigit/translate-patcher/main/install.sh | bash
```

The installer downloads the latest GitHub Release artifact for the current Linux platform, verifies checksums, and installs `translate-patcher` to `~/.local/bin`.

Supported installer targets:

- `x86_64-unknown-linux-musl`
- `aarch64-unknown-linux-musl`

## Build

```bash
cargo build
```

Run the TUI:

```bash
cargo run -p translate-patcher
```

Quality checks:

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test
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
