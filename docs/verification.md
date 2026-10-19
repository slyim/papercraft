# Verification record

18 October 2026. Host: Apple Silicon macOS. Rust 1.99.1; eframe/egui 0.36.2; egui-phosphor 0.14.0; REA 6.7.0.

Entries below retain the capability and verification state at each earlier change. **Release downloads v0.1.2** records the current packaging checks; earlier entries describe their own revisions and platforms. Earlier statements that SVG, tokens or other-platform builds were deferred describe the earlier code.

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

## PaperCraft rename (18 October 2026)

Version 0.1.1 renames the app and Rust package to PaperCraft/papercraft. The current source, documentation and original generated paper-crane logo use MIT; the previously published v0.1.0 dual-license grant is retained with that release. Historical entries above describe the names and licenses at the time of each check.

- Formatting, Clippy with warnings rejected, all 4 unit and 10 integration tests, the research integration test, and the optimized native build pass.
- The legacy-file regression loads the original v0.1.0 starter fixture, saves it as `.papercraft`, verifies document equality, and confirms that the original `.crumbs` bytes remain unchanged. The schema stays at version 1. Existing Save keeps its path; Save As defaults to `.papercraft`. Both extensions are accepted by Open.
- Recent-file storage deliberately retains the old Crumbs folders, avoiding a destructive migration or split history between application versions.
- The embedded PNG decodes at 256×256 for window/dashboard use. The original generated logo and standard macOS ICNS sizes are retained in `assets/`, with the final generation prompt and MIT terms documented there.
- Native editor and dashboard screenshots were inspected at 1380×860 and 960×640. The dashboard shows the new logo/name, and the starter artwork names PaperCraft. Save/reopen and HTML export assertions pass.
- Native rendering, ICNS packaging and the research test require macOS services unavailable in the sandbox. Their checks pass outside it; those sandbox errors are separate from application failures.
- The final `PaperCraft.app` bundle launches without diagnostics. Ad-hoc signature verification passes before and after ZIP extraction, and the extracted executable matches the staged app. The app includes the MIT license, generated ICNS icon and the retained 137 dependency notice sets. The archive's SHA-256 is published in `SHA256SUMS.txt`.

Native file-dialog clicks and a full manual editing session remain unverified. Other-platform release builds, notarization and Craft app interoperability remain outside this rename.

## Reliability and SVG exchange (18 October 2026)

Baseline `bc3c52bf75491510a7d5a7515a5dce5c0bb12f1d`; branch `codex/reliability-svg-exchange`. Host: Apple Silicon macOS, Rust 1.99.1, eframe/egui 0.36.2. The existing worktree was clean before the branch was created. No release, repository transfer or affiliation change was made.

### Defects established before correction

| Defect / cause | Focused proof and correction |
|---|---|
| Valid Unicode layer names could fail duplication: the old name truncation counted characters while document validation limits UTF-8 bytes. | `duplicating_a_valid_unicode_name_keeps_a_valid_name_and_one_history_step` failed with an 85-character/255-byte CJK name. Duplication now reserves the suffix's byte budget and truncates at a valid character boundary. |
| Drag/resize omitted motion delivered with pointer release: only `response.dragged()` applied geometry. | `dragging_and_resizing_include_the_release_position_at_multiple_zoom_levels` demonstrated a partial movement. Release now applies final coordinates before committing; move/resize undo/redo is checked at 1380×860 and 960×640 fitted zooms. |
| Release-frame shortcuts ran before the active gesture committed. | `release_frame_undo_runs_after_the_completed_gesture` failed before correction. Shortcuts on that frame now run after the gesture. A separate Delete regression verifies no stale-target panic and independent move/delete undo/redo steps. |
| Hidden/minimized close bypassed dirty checks: the decision ran only in UI, which eframe skips while hidden. Winit's default macOS Quit also sends `terminate:` directly. | A logic-only minimized close test failed before correction. `App::logic` now cancels dirty close, restores/focuses the window and invokes the existing prompt. macOS uses inspected/pinned `craft-quit` 0.1.0 at `82a24b006eefa036b43c09a67fb9a3fbd17ffe63`, with native Quit interception. The default-Quit cause is source verified; physical Cmd-Q input was not exercised. |

No additional malformed-file, recent-history or selection invalidation defect was demonstrated. Existing validation/preservation regressions continue to pass. Full-document cloning in the inspector and linear dirty comparisons remain potential large-document costs; no performance claim or unrelated refactor was made.

### Completed checks

