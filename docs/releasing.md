# Release downloads

Run **Build release downloads** from the repository's Actions page. The manual workflow builds the checked-out commit on five native GitHub-hosted runners with Rust 1.99.1 and locked dependencies:

| Platform | Download | Build host |
|---|---|---|
| Linux x86_64 | tar.gz | Ubuntu 22.04 |
| Linux ARM64 | tar.gz | Ubuntu 22.04 ARM64 |
| macOS Apple Silicon | ZIP and tar.gz containing PaperCraft.app | macOS 15 ARM64 |
| macOS Intel | ZIP and tar.gz containing PaperCraft.app | macOS 15 Intel |
| Windows x86_64 | ZIP containing papercraft.exe | Windows Server 2022 |

Linux binaries require glibc 2.35 or later, libxkbcommon-x11, a graphical desktop and OpenGL/EGL libraries. File dialogs use an XDG desktop portal. Windows release builds use the static MSVC runtime and the GUI subsystem. macOS bundles are ad-hoc signed; Developer ID signing and notarization are not configured.

Each job runs formatting, all Rust tests, Clippy with warnings denied, and a release build. Linux and macOS also run the native verification example; Windows interaction with a real native window remains a manual check. `packaging/package.py` collects dependency licenses at their Cargo-recorded revisions, bundled font/icon and Rust-library notices, validates the executable architecture, creates archives, then checks their executable bytes, member paths and required notices. Each artifact contains platform-specific SHA-256 checksums. Packaging uses Python's standard library; application code remains Rust.

To prepare a new version, update `Cargo.toml`, the root package entry in `Cargo.lock`, and both version fields in `packaging/macos/Info.plist`. Run the workflow on the committed change. Download all five successful artifacts, check the archives and combine their checksum files into `SHA256SUMS.txt`. Publish the archives, checksums and `LICENSE-MIT` in a new GitHub release targeting that exact workflow commit. The workflow itself cannot publish or modify repository contents. Do not replace an existing release's downloads with a different build.

Keep the early-preview designation until the remaining manual checks in [verification](verification.md) are completed. No installer, automatic updater, Linux package repository or notarization service is part of this packaging workflow.
