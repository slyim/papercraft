# PaperCraft logo

The engraved origami-crane mark is original project artwork generated with the built-in image generation tool. It uses the broad visual convention of an engraved subject on a colored tile; no ArtCraft, Paper or other upstream logo file is included in application assets. The logo is covered by the project's MIT license.

- `papercraft-logo.png`: full-resolution generated logo.
- `papercraft-icon.png`: 256-pixel runtime icon for the window and dashboard.
- `papercraft-macos.png`: 512-pixel rounded-tile runtime icon for the macOS Dock.
- `papercraft.icns`: macOS application icon, containing the standard 16–1024 pixel sizes.

macOS does not mask application icons, so the macOS files carry their own rounded tile (824 pixels on a 1024-pixel canvas) and transparent margin. Regenerate both with `python3 packaging/macos/make_icon.py`.

## Final generation prompt

Create a finished desktop app icon for PaperCraft. One single cream-and-black engraved origami paper crane on a completely opaque flat solid amber #D4A33E square background. No transparency, no alpha cutout, no gradients or background textures. Bird has a long angular folded neck, triangular head facing left, and two elegant high wings made of folded paper sheets, drawn with precise vintage black ink etching and delicate crosshatching on ivory paper. Strong beautiful silhouette, centered, all parts inside an even 8 percent margin, readable small. The engraved paper bird is the entire logo: no text, letters, typography, border, additional graphics, shadows, glow, mockup or photorealism. Background must be a single uniform warm amber color from edge to edge, with no dark patches, no burn marks, no holes, and no grain. Keep the thin fine lines inside the ivory-and-black crane only. This is an original illustration for an independent creative app, not a reproduction of another app icon. Deliver one polished square PNG, 1024 by 1024.
