# Architecture

| Source | Responsibility |
| --- | --- |
| `src/main.rs` | CLI, argument validation, profile import/export |
| `src/gui.rs` | UI state and off-thread device jobs; explicit apply/save flows |
| `src/preferences.rs` | Atomic user preferences and native per-user startup entries |
| `src/alerts.rs` | Native desktop notifications, persistent deduplication and cooldown |
| `src/monitor.rs` | Singleton background battery worker, health and shutdown control |
| `src/tray.rs` | Native tray menu and lightweight platform event loop |
| `src/design.rs` | Palette, fonts, components, icons and vector mouse illustration |
| `src/device.rs` | Identity/capability discovery, settings, memory transactions, backups and process lock |
| `src/profile.rs` | Byte-preserving format-5 profiles, bindings, lighting slots and CRCs |
| `src/transport.rs` | HID++ packet framing, reply matching and deadlines |
| `src/transport/linux.rs` | Linux sysfs discovery and non-blocking hidraw I/O |
| `src/transport/hid.rs` | Native Windows HID and shared macOS HIDAPI access |

The GUI worker owns device operations; the window stays responsive. Each operation takes a file lock and opens the device anew. Discovery prefers USB cable, then the Linux paired mouse node, then receiver. Feature and identity validation happens before settings can be accessed. The Windows backend selects the vendor collection for HID++ long reports, not the OS pointer collection. macOS enables HIDAPI's shared-device feature.

The backup format remains schema 1 across platforms. File creation uses create-new semantics; Unix files are private and reject symlinks. Windows uses the current user's Local AppData permissions. On Unix, parent directories are synced after writing a backup. The process lock uses Rust's cross-platform file locking and is held until the device operation completes.

Flash operations back up all writable sectors, validate old state, preserve unrelated bytes, skip unchanged writes and verify readback. There is no firmware updater, receiver pairing or interception of OS mouse input. An opt-in user process provides background battery monitoring and a tray menu.

Protocol references and exact reference revisions are recorded in `NOTICE`. Linux hardware validation predates the portability layer; see `VALIDATION.md` for the distinction between hardware tests and build coverage.

The GUI serializes explicit actions and automatic reads on one worker. Polls reuse feature discovery after rechecking identity, release the HID lock between reads, refresh status every 2 seconds and profiles every 30 seconds, and back off when disconnected. A separate editor baseline survives incoming snapshots, preserving stale-write detection. Unapplied live inputs update only when they still equal their previously observed values.

The background process owns `monitor.lock`, checks battery every 60 seconds and checks preferences/shutdown every second. Linux's tray uses KSNI/D-Bus without GTK or a GPU; Windows/macOS run a native event loop without a rendering window. `gui.lock` and a small local open request focus the existing foreground window. A separate alert lock serializes foreground/background notification decisions. Preferences and alert state use private temporary files and atomic replacement. Native startup is registered only after explicit opt-in.

For isolated development and lifecycle tests, `G502X_STATE_DIR` overrides the app state directory and `G502X_AUTOSTART_DIR` redirects Linux/macOS startup files. Never use different state directories for concurrently modifying the same physical mouse: the operation lock is inside that directory.
