# Contributing

Use a focused issue or pull request describing the problem and expected behavior. Include OS version, app version, exact mouse model, USB versus LIGHTSPEED, and repeatable steps. Remove unit IDs, serial numbers and personal paths from diagnostics. Do not upload full mouse backups.

## Development

Install Rust with rustup. `rust-toolchain.toml` selects the tested compiler. Keep `Cargo.lock` committed.

Ubuntu/Debian build dependencies:

```sh
sudo apt install build-essential pkg-config libxkbcommon-dev libwayland-dev libgl1-mesa-dev libegl1-mesa-dev libx11-dev libxi-dev libxcursor-dev libxrandr-dev
```

Arch: `sudo pacman -S --needed base-devel rustup libxkbcommon wayland libglvnd libx11 libxi libxcursor libxrandr`.

Windows: install Visual Studio Build Tools with **Desktop development with C++** and the MSVC Rust toolchain. macOS: run `xcode-select --install`; official packages target macOS 13+.

```sh
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo check --locked --no-default-features
cargo run -- list
cargo run
```

Tests do not require a mouse and never write to hardware. Hardware testing should start with read-only `check`. Any flash test must preserve and verify an original backup. Do not generalize device support from a product name: use identity, feature and memory-layout checks.

## Packaging

Build with `cargo build --release --locked`, then run the command matching the host:

```sh
python3 scripts/package.py --platform linux-x86_64
# Also: linux-aarch64, macos-aarch64, macos-x86_64, windows-x86_64
```

The script uses standard Python only. `python3 scripts/test-monitor.py` exercises the background process and startup lifecycle after building, using isolated state/startup folders (Windows uses the test runner account’s app registry entries). Linux and Windows installer smoke tests run in CI, including update/uninstall and corrupt-download checks. Windows builds statically link the C runtime to avoid a separate Visual C++ runtime installation. Linux needs `dpkg-deb`; Windows needs NSIS; macOS uses `hdiutil` and `codesign`. Set `MACOSX_DEPLOYMENT_TARGET=13.0` before building macOS releases. Output goes in `dist/`. Icons are checked in; regenerate from the SVG with `python3 scripts/generate-icons.py` (requires `rsvg-convert`).

## UI artwork and captures

The original device drawing lives in `assets/mouse.svg`. Regenerate its bundled PNG with `python3 scripts/generate-mouse.py` (requires `rsvg-convert`). The LED curves and button callouts in `src/design.rs` use the same 300 × 400 coordinate system. Keep those aligned when changing the drawing.

For a native screenshot, set `G502X_CAPTURE=/absolute/path.png`, `G502X_CAPTURE_TAB=0` (0–4), and optionally `G502X_CAPTURE_SECTION=1` for a profile subtab. `G502X_CAPTURE_SIZE=1120x800` fixes capture dimensions; the supported minimum is 840 × 640. The capture window ignores pointer input and closes once the device read and layout have settled. Use an isolated `G502X_STATE_DIR` to avoid starting a test monitor from a development binary. Capture mode still requires a connected mouse, except on Settings.

## Releasing

1. Update the Cargo version, lockfile, changelog and `docs/RELEASE.md`.
2. Ensure the main-branch workflow succeeds on Linux x86-64/ARM64, Windows x86-64, and macOS Intel/Apple Silicon.
3. Tag the reviewed commit with `vX.Y.Z` and push the tag. The workflow rebuilds, tests, packages and publishes all installers plus SHA-256 checksums only if every platform succeeds.

Windows publisher signing and macOS Developer ID notarization are not configured. Ad-hoc Mac signatures are integrity signatures, not an Apple trust endorsement. Do not describe hardware support as verified merely because a cross-platform build passed.

Contributions are licensed under GPL-2.0-or-later. Preserve upstream and font notices.
