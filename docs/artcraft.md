# ArtCraft interoperability

18 October 2026. Status: proposed implementation plan; repository licensing and links are established. PaperCraft is independently maintained, with no official ArtCraft affiliation or launcher enrollment.

PaperCraft provides the local interface-design canvas and HTML handoff. Other applications can provide image editing, vector illustration, publishing and motion workflows. The current PaperCraft release has rectangle/text layers and HTML export; SVG, PNG export, image layers, CLI automation and MCP remain unimplemented.

## Verified upstream interfaces

These are observations from public repository documentation and source, not runtime compatibility results.

- The native Craft apps and shared libraries declare `MIT OR Apache-2.0`. PaperCraft uses MIT for its original code, documentation and generated logo. Existing third-party material retains its own terms. References: [PhotoCraft package metadata](https://github.com/storytold/photocraft/blob/8d2e615ab93d424c4ec25ab15b4edb5368d1f9a6/Cargo.toml), [craft-libs package metadata](https://github.com/storytold/craft-libs/blob/82a24b006eefa036b43c09a67fb9a3fbd17ffe63/Cargo.toml).
- [VectorCraft](https://github.com/storytold/vectorcraft/blob/2cdb0d2d53a0ff3c4ae994bf4808ae53a5799505/README.md) and [FigCraft](https://github.com/storytold/figcraft/blob/d8ff1441b1cd47cb45f1e58b030409ef6f1e24bf/README.md) document SVG exchange and command-based automation. FigCraft already covers interface design and prototyping; PaperCraft should concentrate on its bounded local canvas and HTML workflow rather than assume that role is missing from the suite.
- [PhotoCraft](https://github.com/storytold/photocraft/blob/8d2e615ab93d424c4ec25ab15b4edb5368d1f9a6/README.md) documents raster formats, SVG import and CLI/MCP automation. Its [desktop control protocol](https://github.com/storytold/photocraft/blob/8d2e615ab93d424c4ec25ab15b4edb5368d1f9a6/docs/control-protocol.md) requires connection authentication; a local socket is not an interchangeable public API across every Craft app.
- [FilmCraft](https://github.com/storytold/filmcraft/blob/48088e3a8e50bd4a7a71d1ebdfde0835b01a3051/README.md) documents still-image import. [EffectCraft](https://github.com/storytold/effectcraft/blob/f36b2b3e6bd121dc52b00503cdf8d60f42ac242a/README.md), [DesignCraft](https://github.com/storytold/designcraft/blob/7cb8b12507def48fb9346e5004340b5e8e4cfcb7/README.md) and [LightCraft](https://github.com/storytold/lightcraft/blob/4f0b110bcbd2e350c6d97b47f1d824c2222295c4/README.md) document their own automation and media workflows. Each adapter needs its own format/version check.

## Implementation order

| Slice | Intended workflow | Acceptance check |
|---|---|---|
| 1. SVG export | Export the existing artboard's rectangles and text for VectorCraft/FigCraft; keep `.papercraft` as the editing source. Add one export path beside HTML, reusing document validation and atomic writes. | A fixture with overlapping rectangles, corner radii, transparency, hidden layers and Unicode text opens in the target apps with the expected size and order. Escape XML correctly. Document font substitution, wrapping and clipping differences; do not promise identical typography. |
| 2. Raster exchange | Add artboard-only PNG export, then PNG/JPEG image placement. PhotoCraft can refine images and return a flattened asset; LightCraft can supply developed images. FilmCraft/EffectCraft can use exported title cards and overlays. | Check pixel dimensions, alpha and colour interpretation independently of viewport zoom. Choose image storage and size limits before changing the document format. Save/reopen preserves image placement; rejected imports leave the open document unchanged. Replacing an image is one undo step. |
| 3. Layout and PDF handoff | Use SVG/PNG placement for publishing. Evaluate an explicit VectorCraft conversion step for PDF before implementing another PDF writer. | Open a test export in DesignCraft and PdfCraft, verify page dimensions and visible content, and state whether text and layers remain editable. Never label a flattened export as a document round trip. |
| 4. CLI and MCP | Route a small Rust command surface through the existing `Editor`, then expose inspect/export and bounded edits over stdio. Agents can coordinate PaperCraft with each Craft app's CLI/MCP server. | The UI and commands produce the same document and history. Invalid commands leave state unchanged; one edit batch has one undo step. Verify discovery, errors and file access boundaries before adding external-app adapters. |

Complete and verify each slice before expanding it. The first proposed implementation is SVG export, not native-format conversion or a suite-wide plugin framework. PaperCraft reads both `.papercraft` and legacy `.crumbs` documents. `.figcraft`, `.vectorcraft` and `.pcraft` files are not currently PaperCraft inputs.

## Shared Rust components

[craft-libs](https://github.com/storytold/craft-libs/blob/82a24b006eefa036b43c09a67fb9a3fbd17ffe63/README.md) offers utilities already used by Craft apps. Evaluate them against an actual PaperCraft requirement and pin an audited revision when adopting one:

- Compare `craft-atomic` with the existing `document::atomic_write` before changing persistence. Preserve rejected-save and existing-file tests; verify durability on supported platforms.
- Evaluate `craft-quit` when testing macOS quit behaviour. Unsaved-document decisions must remain in PaperCraft.
- Consider `craft-docpath` when documents reference external images. Saved paths must not cause unexpected network or device access.
- Consider `craft-ui`, `craft-cms`, `craft-pdf` and `craft-print` only as the corresponding UI, colour, PDF or print requirement arrives. Keep PaperCraft' Phosphor icon choice and application identity.
- [craft-fonts](https://github.com/storytold/craft-fonts/blob/a71c20e30af613287c504f7e685ca8fad77752bc/README.md) is an optional font source; font licenses remain separate from application licenses. Do not add the collection merely to match branding.

No Craft crate is added by this repository-alignment change. Rust/egui remain the application stack; research JavaScript stays separate.

## Distribution and official inclusion

[Craft Launcher's catalog](https://github.com/storytold/craft-launcher/blob/9cef5d2a0f207da50d31547a726057ea40f7e5b4/crates/core/src/catalog.rs) contains a fixed suite and restricts installation to the `storytold` owner. Its bundle-ID and website conventions are also suite-specific. Adding a topic, README link or local manifest cannot register `slyim/papercraft`.

Official inclusion requires maintainer agreement about independent repositories or a separate repository arrangement, plus a reviewed launcher change. Retain `io.github.slyim.papercraft`; do not impersonate an ArtCraft bundle ID or copy its branding. No upstream request or repository transfer has been made.

The [launcher release parser](https://github.com/storytold/craft-launcher/blob/9cef5d2a0f207da50d31547a726057ea40f7e5b4/crates/core/src/release.rs) recognizes `SHA256SUMS.txt`. PaperCraft can provide that checksum alias without replacing its existing assets. Its current macOS download is an ad-hoc-signed arm64 ZIP; the [launcher documentation](https://github.com/storytold/craft-launcher/blob/9cef5d2a0f207da50d31547a726057ea40f7e5b4/README.md) describes signed/notarized DMG installation. Packaging, CPU selection and prerelease handling must be verified before installer compatibility is claimed.

Shared [Craft release actions](https://github.com/storytold/craft-common-actions/blob/58076cf0ffb9cee7973d4bc6b523c8922b4471d2/README.md) can be evaluated once PaperCraft has the packaging inputs they require. License alignment alone does not establish a compatible build pipeline.

## Unknowns and completion boundary

No Craft application was launched for this investigation. Cross-app rendering, text fidelity, native-format conversion, CLI/MCP interoperability and launcher installation are unverified. This change establishes licensing, repository references and an evidence-based roadmap; it does not implement those adapters or establish official membership.
