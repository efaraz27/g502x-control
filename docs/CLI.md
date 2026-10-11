# Command-line interface

The GUI and CLI use the same binary. On Windows, use PowerShell and `./g502x-control.exe`; on macOS the executable is inside `G502 X Control.app/Contents/MacOS/`. Endpoint paths are platform-specific; use `list` to discover them.

## Examples

```sh
# Read-only diagnostics and hardware validation
./g502x-control list
./g502x-control status
./g502x-control check

# Back up before experimenting; the destination must not already exist
./g502x-control backup "$HOME/Downloads/g502x-backup.json"

# Change live settings, then return to onboard mode
./g502x-control mode software
./g502x-control dpi 1600
./g502x-control rate 1000
./g502x-control rgb static 42D6B5
./g502x-control leds FF0000 FF8800 FFFF00 00FF00 00FFFF 0000FF 8800FF FF00FF
./g502x-control mode onboard

# Select saved profile 2, then DPI stage 1
./g502x-control activate 2
./g502x-control stage 1

# Preview profile changes; add --apply only when ready to write
./g502x-control profile 2 --name Linux --dpi 400,800,1600,3200,6400 --default-stage 2
./g502x-control profile 2 --button 10 --binding key:0x06:1
# The second example maps hardware button 10 to Ctrl+C (HID C=0x06).
# Add --g-shift to edit the alternate layer.

./g502x-control export 2 "$HOME/Downloads/profile-2.json"
./g502x-control import "$HOME/Downloads/profile-2.json"
./g502x-control import "$HOME/Downloads/profile-2.json" --apply

# Restore validates without --apply; actual restore makes another backup first
./g502x-control restore "$HOME/Downloads/g502x-backup.json"
./g502x-control restore "$HOME/Downloads/g502x-backup.json" --apply
```

Run `./g502x-control --help` or a subcommand's `--help` for all options. To select a particular discovered endpoint, use `--device PATH` with the path returned by `list`; endpoint paths can change on reconnect. Automatic selection prefers cable, then the receiver's paired mouse node, then the receiver itself.

Bindings: `disabled`, `mouse:1` through `mouse:16`, `key:USAGE:MODIFIER_MASK`, `media:USAGE`, `function:CODE`, or `profile:1` through `profile:5`. Decimal and `0x` hexadecimal numbers are accepted. Modifier bits are Ctrl=1, Shift=2, Alt=4, Super=8; right-hand modifiers use 16, 32, 64, 128. Existing unsupported bindings are kept byte-for-byte until explicitly replaced.
