# Verification record

18 October 2026. Host: Apple Silicon macOS. Rust 1.99.1; eframe/egui 0.36.2; egui-phosphor 0.14.0; REA 6.7.0.

## Passed

- `cargo fmt --check` and `cargo clippy --locked --all-targets -- -D warnings`.
- `cargo test --locked`: three integration tests. Coverage includes creation, editing, duplication, ordering, deletion, undo/redo, save/open equality, escaped Unicode/text export, hidden-layer omission, locked-layer guards, invalid geometry/version/IDs, invalid JSON, and preservation of an existing file after a rejected save.
- Simulated egui input at 1380×860 and 960×640: rectangle shortcut, drawing output, canvas selection, drag and bottom-right resize at each fitted zoom. One undo reverts each complete gesture; redo restores it. Delete during an active gesture does not remove its target.
- `cargo run --locked --example verify_native`: an actual native OpenGL window rendered successfully; its public screenshot event produced visible content. The example also saved/reopened the starter `.crumbs` document and wrote its HTML export.
- Native screenshot inspected after correcting layer-row label/button clipping. `target/verification/crumbs-desktop.png` contains the final reviewed layout.
- Export opened through the available Chrome connection from a loopback server. The DOM reports a 960×640 artboard and zero script elements; no browser warning/error logs were recorded. The HTML request returned 200; the optional favicon request returned 404. There was no browser connection failure. `target/verification/crumbs-html.png` records its appearance. Browser and egui font metrics differ visibly.
- `npm test`: the REA integration check connects using the project configuration, verifies the pinned server version, and verifies scoped analysis and integrity-mismatch handling. A separate live MCP session check also returned version 6.7.0.
- Phosphor and egui-phosphor MIT notices retained in `licenses/`. Paper research remains outside application code.

Generated verification files are ignored by Git. Regenerate native evidence using the example above. Browser inspection is a separate check against its generated HTML file.

## Dashboard and editor chrome (18 October 2026)

Host: Linux aarch64 container, Rust 1.97.0, Xvfb with Mesa OpenGL. The macOS host was not used for this pass.

- `cargo fmt --check`, `cargo clippy --locked --all-targets -- -D warnings` and `cargo test --locked` pass: 3 unit and 9 integration tests.
- New focused tests: recent-file ordering, deduplication, cap, persistence and rejection of invalid lists; relative edit labels; hexadecimal color parsing; page color compatibility with files that omit it and its HTML export; positioned layer creation (rounding, clamping, one history step); click and drag creation with the rectangle/text tools, Escape and outside-artboard starts; side-panel width stability over frames at 1380×860 and 960×640; dashboard search, opening a recent file through the unsaved-changes prompt (Cancel and Discard), and closing the document tab; page color hex editing as one undo step without deleting layers.
- Native screenshots through `examples/verify_native.rs` at 1470×922 and 960×640: editor, selected layer with row hover controls, active rectangle tool with rail hover, dashboard grid with card hover, list view with row hover, empty dashboard without a document tab. Compared with the supplied references for panel widths (240/180/40/280 pt), tab strip height (41 pt), colors and row metrics.
- Account, team, invitation, learning, sharing, MCP, vector, frame, component and image-generation controls in the references are absent: Crumbs has no backend or those model features. Theme and adding pages are shown disabled with explanations.

## Not verified

macOS title-bar integration (tab strip inset beside the window controls, window dragging from the strip; the window controls stay at the system position and are not vertically centered on the 41 pt strip), native file-dialog clicks, unsaved-change dialog choices, a complete manual editing session, screen-reader interaction, and every keyboard shortcut have not been exercised end to end. Model-level file safety and supported pointer gestures have automated coverage.

Windows and Intel macOS builds, Linux release builds, large-document performance, font coverage for every writing system, and exact browser typography are unverified. A prior Linux aarch64 development rendering check is recorded above.

Authenticated Paper editing and runtime parity remain unknown. Static evidence established the shipped shell and bridge, not the hosted editor internals. Nested layouts, images, SVG, multiple artboards, arbitrary CSS editing, Paper document compatibility, collaboration and a local MCP interface are deferred.

## First prerelease (18 October 2026)

Host: Apple Silicon macOS 27.0.1, Rust 1.99.1. Version: 0.1.0, marked as a GitHub prerelease.

- Formatting, all 3 unit and 9 integration tests, Clippy with warnings rejected, and the research tooling integration test pass. The research test now starts the pinned package directly and does not require a checkout-specific agent configuration.
- `cargo build --release --locked` succeeds. The executable is Mach-O arm64, links only system libraries/frameworks, and declares macOS 11.0 as its minimum target. Runtime compatibility was checked only on macOS 27.0.1.
- The native verification example rendered the editor at 1380×860 and dashboard at 960×640. Screenshots were inspected. Its save/reopen and HTML export assertions pass.
- `Crumbs.app` uses the original release executable and `packaging/macos/Info.plist`. Launch Services opened the packaged app. Ad-hoc signature validation passes; Apple Developer ID signing and notarization are not configured.
- The package includes notices for 137 normal/build dependency packages, bundled font/icon notices, and Rust standard-library notices. Package metadata and recovered upstream notice references accompany the license files. The application license was undecided at initial publication; the later license decision is recorded below.
- Local agent configuration, raw research, dependencies and build output are excluded from Git.

Native file-dialog interactions, a complete manual editing session, screen-reader interaction and macOS approval prompts on a separate downloaded installation remain unverified. Finder document associations and automatic updates are not configured.

## License and ecosystem alignment (18 October 2026)

Original Crumbs code and documentation, including original code in v0.1.0, are now offered under `MIT OR Apache-2.0`. Third-party notices retain their own terms. Crumbs remains an independent companion project; no official catalog inclusion is claimed.

Offline Cargo metadata, npm package/lockfile metadata, license texts and relative documentation links were checked. Application code, application dependencies and the existing release archive are unchanged. No additional application tests were run for this documentation and metadata change.

The [integration plan](artcraft.md) cites checked upstream revisions and separates documented interfaces from proposed implementations. No cross-app editing, rendering, automation or installation was tested; adapters remain unimplemented.
