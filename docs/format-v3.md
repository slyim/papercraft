# Document format version 3

Integrated from `feature/design-tools` on 29 October 2026 and frozen for PaperCraft 0.2.0. Only the M1 typography fields below are version 3. The remaining milestone proposals are future work and require version 4 or later.

The [version 1 and 2 rules](document-format.md) remain: strict readers, unknown fields rejected, literals kept synchronized with tokens, and the writer emits the **lowest version that can represent the document**.

## Writer and reader rules

- **Writer.** A document that uses no version 3 field is written exactly as today (version 1 or 2), byte-compatible with v0.1.3. Using any version 3 field writes version 3. Version 3 fields are omitted when they hold their default, so removing the last such value returns the document to version 1 or 2 on the next save.
- **Reader.** Versions 1–3 are accepted. A version 1 or 2 file containing a version 3 field is rejected as malformed, without modification. Versions above 3 are rejected with "created by a newer PaperCraft". v0.1.3 and earlier reject version 3 files as newer, without modification.
- **Validation** runs on load, on every committed edit (GUI and MCP batches) and before every save, so an invalid value is never stored. Numbers must be finite and inside the ranges below.

## Upgrade and preservation

The existing first-save prompt generalizes: when a save would write a higher version than the file was opened with, **Save as new file…** (default) keeps the original bytes, **Upgrade this file** replaces it atomically, **Cancel** writes nothing. Save As and never-saved documents write directly. The MCP `document_save` and CLI never upgrade silently in place: `document_save` to the opened path reports `upgraded_from` in its result when the version rises.

## Milestone fields

### M1 — Typography and Swappable Shapes (this change)

Text layers:

| Field | Default (omitted) | Range | Meaning |
|---|---|---|---|
| `line_height` | absent: font's natural line spacing (today's rendering) | 1–4096 document units | Distance between consecutive baselines, in document units |
| `text_align` | `"left"` | `"left"`, `"center"`, `"right"` | Horizontal alignment of every row inside the layer's width |

Shape layers (`kind`: `"rectangle"`, `"ellipse"`, `"triangle"`):

- `kind: "ellipse"` and `kind: "triangle"` require version 3; `radius` must be `0` on ellipses and triangles.
- A shape layer carrying `line_height`, non-left `text_align` or non-empty `text` is rejected.
- HTML exports ellipses with `border-radius:50%` and triangles with `clip-path:polygon(50% 0%,100% 100%,0% 100%)`; SVG exports `<ellipse>` and `<polygon>`.
- The canvas, inline editor, PNG export and headless rendering share one layout function, so entering or leaving edit mode does not move text. HTML writes `line-height:<n>px` and `text-align`; SVG positions explicit lines at `line_height` spacing with `text-anchor` start/middle/end. Browser and SVG font metrics still differ from egui; automatic wrapping is still not reproduced in SVG.

Rollback: clear line height and set alignment to left on every text layer, and switch any ellipse/triangle layers back to rectangle; the next save writes version 1 or 2.

### M2 — Nested Frames, Vector Paths, Strokes, Procedural Shaders & Directives (implemented)

- **Frames & hierarchy (`kind: "frame"`, `parent`, `clip`):**
  - `kind: "frame"` creates a container layer that defaults to `clip: true`.
  - Any layer may reference an existing frame via `parent: <u64>`. Moving or deleting a frame moves or deletes its descendants; ancestor frames with `clip: true` clip descendant rendering in the canvas, PNG, HTML and SVG exports.
- **Vector Paths (`kind: "path"`, `points`, `closed`):**
  - `points` stores normalized `[x, y]` coordinates in `0.0..=1.0` relative to the layer bounds; `closed` controls whether the path is closed and filled or an open polyline.
- **Strokes (`stroke`, `stroke_token`, `stroke_width`):**
  - Frame, shape and path layers support `stroke: [r, g, b, a]`, optional `stroke_token: <u64>`, and `stroke_width: 0..=256`.
- **Procedural Shaders (`shader`):**
  - Frame and shape layers support `shader: { "kind": "linear" | "radial" | "mesh" | "grain" | "halftone", "secondary": [r, g, b, a], "angle": -360..=360, "intensity": 0..=1 }`.
- **Agent Directives (`directives`, `next_directive_id`):**
  - Top-level `directives` list stores spatial pins (`id`, `layer_id`, `x`, `y`, `message`, `resolved`) rendered as numbered orange badges on the canvas and exposed via CLI inspect and MCP.

### Planned, not yet part of the format

Recorded so later milestones extend one contract instead of competing:

- **M4 Artboards and pages:** the single artboard becomes the first entry of an artboard list; legacy documents load as one page with one artboard.
- **M5 Images (#8):** embedded, size-bounded assets referenced by id, so a moved document keeps its images.
- **M6 Auto layout:** row/column, gap, padding, hug/fill/fixed sizing. A circular sizing rule (a hugging parent with a filling child on the same axis) is rejected at validation rather than resolved to zero.
- **M7 Components and instances.**

Each milestone records its exact fields, ranges, export mapping and rollback here before its schema edits land.
