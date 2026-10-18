# Paper workflow evidence

18 October 2026, Europe/Istanbul. This supplements [the initial analysis](analysis.md); it does not replace its limitations. Proprietary material and raw connector responses remain in ignored `research/private/`. No Paper code, fonts, branding or assets were incorporated into PaperCraft.

## Identity and retained evidence

The installed `/Applications/Paper.app` still reports **0.5.16**, build **261015tx3mukqib**. A fresh SHA-256 of `Contents/Resources/app.asar` is `912bad3fe41e226c67fd4e27352e2b4456d69d19589f4583cba5944932fb78b2`, exactly matching E1, `ev_1c175b88e83c79b3e8dcd0af6431ff71727231ad23616097a7247f60b36ef3f2`. Repeating the static analysis was unnecessary.

E1 covers 60 selected files and leaves 4,627 inventoried files unselected; relationship coverage is partial. Its unpacked CLI helper has an ASAR-integrity mismatch and remains **observed-untrusted**. The new work neither executed that helper nor treats the bundle hash as proof of hosted-editor identity.

| Record | Evidence and scope |
|---|---|
| E1, reused static observation | Hosted editor boundary: `src/window/paper-tab.ts:152–161,338`, `src/lib/resolve-environment.ts:1–26`. MCP renderer forwarding: `src/mcp/server.ts:46–70`, `src/mcp/bridge.ts:193–250`. Shell JSON storage: `src/lib/storage.ts:12–24,33–47,76–126`; it is not the editor document format. |
| R1, live connector observation | `get_guide`, `get_basic_info`, `get_selection`, after launching the installed app. The active Scratchpad exposed one page, zero artboards and zero selected nodes. No writes occurred. This is connector evidence, **not a REA Evidence ID**. |
| D1, public documentation | Official support, SVG, HTML paste, tokens, MCP, comparison and build-log pages consulted on the date above. These are vendor claims, not installed-version interaction tests. |

Raw records: `research/private/paper-workflow-2026-10-18/identity-and-access.json`, `mcp-read-only.json` (SHA-256 `1587ebcf68d0617d2a0333eec9f9d29413ec94856ad1ac60ac4bc51d525e4d73`), `public-docs.json` (SHA-256 `2493d401409db56f2880cfe7ef9986279f09452aa95ad56197c5a37f8c6f6cd9`). D1 preserves retrieved text, not a complete website archive.

The initial connector call reported that Paper Desktop was not running. Launch Services inside the sandbox then reported `kLSNoExecutableErr`; the same authorized launch outside the sandbox succeeded, followed by successful connector reads. This was an environment/access limitation, not evidence of an application defect or invalid credentials. No authenticated Paper editing, undo or export was performed. No screenshot is used as evidence of working behavior.

## Documented workflows and implications

