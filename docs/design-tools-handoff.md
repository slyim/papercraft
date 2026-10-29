# Design tools after v0.2.0

This is a proposed implementation program, not a list of shipped features. Start from the published `v0.2.0` tag and inspect later main commits before creating a new worktree. The release already includes typography, headless commands, PNG and live MCP. The old toolbar/PRODUCT notes contain historical gaps; current source, the release notes and `docs/live-mcp.md` establish the baseline.

## Names and organization

Use these user-facing names. Drawing modes belong in the rail; selection commands belong in menus; properties belong in the inspector. Pages, layers and reusable assets belong in navigation panels. Only implemented behavior is exposed.

| Section | Names | Surface | State at v0.2.0 |
|---|---|---|---|
| Navigate | Select, Hand | Rail | Available |
| Create | Rectangle, Text | Rail | Available |
| Create | Frame, Ellipse, Line, Pen, Image | Rail / grouped shape picker | Planned |
| Organize | Group, Ungroup, Wrap in Frame | Selection actions | Planned |
| Document | Pages, Artboards, Layers | Navigator | Flat layers and one artboard available; pages/multiple artboards planned |
| Arrange | Align, Distribute, Snapping | Selection actions / inspector | Available for supported layers; creation snapping planned |
| Layout | Auto Layout, Padding, Gap, Hug, Fill, Fixed | Inspector | Planned |
| Style | Fill, Color Tokens, Corner Radius | Inspector / Theme panel | Available; radius is uniform |
| Style | Stroke, Per-corner Radius, Opacity, Shadow | Inspector | Planned; layer opacity is distinct from existing fill alpha |
| Effects | Gradient, Grain | Inspector | Planned; custom shaders/video export require a separate scope decision |
| Reuse | Components, Instances, Overrides | Assets panel / inspector | Planned |
| Review | Comment, Resolve Comment | Local comment mode / panel | Planned; no account or collaboration service |
| Automate | Live MCP | Connection command / file menu | Available; shares GUI state and history |

Generate Image and Generate SVG remain deferred until a provider/local-model, credential and cost policy is explicitly authorized. Ellipse and Line are proposed PaperCraft tools; no claim is made that they were observed in Paper.

## Delivery order

1. **Frames and Groups:** hierarchy, nested selection/tree layers, relative geometry, clipping, wrap/unwrap and reparenting.
2. **Shapes and Style:** Ellipse/Line, per-corner radius, stroke, node opacity and one bounded shadow. Preserve token semantics.
3. **Pages and Artboards:** stable identities, active scope, navigation, export selection and undo across scope changes.
4. **Images:** portable local storage, decoded-size and total-size budgets, placement/replacement, fit/fill/crop.
5. **Auto Layout:** row/column, gap/padding, fixed/hug/fill, min/max constraints, deterministic nested layout and cycle rejection.
6. **Pen and Vector Editing:** open/closed paths, corner/smooth points, handles, path edit mode and SVG output.
7. **Components:** definitions, instances, explicit overrides, detach and recursion validation.
8. **Local Comments:** anchored pins, text, resolution, stated orphan/deletion behavior, no appearance in design exports.
9. **Effects:** bounded original gradients/grain with repeatable rendering and supported export mapping. Keep advanced shader/video work deferred.

One implementation owner coordinates serialization and shared files. Published formats 1-3 are frozen. Start a documented version-4 contract for the next coordinated unreleased changes; after any release freezes version 4, later fields require another version. Always emit the lowest representable version.

## Agent prompt

