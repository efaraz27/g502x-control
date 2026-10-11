# Architecture

| Source | Responsibility |
| --- | --- |
| `src/main.rs` | CLI, argument validation, profile import/export |
| `src/gui.rs` | UI state and off-thread device jobs; explicit apply/save flows |
| `src/design.rs` | Palette, fonts, components, icons and vector mouse illustration |
| `src/device.rs` | Identity/capability discovery, settings, memory transactions, backups and process lock |
| `src/profile.rs` | Byte-preserving format-5 profiles, bindings, lighting slots and CRCs |
| `src/transport.rs` | HID++ packet framing, reply matching and deadlines |
| `src/transport/linux.rs` | Linux sysfs discovery and non-blocking hidraw I/O |
| `src/transport/hid.rs` | Native Windows HID and shared macOS HIDAPI access |

The GUI worker owns device operations; the window stays responsive. Each operation takes a file lock and opens the device anew. Discovery prefers USB cable, then the Linux paired mouse node, then receiver. Feature and identity validation happens before settings can be accessed. The Windows backend selects the vendor collection for HID++ long reports, not the OS pointer collection. macOS enables HIDAPI's shared-device feature.

The backup format remains schema 1 across platforms. File creation uses create-new semantics; Unix files are private and reject symlinks. Windows uses the current user's Local AppData permissions. On Unix, parent directories are synced after writing a backup. The process lock uses Rust's cross-platform file locking and is held until the device operation completes.

Flash operations back up all writable sectors, validate old state, preserve unrelated bytes, skip unchanged writes and verify readback. There is no firmware updater, receiver pairing, interception of OS mouse input or persistent daemon.

Protocol references and exact reference revisions are recorded in `NOTICE`. Linux hardware validation predates the portability layer; see `VALIDATION.md` for the distinction between hardware tests and build coverage.
