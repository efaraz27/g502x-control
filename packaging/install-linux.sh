#!/bin/sh
set -eu
app_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
# Works both in a source checkout (packaging/) and an extracted release.
if [ -f "$app_dir/../Cargo.toml" ]; then app_dir=$(CDPATH= cd -- "$app_dir/.." && pwd); fi
app_bin=${1:-"$app_dir/g502x-control"}
if [ ! -x "$app_bin" ]; then
    printf '%s\n' 'Binary missing. Build with cargo build --release --locked, then run:' 'sh packaging/install-linux.sh target/release/g502x-control' >&2
    exit 1
fi
app_prefix=${G502X_INSTALL_PREFIX:-"$HOME/.local"}
mkdir -p "$app_prefix/bin"
app_prefix=$(CDPATH= cd -- "$app_prefix" && pwd)
app_tmp_bin=$(mktemp "$app_prefix/bin/.g502x-control.XXXXXX")
trap 'rm -f -- "$app_tmp_bin"' EXIT HUP INT TERM
install -m755 "$app_bin" "$app_tmp_bin"
mv -f -- "$app_tmp_bin" "$app_prefix/bin/g502x-control"
install -Dm644 "$app_dir/packaging/g502x-control.svg" "$app_prefix/share/icons/hicolor/scalable/apps/g502x-control.svg"
install -Dm644 "$app_dir/packaging/70-g502x-control.rules" "$app_prefix/share/g502x-control/70-g502x-control.rules"
mkdir -p "$app_prefix/share/applications"
app_exec=$(printf '%s' "$app_prefix/bin/g502x-control" | sed 's/\\/\\\\/g; s/"/\\"/g; s/`/\\`/g; s/\$/\\$/g; s/%/%%/g')
cat > "$app_prefix/share/applications/g502x-control.desktop" <<DESKTOP
[Desktop Entry]
Type=Application
Name=G502 X Control
Comment=Configure your Logitech G502 X Plus
Exec="$app_exec"
Icon=g502x-control
Terminal=false
Categories=Settings;HardwareSettings;
StartupWMClass=g502x-control
DESKTOP
if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "$app_prefix/share/applications" || true
fi
printf '%s\n' 'Installed. Open G502 X Control from your application menu.'
if [ ! -f /etc/udev/rules.d/70-g502x-control.rules ] && [ ! -f /usr/lib/udev/rules.d/70-g502x-control.rules ]; then
    printf '\n%s\n' 'If the app reports permission denied, enable mouse access once:'
    printf '  sudo install -m644 "%s" /etc/udev/rules.d/\n' "$app_prefix/share/g502x-control/70-g502x-control.rules"
    printf '%s\n' '  sudo udevadm control --reload-rules' 'Then reconnect your mouse or receiver.'
fi
