# PaperCraft document format

Written 18 October 2026, before the color-token persistence change was implemented. This page records compatibility, downgrade limits and rollback for each format version.

## Version 1 (unchanged)

JSON with `version: 1`, artboard fields, optional `canvas` (page color), `next_id` and ordered `nodes`. Readers before this change parse `Document` and `Node` with `deny_unknown_fields` and accept only `version == 1`. Any unknown field makes them **reject the whole file**. They never drop data silently, and they never overwrite the file they rejected.

## Version 2: color tokens

Version 2 adds these fields. A version 1 document has none of them.

| Field | Meaning |
|---|---|
| `tokens: [{id, name, value}]` | Named color tokens. `id` is stable and unique. `name` is unique when compared case-insensitively, at most 64 bytes, and uses Unicode letters/digits, `-` and `_`. `value` is sRGB RGBA. |
| `next_token_id` | The next token identifier. It must be greater than every token `id`. |
| `background_token`, `canvas_token` | Optional binding of the artboard color and the page color. |
| `nodes[].fill_token` | Optional binding of a rectangle fill or a text color. |

**Literal values stay in the file.** Every bound property also keeps its literal `fill`/`background`/`canvas`, synchronized to the token value on each committed edit and on load. The token wins if the two differ. This means that any field-stripping downgrade, a detach and an export all have the correct resolved color.

**Writer rule.** PaperCraft writes the lowest version that can represent the document. A document with no tokens is written as version 1, without `tokens`, `next_token_id` or binding keys, so its bytes keep the version 1 shape and older PaperCraft can open it. A document with at least one token is written as version 2.

**Reader rule.** Versions 1 and 2 are accepted. Version 1 containing token fields is rejected as malformed. Versions above 2 are rejected with "created by a newer PaperCraft". Unknown fields are still rejected. Bindings to missing tokens are rejected on load and on every commit, so an invalid reference can never be saved.

## Migration and preservation of originals

- Opening a version 1 `.papercraft` or legacy `.crumbs` file never rewrites it.
- The first **Save** that would turn a file opened as version 1 into version 2 asks first. **Save as new file…** (the default) leaves the original bytes intact. **Upgrade this file** replaces it atomically. **Cancel** writes nothing. Save As, and saving a document that was never saved, have no original to protect and write directly.
- Recent-file history is unchanged: same storage location, same path list, no format field.

## Downgrade limits

- The published PaperCraft v0.1.1 build and this branch's baseline (`bc3c52b`) cannot open version 2 files. They reject them without modifying them. Source builds from the token increment support both versions; the package version alone does not distinguish these builds.
- Token names and bindings exist only in version 2. HTML export keeps them as CSS custom properties (`--color-<name>`). SVG export contains only resolved colors.

## Rollback and export strategy

1. **In the app:** delete every token in the Theme tab. Deleting a token detaches its bindings and keeps their current colors as literals, in one undo step. The next save writes version 1, which older versions open.
2. **Without the app:** remove `tokens`, `next_token_id`, `background_token`, `canvas_token` and every `fill_token` key, then set `version` to 1. Because literals are synchronized, the result has the same appearance.
3. **Derived output:** HTML and SVG export resolve the same colors and are independent of the source version.

Rolling back the application binary requires no data migration for version 1 files. Version 2 files need step 1 or 2 first.
