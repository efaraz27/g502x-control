# Validation

Tested on 2026-10-11 against the user's actual Logitech G502 X Plus, on Arch Linux x86-64, kernel 7.2.6-arch2-1, using Rust 1.95.0. The device reports HID++ 4.2, profile format 5, five profiles, eleven buttons with G-Shift, sixteen 255-byte memory sectors, eight LEDs, and DPI choices from 100 to 25,600 in steps of 50.

## Verified on real hardware

- USB (`046d:c095`) and LIGHTSPEED paired HID node (`046d:4099`) discovery, correct device identity, feature queries, battery/status reads, all five profiles, and valid profile/directory CRCs.
- Cable preference when both transports are present; receiver fallback after the cable was unplugged.
- On both USB and LIGHTSPEED: switched to software mode, set DPI to 850 and report rate to 500 Hz, verified readback, restored 800 DPI / 1000 Hz / onboard profile 2 / DPI stage 1.
- No-op saves on all five profiles made no flash writes.
- Over LIGHTSPEED: renamed disabled profile 3 to `Linux save test`, verified the name and CRC in flash, and confirmed other profiles and the directory were unchanged. Restored the full backup and compared **all 16 sectors byte-for-byte**. All matched. Active mode, profile, DPI stage, sensitivity, rate and wheel state were restored.
- Off, static, breathing, spectrum-cycle and eight-segment RGB commands were acknowledged by the mouse. The firmware does not expose live RGB readback, so this verifies protocol acceptance rather than a visual/color measurement.
- Wheel reversal was changed, read back, then restored.
- Onboard profiles remained unchanged after the live lighting/wheel test.

## Software and interface checks

- `cargo test --locked`: 12 unit tests pass, covering CRC vectors, reserved-byte/macro preservation, UTF-16 names, LED slot placement, key/button encodings, DPI range decoding, backup rejection, HID++ error handling, unrelated/truncated replies, and preservation of factory profile-name bytes behind the new editor placeholder.
- `cargo clippy --all-targets -- -D warnings`: passes.
- Release GUI build and headless CLI compilation: pass.
- Native GUI launched under the existing Wayland session; screenshots inspected for the overview and profile editor.
- CLI examples checked for argument parsing and read-only profile previews.

## Version 0.2 interface redesign

The release build and Clippy checks passed again after the redesign. Native screenshots were inspected for Overview, Profiles, button assignments, Lighting, and Device & backups, including an 840 × 740 compact window and wider 1120/1280-pixel windows. Layout fixes cover sidebar anchoring, full-width cards, and eight-segment color controls.

The redesign checks only read the connected mouse. The three protocol, profile-format and transport source files were verified unchanged by SHA-256 against the pre-redesign versions. The hardware write tests above belong to the original implementation validation; they were not repeated for this interface update.

## Limits of this validation

Button remapping and persistent profile lighting use the reviewed upstream wire layouts; each possible mapping/effect has not been physically exercised. Advanced profile angle-snapping/timeouts have not been behaviorally measured. The dedicated receiver fallback endpoint (`046d:c547`) is implemented; the tested wireless route was the kernel's paired device node. Disconnect during flash, old firmware, different kernel versions and other Linux distributions were not tested. Existing multi-step macros and custom animations are preserved, not authored.

Automatic pre-save and pre-restore backups remain local to the user; no device backups are included in this repository. No firmware update, receiver pairing or permanent OS input-driver change was performed.

## Version 0.3 portability and packaging

Linux read-only hardware checks passed after extracting the hidraw backend, including LIGHTSPEED with the Ubuntu-built distributable binary: all five profiles, CRCs, field ranges and no-op saves passed without flash writes. The file storage port adds tests for never overwriting existing JSON and rejecting Unix symlink destinations (14 tests on Unix; 13 on Windows).

The build workflow runs formatting, Clippy, tests, headless compilation, GUI release compilation and packaging on Linux x86-64/ARM64, Windows x86-64, macOS Intel and macOS Apple Silicon. The public Actions history records the result for each commit. Windows and macOS device communication remains experimental; no physical mouse test on those systems has been performed. New installer generation does not itself verify OS installation or hardware behavior.

## Version 0.4 live updates, notifications and tray

- 24 unit tests on Linux, including preservation of unsaved drafts and the original write-conflict baseline, open binding editors, unapplied live inputs, alert cooldown/hysteresis/charging suppression, persisted deduplication, startup path quoting and atomic preferences.
- On the connected G502 X Plus, the running GUI observed an external temporary switch to software mode and 850 DPI without pressing Refresh. After the test, the original mode, active profile, DPI stage, DPI, rate and wheel state were restored. Profile bytes and directory remained unchanged.
- Native Settings screenshots inspected at 1240 × 860 and 840 × 740. Settings use responsive cards with a persistent Save preferences bar.
- Linux notification test accepted by the desktop notification service. The mouse was charging above the warning threshold; low-charge transitions and duplicate suppression were tested with synthetic unit-test data, not by draining the hardware battery.
- Linux StatusNotifierItem registration and tray battery/charging labels verified against the active panel. Real tray menu actions opened the app, selected Settings in the existing window without creating another, and quit the monitor. Monitor/startup lifecycle tests use isolated state and startup locations, without changing the user's real login settings.
- A new cross-platform lifecycle smoke test checks disabled startup, opt-in registration, singleton behavior, session quit, relaunch, persistent stop and startup removal. CI runs it after every native release build.

Windows/macOS tray, login and notification code is included in native CI builds and lifecycle checks. Real desktop visibility, notification permissions and mouse behavior on those platforms still require physical testing. Unsigned-release limitations continue to apply.
