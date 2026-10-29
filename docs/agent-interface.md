# Agent interface: CLI and local MCP

Date: 29 October 2026, Europe/Istanbul. Resolves [#4 headless inspect and HTML/SVG export](https://github.com/slyim/papercraft/issues/4) and [#5 artboard PNG export](https://github.com/slyim/papercraft/issues/5), and adds the local MCP server previously deferred in the [gap assessment](gap-assessment.md).

All commands use the authoritative `Document`/`Editor` model, its validation, token resolution and atomic writes. No second document model, daemon, network listener or account is involved.

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

Layers are in paint order (index 0 is the bottom). `kind` is `"frame"`, `"rectangle"`, `"ellipse"`, `"triangle"`, `"path"` or `"text"`. Colors are resolved `#rrggbbaa` values; `*_token` fields name bound token identifiers and `*_token_name` fields report the resolved token name. Child layers inside a frame carry `parent`; layers with a stroke carry `stroke`, `stroke_token`, `stroke_token_name` and `stroke_width`; layers with a procedural shader carry `shader` (`{kind, secondary, angle, intensity}`). Frame layers carry `clip` and `radius`; rectangle layers carry `radius`; path layers carry normalized `points` (`[[x, y], …]`) and `closed`; text layers carry `text`, `font_size`, `line_height`, `text_align`, `measured_width`, `measured_height`, `line_count` and `clipped` (whether the laid-out text exceeds `width` or `height`). `directives` lists canvas/layer agent notes and prompts. Keys are sorted, so repeated inspection of the same file is byte-identical. `version` is the lowest format version that represents the document.

## PNG export

`render::export_png` paints the artboard with the same `paint_document` function the canvas uses, in a windowless egui context, tessellates with egui's tessellator and rasterizes the triangles in software. Consequences:

- Pixel size is `round(artboard × scale)`, independent of window size, zoom, pan and display density. Scale is 0.1–8; at most 16,384 pixels per side and 64 Mi pixels per image. Limits are checked before rendering and before any destination is chosen or written.
- Paint order, hidden-layer omission, token-resolved colors, clamped rectangle radii, alpha, clipping to layer and artboard, and text wrapping match the canvas. Text uses the bundled egui fonts, so glyph coverage is the same as on the canvas (CJK shows placeholder boxes).
- Output is 8-bit straight-alpha RGBA; a transparent artboard background stays transparent.
- Blending happens in gamma space, as egui's own renderer does. Antialiasing comes from egui's feathered tessellation, not supersampling.

The editor's Export menu offers PNG at 1× and 2×.

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
| `document_render` | PNG image content (`scale` 0.1–8) |
| `document_export` | Write html/svg/png to `path` atomically |
| `document_save` | Validate and write atomically to the document's file or `path` |
| `document_close` | Refuses unsaved changes unless `discard: true` |

`document_save` also reports the written `version`, and `upgraded_from` when it raised the format version of the document's own file. Every successful result includes `state`: `document_id`, `path`, `revision`, `saved_revision`, `dirty`, `can_undo`, `can_redo`, `selected_ids`. `dirty` is `revision != saved_revision`, so undoing back to the saved state reports a clean document.

### Batch operations

`document_apply` takes `operations` (1–10,000) and optional `expected_revision`. Operations:

- `create_frame`, `create_rectangle`, `create_ellipse`, `create_triangle`, `create_shape` (`shape`: `"rectangle"` | `"ellipse"` | `"triangle"`), `create_path`, `create_text`: `name`, `parent` (frame layer id or `null`), `x`, `y`, `width`, `height`, `fill`, `fill_token`, `stroke` (color or `null`), `stroke_token`, `stroke_width` (0–1000; defaults to `1` when `stroke` or `stroke_token` is set while width is `0`), `hidden`, `locked`, `index`; frames also `clip` (`bool`, default `true`) and `radius` (default `12`); rectangles also `radius`; frames, shapes and paths also `shader` (`null` or `{kind: "linear_gradient"|"radial_gradient"|"mesh"|"grain"|"halftone", secondary?, angle?, intensity?}`); paths also `points` (`[[x, y], …]` in `0..=1`), `svg_path` (`"M x y L x y ... Z"` parsed and normalized into `points` and `closed`) and `closed` (`bool`); text also `text`, `font_size`, `line_height` (1–4096 or `null` for the font's spacing), `text_align` (`left`, `center`, `right`) and `fit_text` (`true` auto-sizes `height` to `ceil(measured_height)`).
- `update_layer`: `id` plus any field above valid for its kind (and `shape` to swap a shape layer between `"rectangle"`, `"ellipse"` and `"triangle"`).
- `wrap_in_frame` `{ids, name?}`: creates a padded frame enclosing `ids`, parents those layers to the frame, selects the new frame and reports its id in `created`.
- `delete_layers` `{ids}`, `move_layer` `{id, index}`, `select_layers` `{ids}` (empty array clears selection).
- `align_layers` `{ids, align}` (`left`, `center`/`center_x`, `right`, `top`, `middle`/`center_y`, `bottom`) and `distribute_layers` `{ids, axis}` (`horizontal`, `vertical`).
- `duplicate_layers` `{ids, offset_x?, offset_y?}` (defaults to +20, +20; selects the duplicates and reports their ids in `created`).
- `set_artboard`: `name`, `width`, `height`, `background`, `background_token`, `canvas`, `canvas_token`.
- `create_token` `{name, value}`, `update_token` `{id, name?, value?}`, `delete_token` `{id|name}` (bound properties keep the token's color).
- `create_directive` `{layer_id?, x?, y?, message}`, `resolve_directive` `{id, resolved?}`, `delete_directive` `{id}`: create, resolve or remove canvas/layer agent notes and prompts.

Colors are `"#rgb"`, `"#rrggbb"`, `"#rrggbbaa"` or `[r, g, b, a]`. Token references (`fill_token`, `stroke_token`, `background_token`, `canvas_token`, `update_token.id`, `delete_token.id`) accept either a numeric token ID or a token name string (including tokens created earlier in the same batch). A literal `fill` or `stroke` detaches a bound token; `fill_token: null` or `stroke_token: null` detaches and keeps the current color; `stroke: null` removes the stroke and detaches `stroke_token`.

Rules:

1. The batch commits through `Editor::apply`, which retains a document snapshot for rollback. An operation error names its index; final document-validation errors report the invalid state. Either failure preserves the document and history.
2. `expected_revision`, when given, must equal the current revision; otherwise the batch is rejected with the current revision so the client re-inspects first.
3. Unknown fields and operation names are errors, not ignored.
4. Created layer, frame, token and directive identifiers are returned in order as `created`.

These rules come from failures observed while automating other editors on 29 October 2026: silently ignored layout values, geometry collapsed to zero, saves that never reached storage, and stale state after reconnects. PaperCraft rejects instead of storing invalid state, and reports the revision and saved state on every call.

## Not included

Multiple artboards, bitmap images, drop shadows, typography beyond font size/line height/alignment, and non-color tokens remain outside this interface because the document model does not have them yet. Each would extend the batch operations when the model gains the feature.
