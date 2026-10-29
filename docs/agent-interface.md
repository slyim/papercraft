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
  "version": 2,
  "name": "…",
  "artboard": { "width": 960, "height": 640, "background": "#f8f7f3ff", "background_token": null,
                "canvas": "#282828ff", "canvas_token": null },
  "tokens": [{ "id": 1, "name": "ink", "value": "#291d35ff" }],
  "layers": [{ "index": 0, "id": 1, "name": "Surface", "kind": "rectangle", "x": 56, "y": 56,
               "width": 848, "height": 528, "fill": "#e2e9dcff", "fill_token": null,
               "hidden": false, "locked": false, "radius": 20 }]
}
```

Layers are in paint order (index 0 is the bottom). Colors are resolved `#rrggbbaa` values; `*_token` fields name bound token identifiers. Text layers carry `text` and `font_size` instead of `radius`. Keys are sorted, so repeated inspection of the same file is byte-identical. `version` is the lowest format version that represents the document.

## PNG export

`render::export_png` paints the artboard with the same `paint_document` function the canvas uses, in a windowless egui context, tessellates with egui's tessellator and rasterizes the triangles in software. Consequences:

- Pixel size is `round(artboard × scale)`, independent of window size, zoom, pan and display density. Scale is 0.1–8; at most 16,384 pixels per side and 64 Mi pixels per image. Limits are checked before rendering and before any destination is chosen or written.
- Paint order, hidden-layer omission, token-resolved colors, clamped rectangle radii, alpha, clipping to layer and artboard, and text wrapping match the canvas. Text uses the bundled egui fonts, so glyph coverage is the same as on the canvas (CJK shows placeholder boxes).
- Output is 8-bit straight-alpha RGBA; a transparent artboard background stays transparent.
- Blending happens in gamma space, as egui's own renderer does. Antialiasing comes from egui's feathered tessellation, not supersampling.

The editor's Export menu offers PNG at 1× and 2×.

## MCP server

`papercraft mcp` speaks JSON-RPC 2.0 over newline-delimited stdin/stdout (MCP protocol `2025-06-18`): `initialize`, `ping`, `tools/list`, `tools/call`. Notifications receive no response. The server edits files on disk with the permissions of the user who starts it; it does not connect to a running editor window.

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

Every successful result includes `state`: `document_id`, `path`, `revision`, `saved_revision`, `dirty`, `can_undo`, `can_redo`. `dirty` is `revision != saved_revision`, so undoing back to the saved state reports a clean document.

### Batch operations

`document_apply` takes `operations` (1–10,000) and optional `expected_revision`. Operations:

- `create_rectangle`, `create_text`: `name`, `x`, `y`, `width`, `height`, `fill`, `fill_token`, `hidden`, `locked`, `index`; rectangles also `radius`, text also `text`, `font_size`.
- `update_layer`: `id` plus any field above valid for its kind.
- `delete_layers` `{ids}`, `move_layer` `{id, index}`.
- `set_artboard`: `name`, `width`, `height`, `background`, `background_token`, `canvas`, `canvas_token`.
- `create_token` `{name, value}`, `update_token` `{id, name?, value?}`, `delete_token` `{id}` (bound properties keep the token's color).

Colors are `"#rgb"`, `"#rrggbb"`, `"#rrggbbaa"` or `[r, g, b, a]`. A literal `fill` detaches a bound token; `fill_token: null` detaches and keeps the current color.

Rules:

1. The batch is applied to a copy of the document and committed through `Editor::apply`. If any operation fails, or the result fails document validation (geometry limits, finite numbers, token names, identifier rules), nothing changes and the error names the failing operation.
2. `expected_revision`, when given, must equal the current revision; otherwise the batch is rejected with the current revision so the client re-inspects first.
3. Unknown fields and operation names are errors, not ignored.
4. Created layer and token identifiers are returned in order as `created`.

These rules come from failures observed while automating other editors on 29 October 2026: silently ignored layout values, geometry collapsed to zero, saves that never reached storage, and stale state after reconnects. PaperCraft rejects instead of storing invalid state, and reports the revision and saved state on every call.

## Not included

Live editing of an open editor tab, multiple artboards, groups/frames, images, strokes/shadows, typography beyond font size, and non-color tokens remain outside this interface because the document model does not have them yet. Each would extend the batch operations when the model gains the feature.
