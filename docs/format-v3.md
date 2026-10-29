# Document format version 3

## Released compatibility contract

Version 3 has an existing release collision. PaperCraft **v0.2.0** (`0e3e7737191e185068c580e1d5fded863ee782e4`) accepts only the M1 typography fields. **v0.3.0** (`53f99a6734473a9ca8a6933b5fb69de291dc1172`) also writes the M2 toolkit fields and shape kinds below as version 3. The earlier frozen-M1 claim did not match the later shipped serializer. A v0.2.0 reader rejects those toolkit files; a shared version number does not imply compatibility.

Current readers preserve the actual v0.3.0 superset. **No more persisted fields or changed coordinate semantics may be added to v3.** New native schema work requires v4 or later with explicit migration/rollback. The workspace increment adds no native fields/version. See the [current roadmap and release fixture provenance](editor-completion-plan.md#format-reconciliation-before-new-fields).

The [version 1 and 2 rules](document-format.md) remain: strict readers, unknown fields rejected, literals kept synchronized with tokens, and the writer emits the **lowest version that can represent the document**.

## Writer and reader rules

- **Writer.** A document that uses no version 3 field is written exactly as today (version 1 or 2), byte-compatible with v0.1.3. Using any version 3 field writes version 3. Version 3 fields are omitted when they hold their default, so removing the last such value returns the document to version 1 or 2 on the next save.
- **Reader.** Versions 1–3 are accepted. A version 1 or 2 file containing a version 3 key is rejected as malformed, including explicit null/default/empty values, without modification. Unknown fields still reject. Versions above 3 are rejected with "created by a newer PaperCraft". v0.1.3 and earlier reject version 3 files as newer; v0.2.0 rejects v3 toolkit data, without modification.
- **Validation** runs on load, on every committed edit (GUI and MCP batches) and before every save, so an invalid value is never stored. Numbers must be finite and inside the ranges below.

## Upgrade and preservation

The existing first-save prompt generalizes: when a save would write a higher version than the file was opened with, **Save as new file…** (default) keeps the original bytes, **Upgrade this file** replaces it atomically, **Cancel** writes nothing. Save As and never-saved documents write directly. Headless MCP `document_save` rejects an in-place version increase unless `allow_upgrade:true` is explicit; successful upgrades report `upgraded_from`. Live MCP still requires GUI confirmation or a new path (the flag cannot bypass it). CLI inspect/export never save the source. This repairs the old headless behavior that wrote first and only reported an upgrade afterward.

## Milestone fields

### M1 — Typography and swappable shapes (released subsets)

Line height/alignment shipped in v0.2.0. Ellipse/triangle kinds were added in v0.3.0 using the same version number; they are not readable by v0.2.0.

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

### M2 — Frames, paths, strokes, shaders and directives (released fields; partial workflows)

- **Frames & hierarchy (`kind: "frame"`, `parent`, `clip`):**
  - `kind: "frame"` creates a container layer that defaults to `clip: true`.
  - Any layer may reference an **earlier** frame via `parent: <u64>`; earlier-parent validation rejects cycles. **All x/y remain absolute world coordinates, including children.** Parent links are not local transforms. Native canvas/PNG rendering intersects rectangular ancestor clips; rounded clipping is not implemented. Frame nudge, pointer movement, resize and other hierarchy operations do not yet have consistent descendant semantics; this is stage 2 work, not completed grouping. HTML/SVG exports reject visible ancestor clipping instead of silently omitting it; use PNG.
- **Vector Paths (`kind: "path"`, `points`, `closed`):**
  - `points` stores normalized straight vertices `[x, y]` in `0.0..=1.0` relative to the layer bounds; `closed` controls closure/fill. Native closed fills currently use convex tessellation, so concave fills are incorrect. The Path preset is not authored Pen, and point/Bézier editing is missing. These fields do not imply a completed vector workflow.
- **Strokes (`stroke`, `stroke_token`, `stroke_width`):**
  - Frame, shape and path layers support `stroke: [r, g, b, a]`, optional `stroke_token: <u64>`, and `stroke_width: 0..=1000` (the released validator's actual range). Stroke/export alignment parity is not yet complete.
- **Procedural Shaders (`shader`):**
  - Frame and shape layers store `shader: { "kind": "linear_gradient" | "radial_gradient" | "mesh" | "grain" | "halftone", "secondary": [r, g, b, a], "angle": <finite number>, "intensity": 0..=1 }`. These are the released serde names/ranges (MCP additionally accepts `linear`/`radial` aliases). Native appearance remains partial; HTML/SVG reject visible shaders rather than approximate/drop them.
- **Agent Directives (`directives`, `next_directive_id`):**
  - Top-level `directives` list stores spatial pins (`id`, `layer_id`, `x`, `y`, `message`, `resolved`) rendered as numbered orange badges on the canvas and exposed via CLI inspect and MCP.

### Planned, not yet part of the format

Recorded so later milestones extend one contract instead of competing:

- **Organization/pages:** migration details are not decided. Existing top-level Frame nodes already provide screen composition in the world workspace. Do not introduce a competing artboard collection or reinterpret parent geometry. See the current roadmap before choosing fields.
- **M5 Images (#8):** embedded, size-bounded assets referenced by id, so a moved document keeps its images.
- **M6 Auto layout:** row/column, gap, padding, hug/fill/fixed sizing. A circular sizing rule (a hugging parent with a filling child on the same axis) is rejected at validation rather than resolved to zero.
- **M7 Components and instances.**

Each milestone records its exact fields, ranges, export mapping and rollback here before its schema edits land.
