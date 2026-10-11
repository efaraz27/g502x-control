# Security reports

Do not publish credentials, device backups, serial numbers or private diagnostics in an issue. For a security issue, use the repository's **Security → Report a vulnerability** feature. Include a minimal reproduction and affected version, without private device data.

This application accesses HID configuration and onboard memory as the current user. It installs no privileged daemon. The optional Linux udev rule grants the active local session access only to the supported mouse/receiver IDs. Windows/macOS support is experimental. Releases are currently not publisher-signed/notarized; compare release SHA-256 checksums before installing downloaded files.
