# Live MCP

PaperCraft 0.2.0 shares the running editor's document tabs with MCP. It uses the same `Editor`, validation and undo/redo history as manual editing.

1. Open PaperCraft 0.2.0.
2. Register the executable with `mcp` (or `mcp --live` to require a running GUI) in an MCP client. For example: `claude mcp add papercraft -- /absolute/path/to/papercraft mcp`.
3. Call `document_list` to obtain the GUI document IDs. `document_new` and `document_open` show a new tab; successful apply/undo/redo operations show their target tab. Inspect again after manual editing and pass `expected_revision` with a batch.

With several editor instances, choose **Copy live MCP command** in the editor's file menu or dashboard sidebar; its command includes the explicit session file. Session IDs belong to that running editor and expire when it closes. All live file-operation paths must be absolute.

`papercraft mcp` connects to the running editor when one is open, and serves an isolated headless session when none is running. `papercraft mcp --headless` always serves an isolated headless session. `papercraft mcp --live` fails if no live editor is available; it never silently opens another model. It reconnects neither to a different instance nor to headless state after a disconnection.

## State and preservation

- Manual and MCP edits appear through the same GUI event loop and history. Each MCP batch is atomic and one undo step. Inspector values, layers, rendering, dirty checks and saves read that editor.
- While a local text/property edit, gesture, rename or confirmation is pending, tools other than listing return `Editor busy`. No draft is committed or discarded by that request. Finish the interaction and inspect before retrying.
- Reads and exports do not switch tabs. Apply/undo/redo reveal their target tab. Opening a file already in a tab returns that tab rather than reloading disk over unsaved edits.
- Saves and exports cannot replace another open tab's source file; export cannot replace its own source either. An in-place format upgrade requires confirmation in the GUI, or a new Save As path. `allow_upgrade:true` is headless-only and does not bypass live confirmation. Save does not require an interactive file dialog from the agent.
- `document_render` and PNG `document_export` accept `region:{x,y,width,height}` or `frame_id` for bounded world crops, including off-artboard content. They share the canvas painter, resolve absolute world coordinates and report world bounds/pixel dimensions. Frame crops include the visible composition, not only descendants. HTML/SVG region export rejects explicitly. Batch-local typed references and resulting layer bounds are the same as in headless MCP; see the [agent contract](agent-interface.md).
- Requests have a 32 MiB bound, a bounded GUI queue and response deadlines. A timeout may occur after execution started; inspect the state before retrying. The bridge never automatically repeats a mutation.

## Local IPC

Each running editor owns a private temporary session directory under the legacy Crumbs configuration directory's `live` subdirectory, falling back to `papercraft-live` in the local temporary directory when the configuration directory is restricted. Unix directories are mode 0700; Windows inherits the user's configuration-directory ACLs. The session descriptor carries a random 256-bit authentication token. Requests and replies use atomic local file replacement, with a short polling interval and GUI repaint notifications. No TCP port, HTTP service, remote listener or account is used. Processes running as the same user have the user's existing local file access.

`PAPERCRAFT_LIVE_DIR` overrides the discovery directory for isolated tests. Normal shutdown removes the session directory. A heartbeat excludes stale sessions left after a crash. Do not share the session file or its token.

## Format and remaining scope

Versions 1 and 2 remain readable and are still written when later fields are unnecessary. v0.2.0 and v0.3.0 both wrote version 3 with different supported fields; current readers preserve v0.3.0's actual shape/frame/path/stroke/shader/directive superset, with absolute child coordinates. See [format reconciliation](format-v3.md). The workspace increment does not extend v3 or reinterpret coordinates. Images, authored layout, components, pages, full hierarchy semantics and an embedded ACP/provider runner remain unimplemented. The Agent panel is explicitly local notes/presets, not agent execution. The [current roadmap](editor-completion-plan.md) separates working, partial and missing behavior.
