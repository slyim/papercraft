# PaperCraft

<img src="assets/papercraft-logo.png" alt="PaperCraft engraved paper-crane logo" width="128">

An original, local design editor built entirely in Rust with egui/eframe and Phosphor Icons. Paper research informs the feature contracts; application code and starter artwork are original. Existing JavaScript is used only for REA research tooling.

## Download

The [PaperCraft prerelease](https://github.com/slyim/papercraft/releases/tag/v0.1.1) includes an Apple Silicon macOS build and SHA-256 checksums. Unzip it and move `PaperCraft.app` to Applications. The app is ad-hoc signed and is not notarized; macOS may require approval in System Settings → Privacy & Security after the first launch attempt.

This is an early preview. Windows, Linux and Intel macOS downloads are not provided.

## Run the editor

Install the stable Rust toolchain using [rustup](https://rustup.rs/), then run:

```sh
git clone https://github.com/slyim/papercraft.git
cd papercraft
cargo run --locked
# Open an existing document:
cargo run --locked -- /path/to/design.papercraft
```

Verified locally on Apple Silicon macOS with Rust 1.99.1. A prior Linux aarch64 rendering check is recorded in the verification notes; Linux release builds, Windows and Intel macOS remain unverified. The first build downloads and compiles the locked dependencies; Node.js is unnecessary for the application.

The editor includes one artboard, rectangle/text layers, a layer list and property inspector, dragging, bottom-right resizing, pan/zoom, ordering, hide/lock, undo/redo, validated `.papercraft` JSON files, and standalone HTML/SVG export. New/Open/Close prompt before discarding unsaved changes. macOS Quit uses the same preservation decision, including while minimized. Save writes a temporary file before replacing the destination.

PaperCraft was previously named Crumbs. Existing `.crumbs` documents still open and save in place; Save As defaults to `.papercraft`. The document schema remains version 1, so both extensions contain the same format. Recent-file history continues to use the legacy Crumbs configuration folder so both versions retain access to it. The older v0.1.0 release remains available under its original name.

The window has a Dashboard tab and one document tab. The dashboard lists recently opened and saved files (paths only, stored in a local `recents.json` under the platform configuration folder), with search, grid/list views, document thumbnails and New file. The editor has a file menu on the document name, a Pages/Layers panel, a tool rail, a zoom menu, Export, and an inspector with page color, artboard and layer properties.

Tools: **V** select, **H** hand, **R** rectangle, **T** text. With a creation tool, click inside the artboard to place a layer or drag to size it; the tool then returns to selection. **Esc** leaves a tool, then clears the selection. **F** fits the artboard, arrow keys move a selection, and **Delete** removes an unlocked layer. Command/Ctrl shortcuts cover New, Open, Save, Undo, Redo, Duplicate and dashboard search. The file menu lists Save As, Export HTML/SVG, shortcuts and Close file; the Export button also offers both formats.

HTML export uses positioned rectangles and wrapped text. SVG exports the artboard background, visible rectangles and editable plain text with explicit line breaks, alpha and clipping. It excludes the surrounding page color and does not reproduce automatic wrapping. Receiving applications resolve fonts; typography and glyph coverage differ from egui. A committed exchange fixture was imported and rendered in VectorCraft 0.13.1 and FigCraft 0.7.0; this is bounded exchange, not a lossless document round trip. Nested layouts, image/vector editing, SVG import, Paper file compatibility, arbitrary CSS editing, collaboration and a local MCP interface are deferred.

## Development and verification

```sh
cargo fmt --check
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
cargo run --locked --example verify_native
# Other states and sizes (editor, selected, tool, exchange, export, quit, dashboard, list, empty):
cargo run --locked --example verify_native -- target/verification dashboard 960x640
cargo run --locked --example verify_native -- target/verification selected 1470x922 hover=90,305
cargo run --locked --example verify_native -- target/verification/exchange exchange
cargo run --locked --example verify_native -- target/verification/quit quit 960x640
```

The native verification example opens a real egui window and captures stable renderer output. A separate model preflight creates/edits/undoes/redoes/saves/reopens/exports HTML and SVG; the `quit` scenario uses simulated menu/minimize dispatch through the actual native event loop. Outputs are ignored under `target/verification/`. Model and simulated-input tests cover history, validation, file preservation and editing gestures. Native file-dialog clicks and physical keyboard Quit still need a manual check.

- [Evidence-to-implementation process and feature contracts](docs/implementation-process.md)
- [Verification record](docs/verification.md)
- [Third-party notices](THIRD_PARTY_NOTICES.md)

## ArtCraft interoperability

PaperCraft is an independently maintained companion project targeting interoperability with the [Craft app ecosystem](https://getartcraft.com/apps). It is not an official ArtCraft app or currently included in Craft Launcher.

The [integration plan](docs/artcraft.md) records the implemented SVG exchange, then proposes PNG/image exchange and shared command-based automation. The [source review](docs/craft-source-review.md) pins inspected Craft revisions and licenses. macOS uses the audited `craft-quit` library for native Quit handling; PaperCraft retains its document model, atomic file writer, Phosphor controls and branding. The [ranked gap assessment](docs/gap-assessment.md) separates current behavior from proposals.

## License

PaperCraft 0.1.1 and its original code, documentation and generated logo use the [MIT license](LICENSE-MIT). The native Craft apps offer MIT as one of their license options. The previously published v0.1.0 dual-license grant remains available with that release.

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
