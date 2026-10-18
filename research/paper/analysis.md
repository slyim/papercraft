# Paper: initial analysis

Date: 18 October 2026, Europe/Istanbul. This report records the initial static investigation before application development. Authenticated editor behavior remains unverified. The subsequent original Rust/egui implementation uses Phosphor Icons and follows the [implementation process](../../docs/implementation-process.md); implementation results are separate from Paper evidence.

## Main finding

Paper's packaged desktop code is a shell around a web editor. It creates Electron content views, loads an application URL into them, provides native desktop capabilities, and forwards MCP requests into renderer-side handlers. This pass recovered that desktop integration. It did not recover or execute the hosted editor implementation.

Consequently, recreating the desktop shell alone would not produce an independent free editor. The alternative needs its own document model, canvas editing behavior, persistence, import/export, and agent interface. This is an implementation implication inferred from the observed boundary.

## Artifact and evidence

| Item | Observed value |
|---|---|
| Installed application | `/Applications/Paper.app` |
| Version / build | `0.5.16` / `261015tx3mukqib` |
| Package / entry point | `@paper/desktop` / `dist/main.cjs` |
| Analysis target | `Contents/Resources/app.asar` |
| ASAR SHA-256 | `912bad3fe41e226c67fd4e27352e2b4456d69d19589f4583cba5944932fb78b2` |
| REA | `6.7.0` |
| Canonical Evidence ID | `ev_1c175b88e83c79b3e8dcd0af6431ff71727231ad23616097a7247f60b36ef3f2` |
| Selected sources | 60 files: manifest plus 59 TypeScript files |
| REA relevant inventory / unselected files | 4,687 / 4,627 |
| Selected-source parse failures | 0 |
| Graph | 14,071 nodes, 14,861 edges; coverage explicitly partial |
| Electron syntax observations | 75 IPC operations, 43 literal channels, 2 context-bridge APIs |

The [compact summary](evidence-summary.json) retains the exact selection, channel node/observation IDs, source locations, integrity contradiction and limitations. Complete evidence is in the ignored `../private/paper-RB6VUg/evidence.json`. Source paths below refer to members of the inspected ASAR, not files added to Crumbs.

The source-selection count includes the packaged test preload helper. Third-party dependency source, spec files, test directories and `.env` contents were not selected. No target bootstrap or application code was executed. Packaged source observations do not prove byte-for-byte correspondence to all compiled runtime code.

## Desktop findings

| Question | Static observation | Evidence location |
|---|---|---|
| Where does the editor run? | Tabs construct `WebContentsView` objects with a preload and navigate to an application URL plus a route. Environment selection resolves the production client origin for a packaged build. | `src/window/paper-tab.ts:152–161`, `:338`; `src/lib/resolve-environment.ts:1–26` |
| How do agents reach the editor? | The MCP server obtains tool definitions from the client. The desktop bridge routes calls to a selected tab and invokes renderer handlers after waiting for readiness. | `src/mcp/server.ts:46–70`; `src/mcp/bridge.ts:193–250` |
| What local transport exists? | The packaged server declares port 29979, `/mcp`, a host check, browser-request checks and a loopback bind. Runtime enforcement was not tested. | `src/mcp/server.ts:16–17`, `:211–220`, `:362–385` |
| What does the preload expose? | A `paper` bridge includes PDF export and desktop/agent configuration capabilities. Static IPC observations cover tabs, resources, menus, updates and exports. | `src/preload.ts:31`, `:107`, `:260`; channel references in the compact summary |
| What does the desktop persist? | A schema-validated JSON storage utility stores records in an application-data directory, with separate nonproduction paths. This establishes shell storage, not the editor document format. | `src/lib/storage.ts:12–24`, `:33–47`, `:76–126` |
| How is PDF export supported? | Desktop code invokes Chromium PDF printing and uses `pdf-lib` for PDF processing. | `src/exports/pdf-export.ts:2`, `:38–40`, `:75`, `:143` |
| How are offline loads handled? | URL loading has an offline fallback, polling and cancellation of stale retries. This does not demonstrate offline editing. | `src/window/load-url.ts:1–166` |

These observations share the canonical Evidence ID above. IPC channel names indicate communication structure; they do not establish user-visible behavior, reachability or authorization correctness by themselves.

## Publicly documented editor capabilities

The following are vendor-documented capabilities, independently distinguished from installed-version runtime verification.

