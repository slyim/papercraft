# PaperCraft desktop editor

Mode: Operate. The user's explicit Paper reference fixes the surface structure: a central canvas, left layer list, right inspector, and a compact top toolbar. No new visual direction is being introduced.

- The canvas occupies the remaining window area; controls stay secondary to the document.
- Use a dark neutral application shell, a lighter canvas surround, and a light artboard. Use one blue accent for selection and focus.
- Use egui's bundled readable UI font and Phosphor regular icons with text or tooltips. Icons supplement labels.
- Panels use precise spacing, thin separators and clear section labels. Avoid decorative cards or promotional content inside the editor.
- Show the current file, dirty state, zoom and action status. Errors include the failed action and a recovery path.
- Editing must remain possible through the layer list and inspector, not only pointer gestures on the canvas.
- The left panel's Design/Theme switch selects layers or color tokens. Bound color properties show the token swatch and name with a detach control; every Arrange and token control acts on implemented behavior, and disabled controls state their requirement.
- Support normal document shortcuts and native file dialogs. Confirm unsaved work before replacing a document or closing the app.
- Export opens HTML/SVG choices, with the SVG text-line/font limitation stated beside the choices. Export uses current document state and does not mark the source saved.
- Reference geometry is an original single-artboard editor. It does not assert Paper layout or export fidelity.

## Layout scale

All shell geometry uses the 4-point scale defined in `src/ui.rs`. Verify rendered control bounds and native screenshots; source-literal scans cannot establish layout correctness.

- Spacing steps: 4, 8, 12, 16, 24, 32, 64. Hairlines are 1 pt.
- Type: 11 pt captions and metadata, 12 pt body, 20 pt page headings. Icons: 12 pt in rows, tabs and fields; 16 pt on standalone buttons and the tool rail.
- Rows, fields, tabs and buttons are 24 pt high; tool rail buttons 32 pt. Radii: 4 pt for controls, 8 pt for cards, menus and windows.
- Tab strip 28 pt; dashboard sidebar and layer panel 208 pt; inspector 240 pt; tool rail 40 pt. Panel headers are 40 pt.
- Row content: background inset 8 pt, icon at 16 pt, label at 36 pt.
- Dashboard: 64 pt side padding (32 pt below 640 pt), 32 pt title row 32 pt below the tab strip, 32 pt gap to the cards, 24 pt card gaps, 48 pt card header, 4 pt preview inset.

These values were measured from a user-supplied Paper dashboard screenshot (18 October 2026) and rounded to the grid; see [verification](docs/verification.md#compact-layout-scale-18-october-2026). Paper fonts, branding and assets are not used. Tabs show no unsaved-changes dot; unsaved state is in the tab tooltip and window title. A document tab's close control appears while the tab is hovered.
