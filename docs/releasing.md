# Release installers and downloads

Run **Build release downloads** from the repository's Actions page. The manual workflow builds the checked-out commit on five native GitHub-hosted runners with Rust 1.99.1 and locked dependencies:

| Platform | Download | Build host |
|---|---|---|
| Linux x86_64 | Debian `.deb` and portable tar.gz | Ubuntu 22.04 |
| Linux ARM64 | Debian `.deb` and portable tar.gz | Ubuntu 22.04 ARM64 |
| macOS Apple Silicon | DMG, ZIP and tar.gz containing PaperCraft.app | macOS 15 ARM64 |
| macOS Intel | DMG, ZIP and tar.gz containing PaperCraft.app | macOS 15 Intel |
| Windows x86_64 | Per-user setup `.exe` and portable ZIP | Windows Server 2022 |

Linux binaries require glibc 2.35 or later, libxkbcommon-x11, a graphical desktop and OpenGL/EGL libraries. File dialogs use an XDG desktop portal. Debian packages declare native runtime dependencies; install them with `sudo apt install ./papercraft-<version>-linux-<architecture>.deb`. Other distributions can use the portable archive with the documented libraries. Windows setup installs for the current user without administrator rights and provides an uninstaller; portable builds remain available. Windows release builds use the static MSVC runtime and the GUI subsystem. macOS DMGs offer PaperCraft.app and an Applications shortcut. Bundles are ad-hoc signed; Developer ID signing and notarization are not configured. Windows downloads are unsigned.

Each release job runs formatting, all Rust tests, Clippy with warnings denied, and a release build. Linux and macOS also run asserted native text/marquee/resize/snapping checks; macOS checks dirty Quit. `packaging/package.py` collects dependency licenses at their Cargo-recorded revisions, bundled font/icon and Rust-library notices, validates executable architecture, creates installers/archives, verifies their payloads and emits platform-specific SHA-256 checksums. Packaging uses Python and native platform tools; application code remains Rust. Windows requires NSIS 3.11, macOS uses `hdiutil`, and Linux uses `dpkg-deb`. Automatic PR/main checks in `.github/workflows/ci.yml` complement the manually dispatched five-platform packaging run.

To prepare a new version, update `Cargo.toml`, its root package in `Cargo.lock`, and the version/build fields in `packaging/macos/Info.plist`. Add `docs/release-notes-v<version>.md`, pass local gates/native checks, and push main. Dispatch the exact commit with **Publish a new preview release** enabled. After all five native jobs pass, publication verifies all 12 payload checksums and publishes 14 assets as a new prerelease on that exact source. It refuses an existing tag and never replaces assets. Only the publication job has contents-write permission; ordinary builds remain read-only and publication defaults off.

Keep the early-preview designation until the remaining manual checks in [verification](verification.md) are completed. Installer payload/lifecycle checks do not establish physical-input, native-file-dialog or screen-reader coverage. No automatic updater, Linux package repository, signing service or notarization service is included.