| Workflow | Verified source claim | PaperCraft implication / unknown |
|---|---|---|
| Selection and hierarchy | Paper documents parent/child navigation, deep selection, sibling selection, layer order, locking, visibility and alignment. [Support](https://paper.design/docs/support) | A flat single-selection model is partial parity. Exact hit-testing and transaction rules need controlled runtime tests. |
| Canvas transforms | Paper documents pan, zoom-to-fit/selection, nudging, resizing and pixel snapping. [Support](https://paper.design/docs/support) | Keep geometry independent of viewport zoom; test gesture batching locally. Paper's snapping heuristics remain unknown. |
| Typography | Size, weight, spacing, line height and text alignment have controls/shortcuts. [Support](https://paper.design/docs/support) | Current PaperCraft plain text/font size is partial. Font shaping/fallback parity is unverified. |
| SVG and images | SVG paste imports editable vector contents; layer transforms, fills, strokes and path editing are documented. [SVG](https://paper.design/docs/svg) | SVG exchange is evidence-supported; exact cross-app text metrics and editability need actual target inspection. |
| HTML/layout | Imported HTML becomes editable layers; inline styles are supported, selector styling is discarded, and images require reachable URLs. [HTML paste](https://paper.design/docs/paste/html) Paper advertises HTML/CSS canvas rendering and flexbox. [Comparison](https://paper.design/compare/figma) | egui absolute positioning is a bounded implementation, not a CSS engine. HTML import and nested layout require separate contracts. |
| Styling | Named tokens bind shared color, spacing, radius and typography values. [Tokens](https://paper.design/docs/tokens) | Token bindings require a model design; do not expose disconnected controls. |
| Export and history | The August 2026 log records fixes to token export in SVG/PDF. Earlier entries record redo preservation after selection, Unicode text fixes and a one-action undo fix. [Build log](https://paper.design/build-log) | Export safety, Unicode and batching are useful reliability priorities; exact Paper undo boundaries remain unknown. |
| Automation and multiple files | Desktop CLI stdio reaches the open design; the legacy localhost endpoint is still documented. [MCP](https://paper.design/docs/mcp) The build log documents background-tab agent work and multiple-frame PDF export. [Build log](https://paper.design/build-log) | A future command/MCP surface should mutate the authoritative Editor. Transport reachability does not establish authentication or mutation semantics. |

September 2026's build log still lists rich text, reusable components, a scale tool and remote MCP as forthcoming. The SVG page contains older roadmap wording alongside shipped path creation instructions. These conflicting/stale sections are not treated as proof that every roadmap feature works today. [Build log](https://paper.design/build-log), [SVG](https://paper.design/docs/svg)

## Selected implementation boundary

**Inference / PaperCraft decision:** bounded SVG export is the first coherent exchange slice after demonstrated preservation/history/input fixes. It reaches a documented Paper format and the existing [Craft integration roadmap](../../docs/artcraft.md) using the current rectangle/text/artboard model. It needs no schema change, imported assets, remote services or second document model.

The contract is validated artboard dimensions/viewBox; ordered visible rectangles and plain text; RGBA fills, radii, clipping and XML escaping; atomic output; clear typography limits. HTML remains the browser-layout handoff and `.papercraft`/legacy `.crumbs` version 1 remain the editing source. SVG is not a Paper file conversion or a lossless round trip. Cross-app compatibility may be reported only for the fixture and target revision actually inspected. [Gap assessment](../../docs/gap-assessment.md), [verification](../../docs/verification.md)

## Color tokens and arrangement (D2)

Checked 18 October 2026. The `app.asar` SHA-256 still equals E1 (`912bad3f…b78b2`) and the version is still 0.5.16. A static search of E1's evidence for token or alignment terms found only unrelated authentication tokens and a PDF-library text alignment helper. The token editor lives in the hosted editor that E1 does not contain, so no new REA analysis was useful. No Paper runtime interaction was performed and no Paper connector was available in this session.

D2 is a refetch of the [tokens](https://paper.design/docs/tokens), [support](https://paper.design/docs/support) and [build log](https://paper.design/build-log) pages. All three returned HTTP 200 and show no last-updated date. A paraphrased private record is at `research/private/paper-tokens-2026-10-18/docs-refetch.md` (SHA-256 `2138108b499271d0baa607e0a02f74e3efbc2a15cc254334695df55322bfee60`).

| Kind | Content |
|---|---|
| Documented | Token types include color, radius, spacing, typography and more. Tokens are created in a Theme tab and bound from property dropdowns, with a four-circle control for colors. Updates propagate. Detach keeps the current value. Tokens can be renamed, duplicated and deleted. Align shortcuts are ⌥W/S/A/D/V/H, distribute is ⌃⌥H/V, nudge is 1 px (Shift: 8 px default) and ⌘A selects siblings. In the build log: in-app tokens and a token SVG/PDF export fix (August 2026), creating a color token from a property panel (September 2026), and a fix that stopped locked/hidden elements being selectable on the canvas (January 2026). |
| Not documented / unknown | The effect of deleting a token on bound properties, undo for token changes, page/artboard color tokens, the modifier for adding to a canvas selection, align reference bounds and single-layer behavior, distribute semantics, and how hidden/locked layers affect arrangement. |
| PaperCraft decisions | All of the unknown items above. They are recorded in the [implementation process](../../docs/implementation-process.md#color-tokens-and-arrangement-increment--18-october-2026). Screenshots of Paper were not used. |
