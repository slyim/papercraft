# Crumbs

An original, local design editor built entirely in Rust with egui/eframe and Phosphor Icons. Paper research informs the feature contracts; application code and starter artwork are original. Existing JavaScript is used only for REA research tooling.

## Download

The [first prerelease](https://github.com/slyim/crumbs/releases/tag/v0.1.0) includes an Apple Silicon macOS build and SHA-256 checksums. Unzip it and move `Crumbs.app` to Applications. The app is ad-hoc signed and is not notarized; macOS may require approval in System Settings → Privacy & Security after the first launch attempt.

This is an early preview. Windows, Linux and Intel macOS downloads are not provided.

## Run the editor

Install the stable Rust toolchain using [rustup](https://rustup.rs/), then run:

```sh
git clone https://github.com/slyim/crumbs.git
cd crumbs
cargo run --locked
# Open an existing document:
cargo run --locked -- /path/to/design.crumbs
```

Verified locally on Apple Silicon macOS with Rust 1.99.1. A prior Linux aarch64 rendering check is recorded in the verification notes; Linux release builds, Windows and Intel macOS remain unverified. The first build downloads and compiles the locked dependencies; Node.js is unnecessary for the application.

The first slice includes one artboard, rectangle/text layers, a layer list and property inspector, dragging, bottom-right resizing, pan/zoom, ordering, hide/lock, undo/redo, validated `.crumbs` JSON files, and standalone HTML export. New/Open/Close prompt before discarding unsaved changes. Save writes a temporary file before replacing the destination.

The window has a Dashboard tab and one document tab. The dashboard lists recently opened and saved files (paths only, stored in a local `recents.json` under the platform configuration folder), with search, grid/list views, document thumbnails and New file. The editor has a file menu on the document name, a Pages/Layers panel, a tool rail, a zoom menu, Export, and an inspector with page color, artboard and layer properties.

Tools: **V** select, **H** hand, **R** rectangle, **T** text. With a creation tool, click inside the artboard to place a layer or drag to size it; the tool then returns to selection. **Esc** leaves a tool, then clears the selection. **F** fits the artboard, arrow keys move a selection, and **Delete** removes an unlocked layer. Command/Ctrl shortcuts cover New, Open, Save, Undo, Redo, Duplicate and dashboard search. The file menu lists Save As, Export HTML, shortcuts and Close file.

HTML export uses positioned rectangles and text. Browser typography can differ from egui. Nested layouts, images, SVG, Paper file compatibility, arbitrary CSS editing, collaboration and a local MCP interface are deferred.

## Development and verification

```sh
cargo fmt --check
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
cargo run --locked --example verify_native
# Other states and sizes (editor, selected, tool, dashboard, list, empty):
cargo run --locked --example verify_native -- target/verification dashboard 960x640
cargo run --locked --example verify_native -- target/verification selected 1470x922 hover=90,305
```

The native verification example opens a real egui window, captures the renderer output, saves and reopens the starter document, exports HTML, and closes its own window. Outputs are ignored under `target/verification/`. The model and simulated-input tests cover history, validation, file preservation and editing gestures. Native file-dialog interactions still need a manual check.

- [Evidence-to-implementation process and feature contracts](docs/implementation-process.md)
- [Verification record](docs/verification.md)
- [Third-party notices](THIRD_PARTY_NOTICES.md)

## ArtCraft interoperability

Crumbs is an independently maintained companion project targeting interoperability with the [Craft app ecosystem](https://getartcraft.com/apps). It is not an official ArtCraft app or currently included in Craft Launcher.

The [integration plan](docs/artcraft.md) starts with SVG exchange for VectorCraft and FigCraft, then PNG/image exchange for PhotoCraft and motion tools, followed by shared command-based automation. These integrations are planned; the current release exports HTML only. The plan records the upstream interfaces, shared Rust libraries, acceptance checks and launcher enrollment requirements.

## License

Original Crumbs code and documentation are available under either the [MIT license](LICENSE-MIT) or [Apache License 2.0](LICENSE-APACHE), at your option. This grant also covers the original Crumbs code in the v0.1.0 release. This matches the native Craft apps' `MIT OR Apache-2.0` license choice.

Third-party tooling, fonts and icons retain their own licenses; see [third-party notices](THIRD_PARTY_NOTICES.md). License alignment does not establish ArtCraft affiliation or file compatibility.

## Installed tooling

- `rea-agents` **6.7.0**, pinned in `package.json` and `package-lock.json`.
- Matching official workflow: `.agents/skills/reverse-engineer-anything/`, copied from the installed package with its MIT license.
- A repeatable Paper analysis command and one integration test.

REA requires Node.js 22.19+ on the 22.x line, 24.11+ on the 24.x line, or 26+. The installed Node.js 26.12.0 is supported. [REA installation documentation](https://github.com/morluto/rea/blob/main/docs/installation.md)

```sh
npm ci --ignore-scripts --omit=optional
npm test
npm run rea -- --help
```

Optional dependencies, install scripts, and native disassemblers are unnecessary for the selected static JavaScript workflow. Native analysis and instrumentation need their own prerequisites.

The research integration test starts the pinned local REA package, verifies server version 6.7.0, and discovers the analysis tools. Local agent configuration stays outside Git and is unnecessary for running the editor or its tests.

## Analyze Paper

```sh
npm run paper:analyze
# Optional explicit application or ASAR path:
npm run paper:analyze -- /Applications/Paper.app
```

The command inventories the ASAR, selects `package.json` and packaged `src` JavaScript/TypeScript, and writes a new private output directory on every run. Spec files, test directories, dependency source, and `.env` contents are excluded from source analysis. The full inventory still records unselected entries. Paper code is parsed as inert text.

Outputs in `research/private/paper-*/`:

- `manifest.json`: target path, SHA-256, REA version, exact source selection and integrity policy.
- `evidence.json`: complete REA result or failure diagnostics.

The installed Paper 0.5.16 has an ASAR hash contradiction for an unpacked CLI helper. macOS deep signature verification passed outside the sandbox. The initial analysis explicitly retained the contradictory bytes as **observed-untrusted** using:

```sh
npm run paper:analyze -- --record-integrity-mismatch
```

The default remains fail-on-mismatch. The switch records the contradiction; it does not establish equivalence or authenticity for the affected bytes. The integration test verifies both paths and confirms that unselected environment-file content is absent from evidence.

On macOS, REA uses an OS process-ownership helper. Restricted execution can block its compiler cache or process inspection; run through approved execution when necessary. A sandbox failure is distinct from a target-analysis failure.

## Research results

- [Initial Paper analysis](research/paper/analysis.md)
- [Compact evidence summary and IPC references](research/paper/evidence-summary.json)

The completed static pass is retained locally at `research/private/paper-RB6VUg/`. Raw evidence and public reference screenshots are ignored by Git. They are research material, separate from application code.

To update REA, choose an exact release, update the dependency and lockfile, refresh the bundled project skill from that same package, and rerun the test. Preserve the matching license.
