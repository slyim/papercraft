# ArtCraft interoperability

18 October 2026. Status: the first bounded SVG export slice is implemented and checked in installed VectorCraft/FigCraft; later slices remain proposals. PaperCraft is independently maintained, with no official ArtCraft affiliation or launcher enrollment.

PaperCraft provides the local interface-design canvas, HTML handoff and SVG export of its current rectangle/text artboard. Other applications can provide image editing, vector illustration, publishing and motion workflows. PNG export, image layers, CLI automation and MCP remain unimplemented.

## Current source review

The [18 October source review](craft-source-review.md) inspects current PhotoCraft, VectorCraft, FigCraft, DesignCraft, Craft Launcher and craft-libs source, with exact revisions, model/command/history/rendering/persistence/accessibility boundaries and license checks. The reviewed heads match the earlier references below except VectorCraft, now [`cdba0aac03f83bbc633f5e2aa9500354648938d3`](https://github.com/storytold/vectorcraft/tree/cdba0aac03f83bbc633f5e2aa9500354648938d3). These are source observations; installed releases need separate runtime checks.

- SVG is a concrete exchange surface: VectorCraft and FigCraft import editable primitives and text; FigCraft represents clipping with masks. Explicit text baselines, XML escaping, alpha and paint order need a fixture. Fonts and wrapping differ between renderers. Their native document formats are not interchangeable with `.papercraft`.
- PhotoCraft's SVG open path outlines text and rasterizes clipped/effected content; it does not export SVG. DesignCraft places SVG as a graphic, rasterizes its screen preview and uses vectors for PDF. These can support later raster/publishing workflows without claiming editable document round trips.
- Shared crates use `MIT OR Apache-2.0`; egui controls target 0.36 and Rust 1.95. PaperCraft's existing model, atomic writer and Phosphor controls cover the first exchange slice. The current reliability work adopts only macOS `craft-quit` at reviewed revision `82a24b006eefa036b43c09a67fb9a3fbd17ffe63`, with its MIT notice; no SVG dependency is added. Craft branding has separate restrictions and is not adopted.
- Native source conventions worth retaining are one command path, explicit interaction transactions, selection restored with history, view transforms separate from document geometry, bounded parsers and render caches. They do not require a new multi-crate architecture for the current editor.
- The macOS `craft-quit` integration intercepts Quit through PaperCraft's close/unsaved-work handling. The verification record distinguishes demonstrated PaperCraft behavior from the upstream Touch Bar teardown report, which was not reproduced on this hardware.

The [verification record](verification.md) distinguishes actual imports/renders from source inspection and records the implementation status of the selected slice.

**Bounded SVG exchange verified:** PaperCraft's rectangle/text fixture was opened in installed VectorCraft 0.13.1 and FigCraft 0.7.0. Their actual document models retain the 640×400 drawing, paint order, rounded geometry, transparency, clipping and editable multiline Unicode text. Offscreen renders and native-window screenshots were inspected; font metrics differ. FigCraft names imported layers from SVG IDs. The [source review's runtime record](craft-source-review.md#runtime-boundary) records executable/fixture hashes, assertions, evidence paths and limits. Target editing/resave, PhotoCraft/DesignCraft import and native-format round trips remain unverified.

## Verified upstream interfaces

These are observations from public repository documentation and source, not runtime compatibility results.

- The native Craft apps and shared libraries declare `MIT OR Apache-2.0`. PaperCraft uses MIT for its original code, documentation and generated logo. Existing third-party material retains its own terms. References: [PhotoCraft package metadata](https://github.com/storytold/photocraft/blob/8d2e615ab93d424c4ec25ab15b4edb5368d1f9a6/Cargo.toml), [craft-libs package metadata](https://github.com/storytold/craft-libs/blob/82a24b006eefa036b43c09a67fb9a3fbd17ffe63/Cargo.toml).
- [VectorCraft](https://github.com/storytold/vectorcraft/blob/2cdb0d2d53a0ff3c4ae994bf4808ae53a5799505/README.md) and [FigCraft](https://github.com/storytold/figcraft/blob/d8ff1441b1cd47cb45f1e58b030409ef6f1e24bf/README.md) document SVG exchange and command-based automation. FigCraft already covers interface design and prototyping; PaperCraft should concentrate on its bounded local canvas and HTML workflow rather than assume that role is missing from the suite.
- [PhotoCraft](https://github.com/storytold/photocraft/blob/8d2e615ab93d424c4ec25ab15b4edb5368d1f9a6/README.md) documents raster formats, SVG import and CLI/MCP automation. Its [desktop control protocol](https://github.com/storytold/photocraft/blob/8d2e615ab93d424c4ec25ab15b4edb5368d1f9a6/docs/control-protocol.md) requires connection authentication; a local socket is not an interchangeable public API across every Craft app.
- [FilmCraft](https://github.com/storytold/filmcraft/blob/48088e3a8e50bd4a7a71d1ebdfde0835b01a3051/README.md) documents still-image import. [EffectCraft](https://github.com/storytold/effectcraft/blob/f36b2b3e6bd121dc52b00503cdf8d60f42ac242a/README.md), [DesignCraft](https://github.com/storytold/designcraft/blob/7cb8b12507def48fb9346e5004340b5e8e4cfcb7/README.md) and [LightCraft](https://github.com/storytold/lightcraft/blob/4f0b110bcbd2e350c6d97b47f1d824c2222295c4/README.md) document their own automation and media workflows. Each adapter needs its own format/version check.

## Implementation order

| Slice | Intended workflow | Acceptance check |
|---|---|---|
| 1. SVG export — implemented | Export the existing artboard's visible rectangles and explicit text lines beside HTML, using the authoritative document, validation and atomic writes. Keep `.papercraft` as the editing source. | `tests/fixtures/svg-exchange.papercraft` was exported by PaperCraft and opened in installed VectorCraft 0.13.1/FigCraft 0.7.0. Imported model assertions and inspected native/offscreen images establish the fixture's 640×400 size, paint order, radii, alpha, hidden-layer omission, multiline Unicode text and text clipping. Exact typography, automatic wrapping, art outside the root viewport, target editing/resave and round trips remain unverified or unsupported. |
| 2. Raster exchange | Add artboard-only PNG export, then PNG/JPEG image placement. PhotoCraft can refine images and return a flattened asset; LightCraft can supply developed images. FilmCraft/EffectCraft can use exported title cards and overlays. | Check pixel dimensions, alpha and colour interpretation independently of viewport zoom. Choose image storage and size limits before changing the document format. Save/reopen preserves image placement; rejected imports leave the open document unchanged. Replacing an image is one undo step. |
| 3. Layout and PDF handoff | Use SVG/PNG placement for publishing. Evaluate an explicit VectorCraft conversion step for PDF before implementing another PDF writer. | Open a test export in DesignCraft and PdfCraft, verify page dimensions and visible content, and state whether text and layers remain editable. Never label a flattened export as a document round trip. |
| 4. CLI and MCP | Route a small Rust command surface through the existing `Editor`, then expose inspect/export and bounded edits over stdio. Agents can coordinate PaperCraft with each Craft app's CLI/MCP server. | The UI and commands produce the same document and history. Invalid commands leave state unchanged; one edit batch has one undo step. Verify discovery, errors and file access boundaries before adding external-app adapters. |

Complete and verify each slice before expanding it. SVG export is the first implemented exchange; native-format conversion and a suite-wide plugin framework remain proposals. PaperCraft reads both `.papercraft` and legacy `.crumbs` documents. `.figcraft`, `.vectorcraft` and `.pcraft` files are not currently PaperCraft inputs.

## Shared Rust components

[craft-libs](https://github.com/storytold/craft-libs/blob/82a24b006eefa036b43c09a67fb9a3fbd17ffe63/README.md) offers utilities already used by Craft apps. Evaluate them against an actual PaperCraft requirement and pin an audited revision when adopting one:

- Compare `craft-atomic` with the existing `document::atomic_write` before changing persistence. Preserve rejected-save and existing-file tests; verify durability on supported platforms.
- Evaluate `craft-quit` when testing macOS quit behaviour. Unsaved-document decisions must remain in PaperCraft.
- Consider `craft-docpath` when documents reference external images. Saved paths must not cause unexpected network or device access.
- Consider `craft-ui`, `craft-cms`, `craft-pdf` and `craft-print` only as the corresponding UI, colour, PDF or print requirement arrives. Keep PaperCraft' Phosphor icon choice and application identity.
- [craft-fonts](https://github.com/storytold/craft-fonts/blob/a71c20e30af613287c504f7e685ca8fad77752bc/README.md) is an optional font source; font licenses remain separate from application licenses. Do not add the collection merely to match branding.

The earlier repository-alignment change added no Craft crate. Current reliability work adopts only macOS `craft-quit` at the reviewed immutable revision above. Rust/egui remain the application stack; research tooling stays separate.

## Distribution and official inclusion

[Craft Launcher's catalog](https://github.com/storytold/craft-launcher/blob/9cef5d2a0f207da50d31547a726057ea40f7e5b4/crates/core/src/catalog.rs) contains a fixed suite and restricts installation to the `storytold` owner. Its bundle-ID and website conventions are also suite-specific. Adding a topic, README link or local manifest cannot register `slyim/papercraft`.

Official inclusion requires maintainer agreement about independent repositories or a separate repository arrangement, plus a reviewed launcher change. Retain `io.github.slyim.papercraft`; do not impersonate an ArtCraft bundle ID or copy its branding. No upstream request or repository transfer has been made.

The [launcher release parser](https://github.com/storytold/craft-launcher/blob/9cef5d2a0f207da50d31547a726057ea40f7e5b4/crates/core/src/release.rs) recognizes `SHA256SUMS.txt`. PaperCraft can provide that checksum alias without replacing its existing assets. Its current macOS download is an ad-hoc-signed arm64 ZIP; the [launcher documentation](https://github.com/storytold/craft-launcher/blob/9cef5d2a0f207da50d31547a726057ea40f7e5b4/README.md) describes signed/notarized DMG installation. Packaging, CPU selection and prerelease handling must be verified before installer compatibility is claimed.

Shared [Craft release actions](https://github.com/storytold/craft-common-actions/blob/58076cf0ffb9cee7973d4bc6b523c8922b4471d2/README.md) can be evaluated once PaperCraft has the packaging inputs they require. License alignment alone does not establish a compatible build pipeline.

## Earlier review boundary

No Craft application was launched for this investigation. Cross-app rendering, text fidelity, native-format conversion, CLI/MCP interoperability and launcher installation are unverified. This change establishes licensing, repository references and an evidence-based roadmap; it does not implement those adapters or establish official membership.

That paragraph records the earlier licensing/roadmap pass. The current [source review](craft-source-review.md) and [verification record](verification.md) supersede its coverage only where they explicitly record a completed check. Official affiliation, repository transfer and release publication remain outside scope.
