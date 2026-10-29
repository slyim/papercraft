# Agent interface: CLI and local MCP

Date: 29 October 2026, Europe/Istanbul. Resolves [#4 headless inspect and HTML/SVG export](https://github.com/slyim/papercraft/issues/4) and [#5 artboard PNG export](https://github.com/slyim/papercraft/issues/5), and adds the local MCP server previously deferred in the [gap assessment](gap-assessment.md).

All commands use the authoritative `Document`/`Editor` model, its validation, token resolution and atomic writes. No second document model, daemon, network listener or account is involved. The [current completion roadmap](editor-completion-plan.md) overrides historical completion claims. External MCP clients are the supported agent path; the GUI's notes/presets panel is not an ACP/provider runner.

## Command line

```sh
papercraft [FILE]                                   # editor, as before
papercraft inspect FILE                             # document as JSON on stdout
papercraft export FILE --format html|svg|png --output PATH [--scale N]
papercraft mcp                                      # MCP server on stdin/stdout
papercraft --help | --version
```

- stdout carries only machine-readable output (inspection JSON, help, version). Messages go to stderr.
- Exit status: `0` success; `1` document or file error (malformed, newer version, write failure, PNG limits); `2` usage error (unknown command option, format or missing argument).
- `inspect` and `export` never write the source and never touch recents. `export` writes the destination atomically, only after the export succeeds; an existing destination keeps its bytes on any failure. An output path that resolves to the source document is refused.
- A first argument other than these commands opens the editor with that path, as before. Open a document literally named `inspect`, `export` or `mcp` as `./inspect`.
- Windows release builds use the GUI subsystem; commands attach to the parent console only when stdout is not already redirected, so pipes and MCP stdio are unaffected.

### Inspection JSON

```json
{
  "format": "papercraft",
  "version": 3,
  "name": "…",
  "artboard": { "width": 960, "height": 640, "background": "#f8f7f3ff", "background_token": null,
                "background_token_name": null, "canvas": "#282828ff", "canvas_token": null,
                "canvas_token_name": null },
  "tokens": [{ "id": 1, "name": "ink", "value": "#291d35ff" }],
  "layers": [{ "index": 0, "id": 1, "name": "Surface", "kind": "rectangle", "x": 56, "y": 56,
               "width": 848, "height": 528, "fill": "#e2e9dcff", "fill_token": null,
               "fill_token_name": null, "hidden": false, "locked": false, "radius": 20 }],
  "directives": [{ "id": 1, "layer_id": 1, "x": 56, "y": 56, "message": "Check contrast", "resolved": false }]
}
```

Inspection additionally reports `coordinate_space:"world"`, `artboard_bounds`, `content_bounds` (visible clipped union, or null), supported `capabilities`/limitations, and each layer's `bounds`, `visible_bounds`, `effective_hidden` and `effective_locked`. Bounds use `[left,top,right,bottom]` in document units, x right/y down. **All authored x/y are absolute world coordinates, even for children with `parent`.** There is no authored/computed layout yet; the existing geometry is authored geometry, not a layout result. The artboard is the background/default export region; canvas workspace and view state are independent.

Layers are in paint order (index 0 is the bottom). `kind` is `"frame"`, `"rectangle"`, `"ellipse"`, `"triangle"`, `"path"` or `"text"`. Colors are resolved `#rrggbbaa` values; `*_token` fields name bound token identifiers and `*_token_name` fields report the resolved token name. Child layers inside a frame carry `parent`; layers with a stroke carry `stroke`, `stroke_token`, `stroke_token_name` and `stroke_width`; layers with a procedural shader carry `shader` (`{kind, secondary, angle, intensity}`). Frame layers carry `clip` and `radius`; rectangle layers carry `radius`; path layers carry normalized `points` (`[[x, y], …]`) and `closed`; text layers carry `text`, `font_size`, `line_height`, `text_align`, `measured_width`, `measured_height`, `line_count` and `clipped` (whether the laid-out text exceeds `width` or `height`). `directives` lists canvas/layer agent notes and prompts. Keys are sorted, so repeated inspection of the same file is byte-identical. `version` is the lowest format version that represents the document.

## PNG export

`render::export_png` paints the artboard with the same `paint_document` function the canvas uses, in a windowless egui context, tessellates with egui's tessellator and rasterizes the triangles in software. The painter itself is world/workspace-aware; default PNG export supplies the artboard clip. `export_png_region` / `rasterize_region` supply an explicit world crop instead. Consequences:

- Pixel size is `round(artboard × scale)`, independent of window size, zoom, pan and display density. Scale is 0.1–8; at most 16,384 pixels per side and 64 Mi pixels per image. Limits are checked before rendering and before any destination is chosen or written.
- Paint order, hidden-layer omission, token-resolved colors, clamped rectangle radii, alpha, clipping to layer and artboard, and text wrapping match the canvas. Text uses the bundled egui fonts, so glyph coverage is the same as on the canvas (CJK shows placeholder boxes).
- Output is 8-bit straight-alpha RGBA; a transparent artboard background stays transparent.
- Blending happens in gamma space, as egui's own renderer does. Antialiasing comes from egui's feathered tessellation, not supersampling.

The editor's Export menu offers artboard PNG at 1×/2× and selection-region PNG at 1×. Selection crops use the arrangeable selection's union and include the whole visible composition, not isolated layers.

### Bounded agent rendering

`document_render` and PNG `document_export` accept **one** of:

- `region: {x, y, width, height}`: finite world x/y in ±100,000; width/height 1–100,000.
- `frame_id: <integer>`: an existing Frame's absolute world bounds; missing/non-frame IDs reject. This is a composition crop, not subtree isolation, and includes overlapping visible layers.
- Neither: unchanged artboard crop `[0,0,document.width,document.height]`.

Scale remains 0.1–8 (default 1); 16,384-side/64-Mi-pixel limits are checked before rendering/allocation or output replacement. The pixel size is rounded from the requested world span × scale, at least 1 pixel. Replies report exact world `bounds`, `width`, `height` and state. Outside the artboard background is transparent, not the canvas surround color. Ancestor rectangular clips and inherited hiding apply everywhere; rounded frame clipping is not implemented.

```json
{"document_id":"doc-1","frame_id":7,"scale":2}
```

```json
{"document_id":"doc-1","format":"png","path":"/absolute/path/screen.png","region":{"x":-400,"y":0,"width":320,"height":480}}
```

Region/frame crops and scale for HTML/SVG reject before writing. Default HTML/SVG remain artboard-only, with the existing text font/wrap limitations; visible shaders and ancestor clipping now reject with a PNG alternative rather than approximate/disappear. Stroke/path export parity is not complete and is not a completed vector workflow.

## MCP server

`papercraft mcp` speaks JSON-RPC 2.0 over newline-delimited stdin/stdout (MCP protocol `2025-06-18`): `initialize`, `ping`, `tools/list`, `tools/call`. Notifications receive no response. When a PaperCraft editor window is open, `papercraft mcp` connects to its live session automatically; otherwise (or with `papercraft mcp --headless`) it serves an isolated headless session. Use `papercraft mcp --live` to require a live editor; see [live MCP](live-mcp.md).

Register it with an MCP client, for example Claude Code:

```sh
claude mcp add papercraft -- /path/to/papercraft mcp
```

### Tools

| Tool | Purpose |
|---|---|
| `document_new` | Unsaved document (`name`, `width`, `height` optional) |
| `document_open` | Load a file; an already open file returns its existing `document_id` |
| `document_list` | Open documents and their state |
| `document_inspect` | Inspection JSON plus `state` |
| `document_apply` | One all-or-nothing batch of operations = one undo step |
| `document_undo` / `document_redo` | History, shared with batch edits |
| `document_render` | PNG image content (`scale` 0.1–8), optional world `region` or `frame_id`, resulting bounds/dimensions |
| `document_export` | Write html/svg/png to `path` atomically |
| `document_save` | Validate and write atomically to the document's file or `path` |
| `document_close` | Refuses unsaved changes unless `discard: true` |

`document_save` also reports the written `version`, and `upgraded_from` when it raised the format version of the document's own file. Headless in-place version increases require explicit `allow_upgrade:true` or a new path. Live saves require GUI confirmation or a new path; the flag cannot bypass that protection. Saves/exports cannot overwrite another open source, and export cannot overwrite its own source. Document state includes `document_id`, `path`, `revision`, `saved_revision`, `dirty`, `can_undo`, `can_redo` and `selected_ids`. Most document tools return these fields directly; inspection nests them under `state`, listing under `documents[]`, and rendering includes a text state item alongside the image. Closing returns only `closed`. Trust the reported `dirty` value rather than deriving it from revisions: undoing back to saved document contents reports clean, while GUI property/text drafts may change dirty state before their history commit.

### Batch operations

`document_apply` takes `operations` (1–10,000) and optional `expected_revision`. Operations:

- `create_frame`, `create_rectangle`, `create_ellipse`, `create_triangle`, `create_shape` (`shape`: `"rectangle"` | `"ellipse"` | `"triangle"`), `create_path`, `create_text`: `name`, `parent` (frame layer id or `null`), `x`, `y`, `width`, `height`, `fill`, `fill_token`, `stroke` (color or `null`), `stroke_token`, `stroke_width` (0–1000; defaults to `1` when `stroke` or `stroke_token` is set while width is `0`), `hidden`, `locked`, `index`; frames also `clip` (`bool`, default `true`) and `radius` (default `12`); rectangles also `radius`; frames and shapes also `shader` (`null` or `{kind: "linear_gradient"|"radial_gradient"|"mesh"|"grain"|"halftone", secondary?, angle?, intensity?}`); paths also `points` (`[[x, y], …]` in `0..=1`), `svg_path` (`"M x y L x y ... Z"` parsed and normalized into `points` and `closed`) and `closed` (`bool`); text also `text`, `font_size`, `line_height` (1–4096 or `null` for the font's spacing), `text_align` (`left`, `center`, `right`) and `fit_text` (`true` auto-sizes `height` to `ceil(measured_height)`).
- `update_layer`: `id` plus any field above valid for its kind (and `shape` to swap a shape layer between `"rectangle"`, `"ellipse"` and `"triangle"`).
- `wrap_in_frame` `{ids, name?}`: creates a padded frame enclosing `ids`, parents those layers to the frame, selects the new frame and reports its id in `created`.
- `delete_layers` `{ids}`, `move_layer` `{id, index}`, `select_layers` `{ids}` (empty array clears selection).
- `align_layers` `{ids, align}` (`left`, `center`/`center_x`, `right`, `top`, `middle`/`center_y`, `bottom`) and `distribute_layers` `{ids, axis}` (`horizontal`, `vertical`).
- `duplicate_layers` `{ids, offset_x?, offset_y?}` (defaults to +20, +20; selects the duplicates and reports their ids in `created`).
- `set_artboard`: `name`, `width`, `height`, `background`, `background_token`, `canvas`, `canvas_token`.
- `create_token` `{name, value}`, `update_token` `{id, name?, value?}`, `delete_token` `{id|name}` (bound properties keep the token's color).
- `create_directive` `{layer_id?, x?, y?, message}`, `resolve_directive` `{id, resolved?}`, `delete_directive` `{id}`: create, resolve or remove canvas/layer agent notes and prompts.

Colors are `"#rgb"`, `"#rrggbb"`, `"#rrggbbaa"` or `[r, g, b, a]`. Token references (`fill_token`, `stroke_token`, `background_token`, `canvas_token`, `update_token.id`, `delete_token.id`) accept either a numeric token ID or a token name string (including tokens created earlier in the same batch). A literal `fill` or `stroke` detaches a bound token; `fill_token: null` or `stroke_token: null` detaches and keeps the current color; `stroke: null` removes the stroke and detaches `stroke_token`.

### Typed batch-local references

A single-result `create_*` or `wrap_in_frame` operation may add `ref:"label"` (1–64 ASCII letters/digits/`_`/`-`). Later operations in that **same batch** may use `{"ref":"label"}` instead of a matching numeric ID in `id`, `ids[]`, `parent`, `layer_id`, or token-binding fields. References are typed as layer/token/directive; wrong-kind, duplicate, unknown and forward references reject atomically. Numeric IDs and token-name strings remain accepted. Duplicate operations that create several results cannot define one `ref`.

```json
{"document_id":"doc-1","expected_revision":0,"operations":[
  {"op":"create_frame","ref":"screen","x":-400,"y":0,"width":320,"height":480},
  {"op":"create_token","ref":"ink","name":"ink","value":"#20392e"},
  {"op":"create_text","parent":{"ref":"screen"},"x":-376,"y":24,"text":"Settings","fill_token":{"ref":"ink"}},
  {"op":"select_layers","ids":[{"ref":"screen"}]}
]}
```

References are not persisted and do not replace stable IDs across requests. Successful batches return `refs:{label:id}`, ordered `created` entries (with resulting final `bounds` for layers still present), `content_bounds`, and state. Inspect before refinement and reuse the resulting numeric IDs/revision.

Rules:

1. The batch commits through `Editor::apply`, which retains a document snapshot for rollback. An operation error names its index; final document-validation errors report the invalid state. Either failure preserves the document and history.
2. `expected_revision`, when given, must equal the current revision; otherwise the batch is rejected with the current revision so the client re-inspects first.
3. Unknown fields and operation names are errors, not ignored.
4. Created layer, frame, token and directive identifiers are returned in order as `created`. Selection-only batches do not manufacture a document history step.
5. `svg_path` is a bounded absolute M/L contour with an optional final Z, not an SVG importer. Its legacy normalization is preserved: values all in 0–1 stay unchanged; otherwise values all in 0–100 divide by 100; other coordinates map to the contour's axis-aligned box (axis spans at least 1). This is not SVG user-space placement. Relative commands, curves, multiple subpaths, incomplete pairs and commands after closure reject instead of silently becoming vertices. Existing saved normalized points remain unchanged.

These rules come from failures observed while automating other editors on 29 October 2026: silently ignored layout values, geometry collapsed to zero, saves that never reached storage, and stale state after reconnects. PaperCraft rejects instead of storing invalid state, and reports the revision and saved state on every call.

## Not included

Pages/multi-screen organization, bitmap images, layout/components, drop shadows, typography beyond font size/line height/alignment, and non-color tokens remain outside this interface because the document model does not have them yet. Top-level frames already support screen composition in the world workspace. Full hierarchy semantics and authored vector editing are incomplete, not completed tools. Each would extend the batch operations when the model gains the feature.
