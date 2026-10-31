# Figma import

Introduced in the v0.4.0 preview. The local working-tree build was packaged and installed on macOS before release preparation.

Choose Open and select a `.fig` file, or launch PaperCraft with its path. The converted document opens in a new, unsaved tab. Save As writes a native `.papercraft` file. Import errors leave existing tabs intact. The app refuses saves with a `.fig` extension.

Headless conversion:

```sh
cargo run --locked -- convert design.fig --output design.papercraft
```

Warnings go to stderr. A failed import does not write the output. A successful conversion atomically replaces an existing output; the input cannot be the output. Native `.papercraft` readers remain unchanged.

The importer accepts a ZIP containing `canvas.fig` (stored or deflated), or a bare `fig-kiwi` canvas. Embedded Kiwi schemas are decoded locally; raw deflate, zlib and zstd canvas chunks are accepted. No account, external executable or network service is involved.

Only the first page, ordered by its source sibling position, is converted. Frames, groups and component definitions become Frames; rectangles, ellipses and plain text remain editable. Parent-relative translations become PaperCraft world coordinates. Sibling position determines paint order. Names, visibility, locking, size, uniform corner radius, font size, first visible solid fill and ordinary Frame clipping are retained where present. Missing required geometry or values outside PaperCraft's limits fail conversion.

Unsupported node types and rotated/scaled/skewed layers are skipped with their subtrees. Images, vectors, instances and overrides, strokes, gradients, effects, layout rules, font families, rich typography, variables, prototypes and additional pages are not converted. Group opacity is not composited. A warning describing these limits appears in the editor status and CLI stderr; conversion is not a fidelity guarantee. Source bytes must be retained for later imports.

Limits: 16 MiB source, 64 MiB extracted canvas or inflated chunk, 16 chunks, 1 million decoded values, 10,000 source nodes and 64 hierarchy levels; ordinary native document validation also applies. Unsupported/empty first pages fail rather than producing an empty success.

## Reference observations

Figcraft reference: [storytold/figcraft](https://github.com/storytold/figcraft/tree/e04c5d2274329587b9213f27ad8f88708ea77748), commit `e04c5d2274329587b9213f27ad8f88708ea77748`.

- F1, observed in `crates/format/src/fig.rs`: ZIP/bare canvas framing, schema/message chunks, compression detection and a generic Kiwi decoder.
- F2, observed in `crates/format/src/fig/map.rs` and its `map/` modules: GUID/parent-index relationships, sibling position strings, relative matrices, paints and text characters. Its complete mapping supports substantially more than this importer.
- F3, reused under MIT: `crates/format/src/fig/kiwi.rs`, original SHA-256 `7daa232fcb75c381090e6e2d235e420ae9435cdb1ae1dcb2d93af2e116f72e0a`. Attribution is in `src/fig_import/kiwi.rs` and `licenses/figcraft-MIT.txt`; packaging already includes the project license directory. Encoder/schema-text helpers compile only in tests. PaperCraft implements its own bounded archive reader and document mapping.

Inference: these field names and framing enable a bounded native importer without importing Figcraft's document model, renderer or application. The reference checkout remains ignored at `research/private/figcraft-reference/`.

Unknown: fidelity on actual Figma exports, newer private schemas, component expansion and cross-platform native file-dialog behavior. Synthetic tests do not establish compatibility with arbitrary `.fig` files.

## Verification

Tests construct schema/message bytes and compressed ZIP/bare canvases, convert editable layers, check world coordinates, hierarchy, paint order, color alpha and Unicode text, and save/reopen through the native format. Malformed/truncated data, duplicate IDs and read caps are checked. App-state tests check dirty import tabs, preservation of existing tabs, failed-import preservation and refusal to overwrite `.fig` input. CLI conversion checks source-byte preservation and native reopen.

Verification on 31 October 2026: `cargo test --locked --quiet` passed (172 tests); `cargo fmt --check`, `cargo clippy --locked --all-targets -- -D warnings` and `git diff --check` passed. Synthetic compression cases cover raw deflate, zlib and zstd, and the ZIP case uses deflated storage. Page ordering and rotated-subtree exclusion also pass.

The installed macOS build also passed synthetic `.fig` import through live MCP, editing, undo/redo, native save/reopen and PNG/SVG/HTML export. A fresh isolated installed launch preserved all three imported layers across idle frames. A two-screen mobile design test then verified 84 editable layers, seven color tokens, complete batch undo/redo, native GUI/CLI save/reopen and PNG/SVG/HTML exports while preserving the existing unsaved document.

Native pointer/dialog interaction and real Figma exports remain unverified. Release publication still requires the five-platform workflow gates.
