# Document format v4 — authored Bézier paths

Implementation contract for the Pen increment. v1–v3 remain frozen as documented in `format-v3.md`. No parent coordinates change: all node x/y are absolute world units. GUI starter Frames use existing v3 fields and a transparent legacy export backing; no workspace-mode field is added.

## Canonical geometry

A Path may have `anchors`, an array of 2–1,024 records. Each record has:

- `position: [x,y]` — normalized relative to the node x/y/width/height.
- Optional `incoming: [x,y]`, `outgoing: [x,y]` — control-point positions in the same normalized coordinate space, not offsets. Missing handles lie on the anchor. Handles can extend outside 0–1.
- `mode: "corner" | "smooth" | "symmetric"` (default corner, omitted).

`anchors` is omitted when empty. Nonempty `anchors` and nonempty legacy `points` are mutually exclusive; both reuse node `closed`. Lines have missing handles; cubic segments use the outgoing control of the first anchor and incoming control of the next. An editable SVG string is not stored. The world-coordinate controls are finite and within ±100,000; bounding dimensions remain 1–100,000. Recomputed bounds use cubic extrema, minimum one unit for zero spans. Normalization preserves world coordinates, including horizontal/vertical lines.

Only paths may contain anchors. All records deny unknown fields. Lower-version files carrying `anchors` reject even if empty/null. Current writers emit v4 only with nonempty anchors; straight-only corner paths may retain v3 points. Opening old files does not rewrite them; conversion happens only when editing needs handles/modes. Saves retain the established explicit upgrade consent/Save As path, atomic destination replacement and headless `allow_upgrade` rules.

Rendering/PNG/picking use adaptive cubic subdivision with at most 0.05 world-unit deviation from the control chord and at most 32,768 flattened vertices per path. This is at most 0.4 points at supported PNG scale 8; it is geometric, not antialias/pixel accuracy. Closed fills use even-odd tessellation and matching fill picking, including concavity and single-contour self-intersections. Tessellation output is bounded. Invalid/budget-exceeding edits reject atomically, not silently flatten or truncate source. SVG and inline HTML SVG use genuine cubic `C` commands and explicit `fill-rule="evenodd"`; open contours have no fill. General SVG curve import and multiple subpaths remain unsupported and reject explicitly.

New GUI Pen paths use a visible stroke and transparent fill. v4 paths honor an explicit missing/zero stroke; released v3 path fallback strokes remain supported. Token references and resolved colors use the existing shared resolver.

## Editing and rollback

Drawing drafts and point/handle drag previews are transient, not serialized. Finish is one Editor transaction. Completed point drags/topology changes are individual transactions; cancellation does not alter document/history/revisions/redo. Anchor splitting uses de Casteljau and preserves the outline. Corner handles are independent, smooth handles collinear with independent lengths, symmetric handles mirrored. Point-mode controls accompany keyboard modifiers.

Rollback to an older application: keep the original v1–v3 file via Save As, or explicitly remove handle/mode features to obtain a straight v3 contour. There is no automatic lossy downgrade. SVG/PNG are derived output, not a substitute for editable source. Older applications reject v4 without overwriting it. No release tag is created by this local update.
