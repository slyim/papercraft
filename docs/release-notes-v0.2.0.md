# PaperCraft 0.2.0 preview

- Live MCP: connect an agent to open GUI tabs using `papercraft mcp --live`. Changes appear on the canvas and share manual undo/redo history.
- Local drafts, gestures, save conflicts and format upgrades are protected. Multiple instances can be selected using the session command in the file menu. No network listener is required.
- Integrated design-tool typography: line height and left/center/right alignment in the canvas, inline editor, inspector, MCP, PNG and existing HTML/SVG exports.
- Headless inspection, HTML/SVG/PNG export and isolated MCP remain available. PNG shared triangle edges no longer darken translucent surfaces.

Documents without newer features retain format version 1 or 2. Authored typography uses version 3; older releases reject it without modifying the source. This freezes version 3. Frames, images and auto layout are future work.

Five native targets build and test before release publication. Download the matching installer or portable archive and verify it using `SHA256SUMS.txt`. macOS remains ad-hoc signed/not notarized; Windows is unsigned. Physical-input, IME, accessibility and third-party MCP-client integration checks remain incomplete. See [live MCP documentation](https://github.com/slyim/papercraft/blob/main/docs/live-mcp.md).