| Area | Documented behavior | Primary source |
|---|---|---|
| Canvas and layers | HTML/CSS rendering, frames, text, rectangles, images, selection, nesting, ordering, movement, sizing, clipping, snapping and zoom. | [Support and shortcuts](https://paper.design/docs/support), [web-standard canvas](https://paper.design/compare/figma) |
| Layout and styling | Flex layout; positioning and dimensions; fills, gradients, outlines, shadows, filters, typography and multiple color spaces. | [Comparison](https://paper.design/compare/figma), [build log](https://paper.design/build-log) |
| Import | Editable layers from HTML, plus text, images, SVG and Figma clipboard data. HTML import uses inline styles; selector-based class styling is not retained. | [Paste](https://paper.design/docs/paste), [HTML import](https://paper.design/docs/paste/html) |
| Tokens | Named values for colors, dimensions and typography with property binding. Copies between files do not remain synchronized. | [Tokens](https://paper.design/docs/tokens) |
| SVG | Imported/generated vectors, paths, transforms, fill/stroke and point/segment editing. | [SVG](https://paper.design/docs/svg) |
| Exports | Code, PNG and PDF workflows; media-specific export capabilities vary. | [Support](https://paper.design/docs/support), [build log](https://paper.design/build-log) |
| Files and collaboration | Pages, folders, tabs, background-tab agent work, presence and comments. | [Build log](https://paper.design/build-log) |
| Agents | Tree, style, text, screenshot and JSX reads; artboard creation, HTML insertion, style/text updates and duplication. Desktop CLI provides the current stdio entry point. | [MCP documentation](https://paper.design/docs/mcp), [official agent plugin](https://github.com/paper-design/agent-plugins/blob/main/plugins/paper-desktop/README.md) |

The public homepage was rendered in the built-in browser. Its illustrative editor shows a left page/layer sidebar, narrow tool strip, central canvas, top tabs, and right property inspector. This is a marketing illustration, not a live editor interaction. The local reference screenshot is `../private/paper-public-homepage.png`.

The September 2026 build log lists remote MCP, reusable components, rich text, a scale tool and per-file permissions as forthcoming. Some roadmap descriptions lag behind shipped changes such as nested folders and vector creation. Planning should prioritize current documentation and observed behavior over treating every roadmap item as available. [Build log](https://paper.design/build-log), [roadmap](https://paper.design/roadmap)

## Integrity and access limitations

- The ASAR declares a helper hash that differs from the installed unpacked `dist/cli-bin/darwin-arm64/paper`. The first analysis correctly refused it. The subsequent pass explicitly used `record-and-continue`; the affected bytes remain **observed-untrusted** in evidence.
- macOS `codesign --verify --deep --strict` passed outside the sandbox. The same check inside the sandbox misleadingly reported a signature failure. A valid bundle signature does not erase the separate ASAR metadata contradiction.
- The public browser connection was available, but its external-browser control detached during page reads. Built-in browser rendering succeeded. These are browser-access results, not evidence of an application defect.
- Opening the live editor in the built-in browser reached the normal sign-in page. No authenticated editing, document writes, exports, backend traffic, quotas or performance were exercised.
- The selected source parsed successfully, while the complete relationship graph remains partial. Dynamic behavior, unselected dependencies and the hosted renderer remain unknown.
- Native disassemblers and optional instrumentation were not installed. They are unnecessary for this static source pass.

## Questions for the next analysis pass

| Question | Required proof | Current state |
|---|---|---|
| Exact editing interactions and shortcuts | Signed-in editor: select/nest/drag/resize/type/undo | Documented; runtime unverified |
| Source of truth for saved documents | Controlled create/save/reopen/export round trip | Unknown |
| HTML-to-layer normalization | Small controlled imports: flex, text, SVG, images and invalid markup | Documented limits; runtime unverified |
| Layout fidelity and canvas transforms | Nested layouts at several zoom levels and viewport sizes | Unknown |
| Undo/redo and batching semantics | Human edit → agent edit → undo/redo comparison | Unknown |
| Agent mutation and screenshot semantics | Calls against a disposable research document | Publicly documented; runtime unverified |
| Offline behavior | Open/save/reopen with a controlled network interruption | Shell fallback observed; editor behavior unknown |
| Performance | Repeated edits and rendering measurements on increasing node counts | Unmeasured |

## Inputs for implementation planning

The required icon family is [Phosphor Icons](https://github.com/phosphor-icons/react). For a React implementation, use `@phosphor-icons/react`, rather than the legacy `phosphor-react` package. The official package is MIT licensed; preserve its license notice when distributing it. [License](https://github.com/phosphor-icons/react/blob/master/LICENSE)

A proposed first coherent slice is a local document with artboards and basic nodes, a DOM/CSS canvas, selection and property editing, undo/redo, save/reopen, HTML import/export, and an agent edit of that same document. This is a planning proposal, not recovered Paper architecture or an approved implementation plan.

Platform choice, persistence format, renderer architecture, MCP process boundary and initial export targets remain planning decisions. Cloud accounts, collaboration, generation services, shaders and advanced vector/component tools require separate scope decisions. No Paper source, branding or application assets have been added as implementation code.
