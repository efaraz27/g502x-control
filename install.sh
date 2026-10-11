#!/bin/sh
# Download a prebuilt Linux release. No Rust, root or background service needed.
set -eu
case $(uname -s) in
    Linux) ;;
    *) printf '%s\n' 'Use the Windows installer or macOS DMG: https://github.com/efaraz27/g502x-control/releases/latest' >&2; exit 1 ;;
esac
case $(uname -m) in
    x86_64|amd64) app_arch=x86_64 ;;
    aarch64|arm64) app_arch=aarch64 ;;
    *) printf '%s\n' 'No prebuilt release for this architecture. See the source build guide.' >&2; exit 1 ;;
esac
command -v curl >/dev/null 2>&1 || { printf '%s\n' 'Install curl first.' >&2; exit 1; }
command -v sha256sum >/dev/null 2>&1 || { printf '%s\n' 'Install coreutils (sha256sum) first.' >&2; exit 1; }
app_tmp=$(mktemp -d)
trap 'rm -rf -- "$app_tmp"' EXIT HUP INT TERM
app_release=https://github.com/efaraz27/g502x-control/releases/latest/download
app_asset="g502x-control-linux-$app_arch.tar.gz"
curl --proto '=https' --tlsv1.2 -fsSL "$app_release/$app_asset" -o "$app_tmp/$app_asset"
curl --proto '=https' --tlsv1.2 -fsSL "$app_release/SHA256SUMS.txt" -o "$app_tmp/SHA256SUMS.txt"
# Verify exactly the archive being installed, using its release checksum.
awk -v asset="$app_asset" '$2 == asset {print}' "$app_tmp/SHA256SUMS.txt" > "$app_tmp/selected.sha256"
[ -s "$app_tmp/selected.sha256" ] || { printf '%s\n' 'Release checksum missing.' >&2; exit 1; }
(cd "$app_tmp" && sha256sum --check selected.sha256)
tar -xzf "$app_tmp/$app_asset" -C "$app_tmp"
sh "$app_tmp/g502x-control/install.sh"