- `cargo fmt --check` passes.
- `cargo test --locked` passes: 4 unit tests and 18 integration tests (10 existing, 6 reliability/UI regressions, 2 SVG regressions). Coverage retains legacy `.crumbs`/version-1 round trips, recents, rejected file preservation, invalid geometry/version/IDs, canvas gestures and property transactions. New SVG coverage checks visible order, dimensions, alpha, radius clamping, XML escaping, Unicode, clipping references, hidden omission, unchanged source/save/reopen and rejection before replacing an existing export.
- `cargo clippy --locked --all-targets -- -D warnings` passes after the final example changes.
- `cargo build --release --locked` passes; `target/release/papercraft` is Mach-O arm64. No package or release was published.
- Native example runs succeeded for `exchange` at 1380×860 and `export`/`quit` at 960×640. Their actual OpenGL renderer screenshots were inspected: `target/verification/exchange/papercraft-exchange-1380x860.png`, `target/verification/export/papercraft-export-960x640.png`, `target/verification/quit/papercraft-quit-960x640.png`. The example disables animations for stable captures only; application animation behavior is unchanged.
- The `export` scenario uses **simulated egui pointer events** to open the real native menu. The `quit` scenario uses **simulated menu/minimize dispatch** through the real native event loop, delegates to PaperCraft's `App::logic`, and confirms unsaved document state survives while the visible Save/Discard/Cancel prompt is rendered. It restores the saved state before exiting. This is not a physical keyboard/menu-click test.
- The example's separate **model preflight** exercises create → edit Unicode text → undo → redo → atomic save → reopen/equality → HTML and SVG export. These file operations are model calls, not native dialog clicks. Existing simulated egui tests separately cover creation, inspector edits, gestures and dashboard replacement prompts.
- Generated SVG parses as XML, has a 640×400 viewBox, four visible layers in order, two editable text elements, two clip paths, escaped names/text and no script elements. `starter.svg` SHA-256: `cad886453add242c799e46e5229d6bc00f96f092104460cf68642b84cb140071`.
- The available Chromium browser connection opened the generated SVG and HTML from an isolated loopback server. Both rendered and were visually inspected; Unicode, overlap/alpha and clipping were visible. The server returned 200 for both files. Browser access succeeded. This pass did not collect browser-console logs.
- Independent read-only diff review found no further actionable correctness/safety regression. `git diff --check` passes. License notice for adopted `craft-quit` is retained in `licenses/craft-libs-MIT.txt`; original MIT/crane branding remains intact.

### Actual cross-application check

The committed `tests/fixtures/svg-exchange.papercraft` was exported by PaperCraft and imported through the real installed applications' opt-in loopback control servers. These were **actual importer, document-model, native-window and renderer checks**, not replayed source tests or simulated pointer input.

