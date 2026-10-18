# PaperCraft desktop editor

Mode: Operate. The user's explicit Paper reference fixes the surface structure: a central canvas, left layer list, right inspector, and a compact top toolbar. No new visual direction is being introduced.

- The canvas occupies the remaining window area; controls stay secondary to the document.
- Use a dark neutral application shell, a lighter canvas surround, and a light artboard. Use one blue accent for selection and focus.
- Use egui's bundled readable UI font and Phosphor regular icons with text or tooltips. Icons supplement labels.
- Panels use precise spacing, thin separators and clear section labels. Avoid decorative cards or promotional content inside the editor.
- Show the current file, dirty state, zoom and action status. Errors include the failed action and a recovery path.
- Editing must remain possible through the layer list and inspector, not only pointer gestures on the canvas.
- Support normal document shortcuts and native file dialogs. Confirm unsaved work before replacing a document or closing the app.
- Export opens HTML/SVG choices, with the SVG text-line/font limitation stated beside the choices. Export uses current document state and does not mark the source saved.
- Reference geometry is an original single-artboard editor. It does not assert Paper layout or export fidelity.
