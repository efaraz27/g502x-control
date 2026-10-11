A native Rust control panel for the Logitech G502 X Plus, with live device data, a system tray, and battery alerts.

### Downloads

- **Windows:** open `g502x-control-windows-x86_64-setup.exe`. Portable ZIP also available.
- **Mac:** open the DMG for Apple Silicon (`aarch64`) or Intel (`x86_64`) and drag the app to Applications.
- **Linux:** install the `.deb`, or extract the `.tar.gz` and run `sh install.sh`. The README also has a one-command installer.

### What's included

- **Live UI:** battery, DPI, rate, mode and active-profile readings update automatically. Reconnects are retried and unsaved edits stay intact.
- **System tray:** optional background battery monitoring, battery status in the tray menu, quick open/settings actions and a quit action.
- **Login startup:** opt in from Settings to launch quietly when you sign in. No window opens at startup.
- **Low-battery alerts:** native OS notifications at 20% by default, configurable from 5–50%; no alerts while charging or repeated reminders while low.
- **Settings:** control polling, alerts, background monitoring and startup; test notifications from the app.

The existing onboard profiles, button/G-Shift assignments, lighting, automatic backups and CLI remain available. Background monitoring and startup are off by default. On Linux, GNOME may need AppIndicator support to show the tray; the battery monitor continues without it.

### Support status

Linux has been physically tested over USB and LIGHTSPEED. Windows and macOS are experimental: automated builds and tests cannot verify mouse communication without hardware. Releases are not publisher-signed or notarized, so Windows/macOS may show first-launch security prompts. See the README for details. No firmware flashing, receiver pairing or multi-step macro editor is included.

`SHA256SUMS.txt` covers every downloadable installer/archive. Source is available with this release. Close other configuration apps while changing settings and keep the mouse connected during saves.