- VectorCraft **0.13.1** and FigCraft **0.7.0** accepted the 640×400 SVG. Inspected model data and images preserve visible ordering, base/overlap geometry, radii 28/70, alpha ≈0.502, hidden-layer omission, multiline Unicode strings as editable text, and clipping of the long text box to 180×28.
- Native windows and 640×400 offscreen output were inspected: `target/verification/exchange/{vectorcraft,figcraft}-{window,import}.png`. Target font choices and metrics differ. FigCraft uses SVG IDs as layer names; VectorCraft retains the exported labels. CJK/Greek render in both target images, while native PaperCraft's bundled font shows placeholder glyphs for CJK.
- Exact installed executable hashes, source revisions, fixture hash, invocation/inspection methods and retained JSON/image hashes are in [the Craft runtime record](craft-source-review.md#runtime-boundary). A source Git revision is not asserted to identify the installed executable. Only disposable target documents/processes were used; both owned target PIDs exited.

### Evidence, compatibility and remaining limits

Paper 0.5.16/build 261015tx3mukqib still matches E1's ASAR SHA-256 `912bad3fe41e226c67fd4e27352e2b4456d69d19589f4583cba5944932fb78b2`; the recorded analysis was reused. Current official docs plus authenticated read-only connector checks support the [workflow evidence](../research/paper/workflow-evidence.md). Existing Paper content was untouched. No Paper editing/history/export runtime parity is claimed. The [gap assessment](gap-assessment.md) ranks the next bounded increments.

No document migration is required: `.crumbs` and `.papercraft` remain version 1; fields, IDs and recent-file paths are unchanged. Export is derived output and does not mark the source saved. The old application can still read source documents created by this increment; rollback requires no data conversion.

SVG keeps explicit text lines and clips overflow; automatic native/HTML wrapping, exact baselines/fonts and full glyph coverage are not reproduced. Root clipping of objects extending outside the artboard, target editing/resave, full cross-app round trips, PhotoCraft/DesignCraft runtime workflows, native file-dialog clicks, physical Cmd-Q, actual screen-reader use, large-document performance and other-platform builds remain unverified. Grouping/hierarchy, snapping/alignment, richer typography, image/vector layers, multiple artboards, layout and CLI/MCP are still gaps. The upstream Touch Bar teardown issue was not reproduced on this hardware.

**Environment failures:** sandboxed native rendering produced macOS Launch Services/XPC errors and stalled; the owned verification PID was stopped. The same native checks succeeded outside the sandbox. Loopback binding was denied inside the sandbox and succeeded outside it. Dependency fetching needed access to Cargo's cache outside the writable roots and then succeeded at the pinned revision. These are sandbox/service restrictions, not application test failures. Research tooling was unchanged; `npm test` was not rerun for this increment.

## Color tokens and arrangement (18 October 2026)

Branch `codex/theme-tokens-arrangement`, created from the uncommitted `codex/reliability-svg-exchange` work on `bc3c52b`. That work was carried over unchanged; a patch and untracked-file archive were also kept outside the repository before editing. Host: Apple Silicon macOS, Rust 1.99.1, eframe/egui 0.36.2. No release, push or affiliation change was made.

### Defect established before correction

| Defect / cause | Focused proof and correction |
|---|---|
| A token-bound color property widened the inspector from 280 to 296 pt at both test sizes. The bound chip's name allowance left 26 pt for an 8 pt gap and a 22 pt detach button. Found while inspecting a native screenshot of this increment's own UI; it was not a baseline defect. | `inspector_width_is_stable_for_bound_and_multiple_selections` failed (296 ≠ 280) before the fix. The allowance now reserves 30 pt. The test also covers single, multiple and empty selections, and the Theme tab's left panel at 180 pt. |

No baseline defect was reproduced in the selection, history, persistence or export paths touched here. The baseline already excluded hidden and locked layers from canvas hit-testing; Paper's January 2026 build log records fixing exactly that in Paper.

### Completed checks

- `cargo fmt --check`, `cargo clippy --locked --all-targets -- -D warnings` and `git diff --check` pass.
- `cargo test --locked` passes: 4 unit and 35 integration tests. The 17 new tests are in `tests/tokens.rs` (9) and `tests/arrangement.rs` (8). One existing assertion changed: version 2 is now valid, so the unsupported-version case uses version 3.
- **Model tests:**
  - token propagation to layer, artboard and page bindings; unbound literals unaffected
  - name rules: case-insensitive uniqueness, CSS-safe characters, 64-byte limit
  - rejected invalid references (dangling, v1-with-tokens, newer version, duplicate IDs/names, IDs beyond `next_token_id`) without replacing the current document
  - delete-detaches-and-preserves, including on locked layers
  - one history step each for create, create-from, rename, value, bind, detach and delete
  - selection invariants under delete, duplicate and undo/redo
  - align against shared bounds and the artboard, distribute with equal and overlapping gaps
  - hidden/locked exclusion; nudge and hide/lock toggles
- **Persistence and migration:**
  - the committed `tests/fixtures/tokens-v2.papercraft` loads and resolves its deliberately stale literals to token values
  - the `legacy.crumbs` and `svg-exchange.papercraft` version 1 fixtures re-save with no new keys and `version: 1`
  - adding a token writes version 2 to a new path, leaving original bytes unchanged; deleting all tokens writes version 1 with the same colors
- **Baseline reader, real code:** an example built from `bc3c52b` in a temporary worktree rejected the fixture and a generated workflow file (unknown field `background_token`), left both byte-identical, and opened a tokenless file written by this build as version 1.
- **Simulated egui input** at 1380×860 and 960×640 where noted:
  - inspector token menu: create from color, bind a second layer, show the bound name, detach
  - Theme tab: select a token and type a hex value. The canvas repaints both layers before Enter; one undo reverts the edit.
  - dashboard thumbnails paint resolved token colors and never the stale literal
  - Shift-click add/remove on the canvas, with locked/hidden layers not pickable
  - a shared drag at both zoom levels moves every arrangeable layer by the same document distance, as one undo step
  - ⌘A, ⌥W, ⌃⌥H (distinguished from center alignment), arrow nudge: one step each
  - the inspector Align/Distribute buttons
  - layer-list Shift-click including a locked layer, which is not moved
  - ⌘S on a version 1 file with tokens shows the upgrade prompt and writes nothing. Cancel keeps the file; Upgrade writes version 2; later saves do not prompt.
- **Native rendering** (`examples/verify_native.rs`, actual OpenGL window, screenshots inspected): `theme`, `bound` and `arranged` at 1380×860 and 960×640 under `target/verification/tokens/`. `selected`, `dashboard` and `export` were re-rendered under `target/verification/regress/` without visible regressions. The example's model preflight runs create → create-from/bind/rename/edit tokens → align/distribute → undo/redo → save → reopen (version 2, equal) → HTML/SVG export, then asserts the resolved accent color in both outputs.
- **Browser:** the built-in Chromium pane, via a loopback server, opened `workflow.html` (SHA-256 `7888454e88ad17b2ea6b1cb9f50d5ab9797d0c811cf172a0af18faaedccbe9c0`) and `workflow.svg` (`1649cb80acf2e7b70a62bbe9ee951471ffb1ec59a9810d37532e195bfc36bf92`). Computed colors of every layer match between the `var(--color-…)` HTML and the literal SVG (for example accent `rgb(176, 72, 44)`). There are zero scripts. The SVG screenshot was inspected.
- `cargo build --release --locked` passes; `target/release/papercraft` is Mach-O arm64.

### Not verified

- **Physical input:** keyboard/mouse input to the native window was not exercised. No computer-use tools were available in this session. Every interaction above is either a model call or simulated egui events, including the native example's renderer run.
- **Platform and environment:** native file dialogs (Save As from the upgrade prompt), macOS Option-key delivery from a physical keyboard, screen readers, other platforms and large-document performance are unverified.
- **Paper runtime:** no Paper runtime comparison was made.

**Environment:** the native examples and the loopback server ran successfully in this session; no sandbox or service failure occurred. The baseline worktree was temporary and has been removed.

### Compatibility and limits

- **Format:** documents without tokens remain version 1 and still open in earlier builds. Documents with tokens are version 2, which earlier builds refuse without modifying them. Rollback is to delete the tokens, or to strip the version 2 keys as described in [document format](document-format.md). Recent-file storage is unchanged.
- **Not implemented:** marquee selection, multi-layer resize, grouping, snapping, theme modes, non-color tokens, token copy/duplicate, and multi-selection property editing.

## Review before landing (18 October 2026)

The two original commits, `5f004a9` and `0868ef3`, are reviewed separately and retained unchanged. Corrections are added on `codex/theme-tokens-arrangement`.

### Persistence corrections

- `version_1_rejects_every_token_field_even_when_empty` failed before correction: the loader accepted `tokens: []` in version 1, contrary to the documented format. The loader now checks field presence before deserializing, rejecting all version 2 keys in version 1, including empty arrays, default identifiers and null bindings. The regression passes and checks that rejected input bytes remain unchanged.
- `unsupported_versions_cannot_be_saved_or_committed` failed before correction: save normalized version 0 into version 1 and replaced the destination. Normalization now retains unsupported versions for validation to reject. The passing regression covers version 0 and the next unsupported version, preservation of existing bytes, commit rollback and unchanged history. Supported version 1/2 normalization remains in place.

### Arrangement correction

Three `coordinate_limits_*` regressions failed before correction. Nudging and dragging previously clamped each layer separately, changing the spacing of a selection when one layer reached ±100,000. They now share one bounded translation; canvas movement rounds the delta instead of rounding each absolute position, retaining fractional offsets. Align/distribute previously clamped individual results and could report success with misaligned edges or unequal gaps. An out-of-range arrangement now fails validation and restores the whole document without adding history. The regressions pass, covering both coordinate limits, both nudge axes, a simulated canvas drag, fractional positions, rejection and undo.

### Independent gates and hygiene

| Revision | `cargo fmt --check` | `cargo test --locked` | `cargo clippy --locked --all-targets -- -D warnings` |
|---|---|---|---|
| `5f004a9`, detached temporary worktree | Pass | 4 unit + 18 integration | Pass |
| `0868ef3`, separate detached temporary worktree | Pass | 4 unit + 35 integration | Pass |
| Corrected branch | Pass | 4 unit + 40 integration | Pass |

- The original tip and corrected branch both pass `cargo build --release --locked` and `cargo run --locked --example verify_native -- target/verification/final theme 1380x860`. Both screenshots were opened and inspected: readable Theme token controls, propagated accent colors, bound artboard color and stable inspector widths. The original image is retained under `target/verification/review-original-0868ef3/`; the corrected image is `target/verification/final/papercraft-theme-1380x860.png`. The example's model lifecycle/token/arrangement/export assertions pass.
- Native runs inside the sandbox stalled with macOS Launch Services/XPC errors. Only the two owned verification PIDs were stopped; the same checks passed outside the sandbox. These are environment failures, not application defects. Both temporary worktrees were clean after verification and removed.
- The working tree was initially clean. A fresh fetch confirmed `origin/main` remained at `bc3c52b` before review. Each original commit was inspected separately, including its message and final `Co-Authored-By` trailer. New fix messages also retain that trailer; the original commits were not amended or rewritten.
- `git ls-files research/private` prints nothing. Tracked-file scans detect no credential patterns or personal absolute paths in the corrected tree. Personal paths in the original Craft runtime reproduction document were replaced with repository-relative instructions; the original commits remain in history. No Paper application source, fonts, branding or assets were added. Required MIT and third-party notices remain present; the craft-libs MIT notice matches the pinned dependency's notice byte for byte.
- README, product/design documents, all `docs/*.md` and `research/paper/*.md` were checked against the implementation and historical verification boundaries. All 52 local links/anchors resolve across 12 Markdown files. Published v0.1.1 compatibility wording, including the upgrade dialog, now distinguishes the older published build from token-capable source builds with the same package version. The existing upgrade-prompt regression passes after that text correction.
- The retained D2 documentation record and both Craft imported-document JSON records match their recorded SHA-256 hashes. Paper runtime behavior and Craft imports were not rerun; earlier source/runtime evidence remains explicitly historical. Research tooling is unchanged; `npm test` was not rerun. `git diff --check` passes. No package, tag or release was created.

Physical native keyboard/mouse input, native file dialogs, screen-reader interaction, other-platform builds and large-document performance remain unverified. Version 2 documents require a token-capable build or the documented downgrade procedure.

## Release downloads v0.1.2 (18 October 2026)

Release source: `40765dfc92fb85e721dda0c4ecbc2e4875bb2cd7`. The [native build run](https://github.com/slyim/papercraft/actions/runs/39155293943) uses the manually dispatched, read-only repository workflow in `.github/workflows/release.yml`. Original application logic remains Rust; the archive/notice helper uses Python's standard library only during packaging. Package version is 0.1.2; macOS bundle build number is 3.

| Target / native runner | Formatting, 44 Rust tests, Clippy, release build | Native renderer | Archive checks |
|---|---|---|---|
| Linux x86_64 / Ubuntu 22.04 | Pass | Pass, Xvfb/Mesa software OpenGL | Pass, tar.gz |
| Linux ARM64 / Ubuntu 22.04 ARM64 | Pass | Pass, Xvfb/Mesa software OpenGL | Pass, tar.gz |
| macOS Apple Silicon / macOS 15 ARM64 | Pass | Pass | Pass, ZIP and tar.gz |
| macOS Intel / macOS 15 Intel | Pass | Pass | Pass, ZIP and tar.gz |
| Windows x86_64 / Windows Server 2022 | Pass | Not exercised | Pass, ZIP with GUI subsystem and static MSVC runtime |

### Corrections established during release checks

- The first Windows run failed the existing recent-file unit test because `/x.crumbs` is not an absolute Windows path. `16e9f17` replaces that Unix-only fixture with the native temporary path already used by the test, serialized through JSON. Production recent-file path validation is unchanged; the Windows test suite now passes.
- The first Linux rendering runs failed loading `libxkbcommon-x11.so`. `40765df` installs the missing runner runtime and documents the dependency for Linux downloads. Both Linux native rendering checks now pass. These were missing runtime-library failures, not compiler or document-model defects.
- Local archive assembly reproduced Python ZIP's rejection of pre-1980 timestamps preserved from dependency notices. ZIP creation now uses the standard library's timestamp normalization. Both actual macOS archive checks pass with those same notice files.

### Packaging and local checks

- `cargo fmt --check`, 4 unit plus 40 integration tests, warnings-denied Clippy and the release build pass locally on Apple Silicon. The portable recent-file correction also passes its focused local regression before the native CI rerun.
- Every archive is checked against the actual built executable bytes, executable architecture, required notices and safe member paths. ZIP integrity and tar executable permissions are checked. All seven downloaded CI archives independently pass their platform SHA-256 manifests before publication; dependency records include 278 packages on each Linux target, 170 on each Mac target and 158 on Windows.
- Notices include exact Cargo-resolved normal/build dependency metadata, locally shipped notices, pinned upstream notices where crates omit them, Rust library notices and bundled font/icon terms. The retained Sheldon MIT notice covers the objc2 family and the MIT-declared dispatch package. No research, agent settings or Git metadata appears in the application archives.
- The local macOS bundle passes ad-hoc signature verification before and after ZIP extraction, uses only system frameworks/libraries, and its extracted executable launched without diagnostics; only that owned process was stopped. Both downloaded CI Mac ZIPs also pass signature verification after extraction.
- The local Python installation initially failed CA verification for notice downloads. Using the operating system CA bundle resolved it without disabling TLS verification. GitHub runners use their configured Python trust stores. Native launch/signature checks use approved execution outside the sandbox.

macOS apps are ad-hoc signed, not notarized; Windows binaries are unsigned. Linux downloads require glibc 2.35 or later, libxkbcommon-x11 and desktop graphics libraries; file dialogs use an XDG desktop portal. Physical keyboard/mouse input, native file-dialog choices, Windows native-window interaction, screen readers, large-document performance and compatibility with other OS/distribution versions remain unverified. No installer or automatic updater was added. Research tooling was unchanged and `npm test` was not rerun.

## Tool rail T1 (18 October 2026)

Contract: [implementation process](implementation-process.md#tool-rail-t1--18-october-2026) and [tool rail plan](toolbar-plan.md).

- `tests/tools.rs` (9 tests): constrained/centered creation geometry; every rail entry's shortcut selects its tool; ⇧ and ⌥ drags at the fitted zoom; clicked text sized to content, typing replaces the placeholder, width follows, Escape ends editing and keeps the selection, two undo steps (edit, then placement); dragged text keeps its width and grows in height; Enter edits without inserting a newline and undo restores the document; new text inherits the last selected text style; marquee selects visible unlocked layers, ⇧ adds, no history entry; auto-width text renders on one line at four window sizes (different fit zooms).
- Defects found while testing this increment's own code and fixed before recording: the inspector's idle commit ended an in-place edit before it took focus; egui withdraws focus from a widget that first appears on a click frame; egui's own Escape handling cleared the selection. Each had a failing test first.
- `cargo fmt --check`, warnings-denied Clippy and the full suite pass locally on Apple Silicon (59 tests after tab/inspector integration checks).
- Native rendering (`examples/verify_native.rs`, actual OpenGL window, scripted input, screenshots inspected): `text` at 1380×860 and 960×640 shows a click-placed layer typed in place at its content width (299 × 32 at 28 px), matching the inspector; `marquee` at both sizes shows the selection rectangle mid-drag and three intersected layers selected. Stored under `target/verification/tools/`.

Not verified: physical keyboard/mouse input and IME composition; the caret is not drawn in captures because the test window is not focused. The constrained/centered drags were exercised in the model harness only, not in a native capture.

### Document tabs and inspector integration preparation

Before merging the remote canvas work, formatting, warnings-denied Clippy and all 59 tests pass. Additional checks cover independent tab history, New/Open/close behavior, Quit visiting every dirty tab, existing-file aliases activating one tab, refusal to overwrite another tab's file, pending names surviving tab commands, one-step rename/undo, Escape cancellation and New File remaining visible with 13 tabs at 960×640.

Regression tests first reproduced duplicate file tabs, pending-name loss and offscreen New File controls. File identity checks now cover every open entry point and Save As; document commands flush pending names before switching or checking dirty state; excess tabs scroll. Native checks for the combined local and remote implementations are recorded separately after integration.

## Canvas and performance (18 October 2026)

Contract: [canvas and performance increment](implementation-process.md#canvas-and-performance-increment--18-october-2026). Baseline `1ecd3b8`. Environment: Linux x86_64 cloud container, Rust 1.97.0, Xvfb with Mesa; not the macOS reference platform.

### Frame time

`cargo run --release --example bench`: headless egui frames at 1380×860 including tessellation, not GPU upload; layers are 32-unit squares on a 4000×4000 artboard, a quarter of them text. Median milliseconds, three runs each; the same harness was built against the baseline in a temporary worktree (removed afterwards). Timings on this shared machine vary by roughly ±20%.

| Layers | Idle frame | Select all | Idle, all selected | Drag frame, all selected | Undo |
|---|---|---|---|---|---|
| 1,000 baseline → now | 2.1–2.5 → 0.5–0.8 | 0.3–0.5 → 0.03–0.04 | 3.4–3.7 → 1.0 | 3.9–4.5 → 1.1–1.5 | 0.18–0.31 → 0.03 |
| 5,000 | 13–18 → 2.6–2.9 | 7.7–9.7 → 0.12–0.18 | 34–40 → 4.9–5.6 | 41–48 → 5.8–6.4 | 4.4–7.5 → 0.11–0.21 |
| 10,000 | 26–33 → 4.0–4.3 | 31–45 → 0.30 | 122–134 → 10–11 | 157–167 → 11–13 | 20–32 → 0.40–0.51 |

The baseline drag column already includes snapping work in the current build (snapping did not exist at baseline). A callgrind profile of the current build attributes about half of all instructions to egui tessellation of the visible layers; the remaining app-side cost is linear in the layer count.

### Checks

- `cargo fmt --check`, `cargo clippy --locked --all-targets -- -D warnings` and `git diff --check` pass. `cargo test --locked`: 4 unit + 49 integration tests pass.
- `tests/performance.rs`: revisions through undo/redo and no-op commits; undoing to the saved state needs no unsaved-changes prompt; a 10,000-layer document builds fewer than 100 layer rows, and select-all, toggle, delete and undo keep the selection consistent.
- `tests/canvas.rs`, simulated egui input at 1380×860 and 960×640: marquee selection with hidden/locked exclusion, Shift-add and no history; top-left, right-edge, top-edge, minimum-size and aspect-locked resize, each one undo step; group scaling of two layers with hidden/locked layers unchanged; snapped move and resize land exactly on a target edge and draw a guide; ⌘/Ctrl and the toggle bypass snapping; a hidden layer is not a target while the same layer visible is. Geometry unit checks cover the snapping, resize and mapping helpers.
- `tests/editor.rs` `native_editor_renders_at_supported_sizes_and_keyboard_actions_use_history` checks exact pointer alignment; it now turns snapping off because its drag ends within 5 points of another layer's edge, which snapping intentionally changes.
- After merging `main` at `7220e9d` (release packaging, portable recent-file fixture), formatting, warnings-denied Clippy, all 53 tests, the release build and native `editor`/`selected`/`arranged` renders were rerun on the merged tree and pass.
- Native rendering: `xvfb-run cargo run --locked --release --example verify_native -- target/verification/canvas selected|arranged 1380x860` rendered through OpenGL (Mesa). Screenshots were inspected: four corner handles on the single selection and on the group bounds, handles on the group outline, crisp per-layer outlines, the magnet toggle in the tool rail. The example's model workflows pass.

### Not verified

- Guides and the marquee in a native window: covered by simulated input only, as the native example captures a static frame.
- Physical mouse and trackpad input, macOS rendering, GPU upload cost and large documents with long text.
- Paper's own handle, marquee and snapping behavior; the rules above are PaperCraft decisions.

## Compact layout scale (18 October 2026)

Contract: [layout scale](../DESIGN.md#layout-scale). Environment: Linux x86_64 container, Xvfb with Mesa.

**Reference measurement.** A user-supplied screenshot of the Paper dashboard (2000×1248 px) was measured. The macOS window controls are 31 px apart (20 pt on macOS), giving about 1.55 px per point; 38 px rows (≈24 pt) agree. Measured in points: tab strip ≈29, tabs and sidebar rows 24, sidebar ≈211 wide with ≈10 pt insets, search field 24, content side padding ≈70, title row ≈32 tall starting ≈32 below the strip, cards ≈292×240 with a ≈48 header, ≈4 pt preview inset and ≈22 pt gaps, body text ≈11–12 pt. Values were rounded to the nearest grid step. The earlier panel-width reference (240/180/40/280 pt) was not reproduced by this screenshot and was replaced. The editor was not visible in it; editor panels reuse the same scale and remain to be checked against a Paper editor screenshot.

**Checks.** `cargo fmt --check`, warnings-denied Clippy and `cargo test --locked` (4 unit + 51 integration) pass. `tests/design_grid.rs` scans `src/ui.rs`, `src/app.rs` and `src/app/dashboard.rs` and fails on any off-grid layout literal. The panel-width tests now assert the shared constants. The tab test checks that the close control is hidden until the tab is hovered. Native renders of `dashboard`, `editor`, `selected` and `theme` at 1290×805 were inspected: no clipped or wrapped controls after sizing the New file button from its label.

**Not verified.** macOS rendering and the title-bar inset (72 pt beside the window controls), physical input, and a side-by-side comparison of the editor with Paper.

## Combined main integration (18 October 2026)

Source: merge `1399d45` combines local toolbar/tab/inspector commits `4435f9a` and `49a3edc`, icon commit `40f825d`, and remote main `34646a0` without rebasing. The merge commit records the five conflicted files and their resolutions. The later native verification example adds reproducible resize and snapping scenarios; application behavior is unchanged by that harness addition.

### Checks on macOS Apple Silicon

- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` (9 unit + 63 integration tests), `cargo build --release --locked` and `git diff --check` pass. The 4-point-grid scan includes the new inspector and rename controls; single, multi-selection and token-bound inspector widths remain stable. Marquee, all resize handles, group scaling, snapping/bypass, geometry limits, undo/redo, tokens, save compatibility and HTML/SVG checks pass together with the tool and tab tests.
- Final layout inspection aligned registry glyphs with the shared icon size, the inspector caret gap with the spacing scale, and all fill-selector cells/insets with the 4-point grid. All checks above passed again; selected-layer native renders at both sizes were repeated after this adjustment.
- Added integration checks cover dirty-cache isolation when different tabs have equal revision numbers, per-tab text styles/token IDs, and live in-place text changes being dirty before commit. Existing 10,000-layer selection and virtualized-list checks pass.
- `cargo run --release --locked --example verify_native -- target/verification/integrated <scenario> <size>` passes for `editor`, `selected`, `arranged`, `text`, `marquee`, `dashboard` and `theme` at 1380×860; `selected`, `text`, `marquee`, `resize` and `snapping` at 960×640; and `resize`/`snapping` at 1380×860. Screenshots were opened and inspected: readable controls, inherited compact spacing, four corner handles, group outlines, in-place text and marquee rectangles. Resize asserts `[100, 100, 160, 140]`; the snapped move asserts `[200, 100, 100, 100]` and its capture shows the alignment guides.
- Native `quit` at 960×640 passes the simulated macOS Quit/minimize check: the unsaved document survives and the prompt renders. Model workflows in every native run verify create/edit/undo/redo/save/reopen and HTML/SVG export, including tokens and arrangement.
- The release executable was launched separately. OS-delivered input exercised new tabs, the rectangle tool, creation, resizing and a visible snapping guide in an unsaved verification document. Window screenshots were inspected. Foreground changes prevented a repeatable OS-level marquee check; the native scripted marquee and gesture tests above remain its verified coverage. Only the owned verification process was stopped; its unsaved test changes were discarded.
- `cargo run --release --locked --example bench`, one local run: median idle / selected idle / drag times were 0.60 / 0.95 / 1.22 ms for 1,000 layers, 2.13 / 1.72 / 2.60 ms for 5,000, and 3.19 / 5.36 / 4.29 ms for 10,000. These include headless egui tessellation, not GPU upload; no same-machine baseline comparison was run.

### Remaining verification limits

Physical keyboard/mouse/trackpad input, IME composition, screen readers, native file-dialog choices, other operating systems and large documents with long text remain unverified for this combined build. Native screenshots and scripted input establish macOS rendering and gesture behavior, not Paper parity. Disabled inspector properties remain unavailable as described in the contract. Research tooling and release packaging were unchanged; their suites and cross-platform CI were not rerun.

## Parity and branch review (19 October 2026)

Reviewed application source: `6065d02`. The review updates current capability/agent-handoff documentation and deletes the 105-line implementation-only `tests/design_grid.rs` scanner and its two tests. Native layout/control-bound regressions remain; production application code, dependencies and formats are unchanged. Historical counts and claims above describe their original revisions.

### Branch disposition

After `git fetch --prune origin`, every non-main branch tip was an ancestor of `main`; no commit was unique to those refs. `git log --all --not main --oneline --decorate=full` returned no commits. The following deletions completed successfully; their original commits remain reachable from main and no release tag was deleted.

| Deleted ref | Tip | Preserved integration |
|---|---|---|
| Local `backup/main-pre-merge` | `40f825d` | First-parent history of merge `1399d45` |
| Local `backup/origin-main-pre-merge` | `34646a0` | Second parent of `1399d45` |
| Local `codex/release-downloads` | `7908eee` | Release history through `40765df` / `7220e9d` |
| Local `codex/reliability-svg-exchange` | `bc3c52b` | Main ancestry; substantive reliability/SVG commit `5f004a9` is also retained |
| Local `codex/theme-tokens-arrangement` | `1ecd3b8` | Common ancestry of release and canvas work |
| Remote `claude/fervent-feynman-vchls5` | `34646a0` | Exact merge input; remote deletion used a force-with-lease guard |

No application function was proven obsolete. Legacy `.crumbs`, v1/v2 readers and the Crumbs recents location remain supported and actively used, not deleted based on age.

### Checks exercised

- `cargo fmt --check`: pass.
- `cargo test --locked`: **70 pass** (9 unit + 61 integration), zero failures. The prior combined-build total of 72 included the two removed source-scanner tests. Retained cases cover tabs/file preservation, text tools, gestures, geometry, tokens, history, save compatibility and exports.
- `cargo clippy --locked --all-targets -- -D warnings`: pass.
- Actual macOS native windows: `cargo run --locked --example verify_native -- target/verification/parity-review <scenario> <size>` passed for `resize`, `snapping` and `quit` at 960×640. Screenshots were opened: resized geometry/inspector, snapped edges/guides and the dirty-Quit preservation prompt were visible. Resize asserted `[100, 100, 160, 140]`; snapping asserted `[200, 100, 100, 100]`; simulated native Quit/minimize preserved the unsaved document.
- After removing the scanner, native `resize` at 960×640 and `theme` at 1380×860 also passed under `target/verification/parity-review-final/`; both screenshots were opened. Theme controls, resolved bound colors and inspector layout rendered.
- Every native invocation completed the example's model create/edit/undo/redo/save/reopen/HTML+SVG lifecycle and token/arrangement/export assertions. This is model-workflow proof, not physical input.

### Confirmed defects, not fixes

A throwaway Rust example, `cargo run --locked --example review_repro`, drove `PaperCraftApp::show` using simulated egui events plus the real Editor setup/token APIs. Its final run reproduced all three defects below. The example was removed after investigation; issue bodies retain the triggers and acceptance requirements.

| Issue | Observed result |
|---|---|
| [#12 Cross-tab inline undo](https://github.com/slyim/papercraft/issues/12) | Equal-ID text layers in separate tabs: undo in B changed `B-original` to `A-original` from A's widget history |
| [#10 Automatic-size text loss](https://github.com/slyim/papercraft/issues/10) | 10,000 `W` characters produced width `261250`; Escape restored `New text`, width `117`, discarding the typed session |
| [#11 Cached token-ID reuse](https://github.com/slyim/papercraft/issues/11) | After red-token deletion and blue-token creation, new text inherited `[10, 20, 200, 255]`, `Some(1)`, instead of the prior red literal |

The first throwaway-harness attempt stopped because unconsumed egui texture deltas were not cleared; correcting the harness, following the existing headless-test convention, produced the observations above. This was not an application failure. Existing passing tests do not establish correctness for these newly demonstrated sequences.

### Verification and release limits

Native `text` at 1380×860 exited successfully, but its inspected screenshot showed only the starter layers and no newly placed text. This run is **not** counted as text-interaction proof; startup/focus cause remains unconfirmed. An unsupported scenario name also silently rendered the ordinary editor. [#1](https://github.com/slyim/papercraft/issues/1) requires unknown-name rejection and meaningful text/marquee postconditions.

The only workflow is manually dispatched release packaging. Its latest successful [five-platform run](https://github.com/slyim/papercraft/actions/runs/39155293943) covers published v0.1.2 source `40765df`, not this integrated main. [#2](https://github.com/slyim/papercraft/issues/2) adds automatic gates; [#3](https://github.com/slyim/papercraft/issues/3) tracks a new verified preview after the defects and verification gaps are resolved.

No new release, cross-platform build, physical-input/file-dialog/IME/screen-reader check, Paper runtime comparison, Craft import rerun or REA/npm tooling test was performed. Current parity and the prioritized issue/dependency queue are in the [gap assessment](gap-assessment.md#current-parity--19-october-2026).
