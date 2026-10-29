# PaperCraft

<img src="assets/papercraft-logo.png" alt="PaperCraft engraved paper-crane logo" width="128">

An original, local design editor built entirely in Rust with egui/eframe and Phosphor Icons. Paper research informs the feature contracts; application code and starter artwork are original. Existing JavaScript is used only for REA research tooling.

## Download

The [PaperCraft v0.1.3 prerelease](https://github.com/slyim/papercraft/releases/tag/v0.1.3) includes platform installers and portable downloads. Rust and Node.js are not required.

| Platform | Downloads |
|---|---|
| Linux x86_64 | [Debian package](https://github.com/slyim/papercraft/releases/download/v0.1.3/papercraft-0.1.3-linux-x86_64.deb) · [tar.gz](https://github.com/slyim/papercraft/releases/download/v0.1.3/papercraft-0.1.3-linux-x86_64.tar.gz) |
| Linux ARM64 | [Debian package](https://github.com/slyim/papercraft/releases/download/v0.1.3/papercraft-0.1.3-linux-arm64.deb) · [tar.gz](https://github.com/slyim/papercraft/releases/download/v0.1.3/papercraft-0.1.3-linux-arm64.tar.gz) |
| Mac Apple Silicon | [DMG](https://github.com/slyim/papercraft/releases/download/v0.1.3/papercraft-0.1.3-macos-arm64.dmg) · [ZIP](https://github.com/slyim/papercraft/releases/download/v0.1.3/papercraft-0.1.3-macos-arm64.zip) · [tar.gz](https://github.com/slyim/papercraft/releases/download/v0.1.3/papercraft-0.1.3-macos-arm64.tar.gz) |
| Mac Intel | [DMG](https://github.com/slyim/papercraft/releases/download/v0.1.3/papercraft-0.1.3-macos-x86_64.dmg) · [ZIP](https://github.com/slyim/papercraft/releases/download/v0.1.3/papercraft-0.1.3-macos-x86_64.zip) · [tar.gz](https://github.com/slyim/papercraft/releases/download/v0.1.3/papercraft-0.1.3-macos-x86_64.tar.gz) |
| Windows x86_64 | [Setup installer](https://github.com/slyim/papercraft/releases/download/v0.1.3/papercraft-0.1.3-windows-x86_64-setup.exe) · [portable ZIP](https://github.com/slyim/papercraft/releases/download/v0.1.3/papercraft-0.1.3-windows-x86_64.zip) |

On macOS, open the DMG and drag `PaperCraft.app` to Applications. On Windows, run the per-user setup installer; administrator rights are unnecessary. On Debian/Ubuntu, install the matching package with `sudo apt install ./papercraft-0.1.3-linux-x86_64.deb` (use `arm64` on ARM). Portable archives remain available: extract and run the included app/executable. Linux requires glibc 2.35 or later, libxkbcommon-x11, a desktop session and OpenGL/EGL drivers; file dialogs use a desktop portal. macOS apps are ad-hoc signed and not notarized; macOS may require approval in System Settings → Privacy & Security after the first launch attempt. Windows builds and installers are unsigned.

[SHA-256 checksums](https://github.com/slyim/papercraft/releases/download/v0.1.3/SHA256SUMS.txt) cover every installer and archive. This remains an early preview; native file-dialog and physical-input checks are incomplete. The [release workflow](docs/releasing.md) records build targets and packaging checks.

## Run the editor

Install the stable Rust toolchain using [rustup](https://rustup.rs/), then run:

```sh
git clone https://github.com/slyim/papercraft.git
cd papercraft
cargo run --locked
# Open an existing document:
cargo run --locked -- /path/to/design.papercraft
```

The release workflow builds and runs Rust tests on Linux x86_64/ARM64, macOS Apple Silicon/Intel and Windows x86_64 with Rust 1.99.1. Native renderer checks run on Linux and macOS; Windows native interaction remains unverified. Local Apple Silicon checks and remaining gaps are recorded in the [verification notes](docs/verification.md). The first source build downloads and compiles the locked dependencies; Node.js is unnecessary for the application.

The editor includes one artboard, rectangle/text layers with inline text editing, independent document tabs and layer renaming, color tokens, multi-selection with align/distribute, a layer list and property inspector, dragging, marquee selection, resizing from any corner or edge (one layer or a group), snapping with guides, pan/zoom, ordering, hide/lock, undo/redo, validated `.papercraft` JSON files, standalone HTML/SVG/PNG export, and a headless command line with a local MCP server for agents. New/Open preserve existing tabs; closing a dirty tab prompts before discarding it, and Quit checks every dirty tab, including while minimized. Save writes a temporary file before replacing the destination. The v0.1.3 preview includes these integrated features; older v0.1.2 downloads predate the tab/tool and canvas/snapping increments.

PaperCraft was previously named Crumbs. Existing `.crumbs` documents still open and save in place; Save As defaults to `.papercraft`. Documents without color tokens are still written as version 1. Documents with tokens use version 2, which older PaperCraft rejects without modifying the file; the first in-place save that would upgrade a version 1 file asks first. See [document format](docs/document-format.md) for downgrade and rollback. Recent-file history continues to use the legacy Crumbs configuration folder so both versions retain access to it. The older v0.1.0 release remains available under its original name.

The window has a Dashboard tab and independent document tabs. Each document retains its own history, selection, file path, view and last-used text style. Opening an already-open file activates its tab without reloading unsaved work. The dashboard lists recently opened and saved files (paths only, stored in a local `recents.json` under the platform configuration folder), with search, grid/list views, document thumbnails and New file. The editor has a file menu on the document name, a Pages/Layers panel, a tool rail, a zoom menu, Export, and an inspector with page color, artboard and layer properties.

**Color tokens.** The Theme tab lists named color tokens: add, rename, edit RGBA values and delete. The four-circle button beside any color property (layer fill or text color, artboard, page) binds a token, or creates a token from the current color; a bound property shows the token name and a detach button. Editing a token updates every bound property. Detaching or deleting a token keeps each bound property's current color as a plain value. Each completed token edit, binding change or deletion is one undo step. HTML export writes tokens as `--color-<name>` custom properties; SVG export writes the resolved colors.

**Selection and arrangement.** Shift-click on the canvas or in the layer list adds or removes layers (⌘/Ctrl-click also works in the list). Dragging any selected layer moves the visible, unlocked selection together. The inspector's Arrange section and ⌥A/⌥H/⌥D, ⌥W/⌥V/⌥S align; ⌃⌥H/⌃⌥V distribute. Two or more layers align to their shared bounds; a single layer aligns to the artboard. Distribution gives three or more layers equal gaps and keeps the outer layers in place. Hidden and locked layers can be selected in the layer list but are never moved, aligned or distributed. ⌘/Ctrl+A selects visible, unlocked layers; ⇧⌘H and ⇧⌘L hide or lock the selection.

**Marquee, resize and snapping.** Dragging on empty canvas selects the visible, unlocked layers the marquee touches; Shift adds them to the selection. Corner handles and edges resize the selected layer; with several layers selected, the shared bounds scale them together. Holding Shift during a corner resize keeps the proportions. Moving and resizing snap edges and centers to the artboard and other visible layers within 5 screen points, and a guide marks each match. Hold ⌘/Ctrl while dragging to bypass snapping, or turn it off with the magnet button in the tool rail. Each gesture is one undo step.

Tools: **V** select, **H** hand, **R** rectangle, **T** text. With a creation tool, click inside the artboard to place a layer or drag to size it; the tool then returns to selection. Shift constrains rectangles to squares; Option/Alt draws from the center. Text placement starts inline editing; click-placed text follows its content width, while dragged text retains its width. Enter or double-click edits an existing text layer; double-click a layer name to rename it. **Esc** ends an edit, leaves a tool, then clears the selection. **F** fits the artboard, arrow keys move a selection, and **Delete** removes the unlocked selected layers. Command/Ctrl shortcuts cover New, Open, Save, Undo, Redo, Duplicate and dashboard search. The file menu lists Save As, Export HTML/SVG/PNG, shortcuts and Close file; the Export button offers the same formats.

HTML export uses positioned rectangles and wrapped text. SVG exports the artboard background, visible rectangles and editable plain text with explicit line breaks, alpha and clipping. It excludes the surrounding page color and does not reproduce automatic wrapping. Receiving applications resolve fonts; typography and glyph coverage differ from egui. A committed exchange fixture was imported and rendered in VectorCraft 0.13.1 and FigCraft 0.7.0; this is bounded exchange, not a lossless document round trip. PNG export (1× or 2× in the editor, 0.1–8× from the command line) renders the artboard with the canvas's own text layout and fonts, independent of window size, zoom and display density. Nested layouts, image/vector editing, SVG import, Paper file compatibility, arbitrary CSS editing and collaboration are deferred.

**Command line and MCP.** `papercraft inspect FILE` prints the document as deterministic JSON; `papercraft export FILE --format html|svg|png --output PATH [--scale N]` exports without opening a window; `papercraft mcp` serves the Model Context Protocol on stdin/stdout. MCP clients open, inspect, edit, render, export and save documents through the editor's own transactions: each edit batch is validated as a whole and becomes one undo step, or is rejected without changes; `expected_revision` guards against stale state; every result reports the revision and whether it is saved. Commands never write the source and keep an existing destination on failure. Exit status is 0 for success, 1 for document or file errors and 2 for usage errors. See [agent interface](docs/agent-interface.md).

## Development and verification

```sh
cargo fmt --check
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
cargo run --locked --example verify_native
# Other states: verify_native --help (includes text, marquee, resize, snapping, long-text, text-token and text-tabs).
cargo run --locked --example verify_native -- target/verification dashboard 960x640
cargo run --locked --example verify_native -- target/verification selected 1470x922 hover=90,305
cargo run --locked --example verify_native -- target/verification/exchange exchange
cargo run --locked --example verify_native -- target/verification/quit quit 960x640
```

The native verification example opens a real egui window and captures stable renderer output. Separate model preflights create/edit/undo/redo/save/reopen/export HTML and SVG, and run a token → arrange → undo/redo → save → reopen → export workflow; the `quit` scenario uses simulated menu/minimize dispatch through the actual native event loop. Outputs are ignored under `target/verification/`. Model and simulated-input tests cover history, validation, file preservation and editing gestures. `cargo run --release --example bench` reports headless frame times for 1,000–10,000 layers. Native file-dialog clicks and physical keyboard Quit still need a manual check.

- [Evidence-to-implementation process and feature contracts](docs/implementation-process.md)
- [Verification record](docs/verification.md)
- [Document format, compatibility and rollback](docs/document-format.md)
- [Agent interface: CLI and local MCP](docs/agent-interface.md)
- [Third-party notices](THIRD_PARTY_NOTICES.md)

## ArtCraft interoperability

PaperCraft is an independently maintained companion project targeting interoperability with the [Craft app ecosystem](https://getartcraft.com/apps). It is not an official ArtCraft app or currently included in Craft Launcher.

The [integration plan](docs/artcraft.md) records the implemented SVG exchange, then proposes PNG/image exchange and shared command-based automation. The [source review](docs/craft-source-review.md) pins inspected Craft revisions and licenses. macOS uses the audited `craft-quit` library for native Quit handling; PaperCraft retains its document model, atomic file writer, Phosphor controls and branding. The [ranked gap assessment](docs/gap-assessment.md) separates current behavior from proposals.

## License

PaperCraft and its original code, documentation and generated logo use the [MIT license](LICENSE-MIT). The native Craft apps offer MIT as one of their license options. The previously published v0.1.0 dual-license grant remains available with that release.

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