```text
Implement PaperCraft's next design-tool milestones from the completed v0.2.0 release.

Repository: slyim/papercraft (Rust/egui desktop editor).
Original checkout: C:\Users\shyle\papercraft.
The verified release checkout used in the preceding task is:
C:\Users\shyle\Documents\Codex\2026-10-29\new-chat-3\papercraft-finalize.
Do not assume the original local main is current. Its old design worktree could not
be cleaned because this session could not write its Git metadata.

Read AGENTS.md first. Read DESIGN.md, docs/implementation-process.md,
docs/document-format.md, docs/format-v3.md, docs/live-mcp.md and this handoff.
Read relevant tests and code before changing their contracts. Treat older plans
that call hierarchy 'format 3' or MCP/PNG 'missing' as historical. Version 3 has
shipped with typography and is frozen.

Fetch origin and verify the published v0.2.0 tag, release source and current main.
Preserve any later main commits. Create one isolated feature/design-tools-next
worktree from the current main. Inventory uncommitted/unique work first; never
reset, delete or overwrite an unrelated checkout or handoff file.

Use the names, surfaces and ordered milestones in this document. Implement in
that order, beginning with Frames and Groups. Complete a usable milestone, its
tests and native proof before starting another. Commit milestones separately.
Do real implementation work rather than stopping after a plan. If a later stage
cannot be finished, leave the last completed milestone reviewable and report
exactly what remains; never label placeholders as implemented.

All application code is original Rust. Retain egui/eframe, Phosphor Icons and
the existing interface direction. Do not copy Paper source/assets or migrate
the app to a browser renderer. No cloud accounts, provider keys, paid generation,
collaboration service, advanced shader engine or speculative engine rewrite.

Before adding each serialized feature, document its trigger, visible behavior,
data fields/defaults, limits, coordinate system, export mapping, invalid-input
behavior and migration/rollback rules. One owner controls the next schema.
Preserve strict readers, versions 1-3, lowest-version writing, atomic file output
and explicit in-place upgrades. Reject unsupported newer fields without altering
existing documents or files. Do not continue adding fields to a frozen release.

Frames/Groups acceptance:
- Use stable IDs, one hierarchy representation and one world/local transform path.
- Validate missing/non-container parents, cycles, paint order and subtree contiguity.
- Preserve world geometry on wrap, unwrap and reparent; each action is one undo step.
- Define selection/deep-selection, ancestor hide/lock, clipping and resize semantics.
- Layer navigation, hit testing, canvas, inspector, HTML/SVG/PNG and MCP must agree.
- Old flat documents load unchanged and remain old-format until newer features are used.

For every subsequent milestone, preserve the same complete path:
GUI creation/editing -> Editor transaction -> validation -> undo/redo -> save/reopen
-> HTML/SVG/PNG as supported -> headless MCP -> live MCP -> visible GUI state.
Add relevant MCP operations, input schemas and inspection fields together.
Keep the GUI event loop authoritative. Preserve stale-revision checks, draft and
gesture conflicts, cross-tab save protection and no silent headless fallback.

Keep TOOLS as the source for implemented rail modes, labels, icons, groups and
shortcuts. Keep V/H/R/T and existing navigation behavior. F currently fits the
artboard: audit shortcut collisions before assigning Frame or other new modes.
Do not turn inspector properties or organizing actions into extra drawing modes.

Use meaningful regressions for uncertain behavior: nested transforms/clipping,
failed-batch rollback including redo, invalid/oversized assets, layout cycles,
old-file preservation and token deletion/binding. Do not add source-scanner tests
or tests that merely mirror implementation structure. No skipped/weakened gates.

Run cargo fmt --check, cargo clippy --locked --all-targets -- -D warnings,
and cargo test --locked. Run relevant native scenarios and extend the native live
MCP example to prove newly supported nodes. Open exported PNGs and native captures.
Report actual counts, failures, platforms and manual checks not performed.

On this Windows host use the installed GNU Rust toolchain with Scoop MinGW;
do not switch to MSVC without its linker. In Git Bash source
~/penpot-mcp/work/rustenv.sh. If the sandbox default TEMP rejects atomic replacement,
use C:\Users\shyle\AppData\Local\Temp for TEMP/TMP. Do not disable validation,
tests or lints to work around environment failures.

Use Conventional Commits and an accurate Co-Authored-By footer. Keep docs concise
and separate shipped behavior from plans. Never force-push or rewrite published
history, change old release tags/assets, or use --no-verify. Do not publish a new
release automatically under this prompt. Report completed milestones, commits,
verification evidence and the next bounded milestone for review.
```
