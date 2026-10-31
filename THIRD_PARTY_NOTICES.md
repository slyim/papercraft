# Third-party notices

The native UI uses Phosphor Icons through `egui-phosphor` 0.14.0. Preserve both notices when distributing the icon font or the integration:

- [Phosphor Icons MIT license](licenses/phosphor-icons-MIT.txt), from the [official icon repository](https://github.com/phosphor-icons/core/blob/main/LICENSE).
- [egui-phosphor MIT license](licenses/egui-phosphor-MIT.txt), copied from the installed crate.

REA is a development tool. Its copied workflow retains the upstream license in `.agents/skills/reverse-engineer-anything/LICENSE`.

macOS Quit handling uses `craft-quit` 0.1.0 from [craft-libs revision 82a24b0](https://github.com/storytold/craft-libs/tree/82a24b006eefa036b43c09a67fb9a3fbd17ffe63/crates/quit), under its MIT option. The [copyright and MIT notice](licenses/craft-libs-MIT.txt) are retained. Its native menu dependency `muda` and other transitive dependencies retain their package licenses and locked versions. No Craft application branding or assets are included.

The objc2 family includes upstream licensing references and the retained [Steven Sheldon MIT notice](licenses/steven-sheldon-MIT.txt), from [upstream revision 4fc083f](https://github.com/madsmtm/objc2/blob/4fc083f1c6d6784577e38b0ee8dbd344481e2fd2/LICENSE.txt). The same copyright/license text accompanies the MIT-licensed `dispatch` 0.2.0 crate, whose package declares MIT and credits Sheldon but omits a license file. Each archive records the exact dependency versions and upstream notice sources.

Bounded even-odd path fill tessellation uses `lyon_tessellation` (locked version 1.0.23, MIT OR Apache-2.0) and its Cargo-resolved dependencies. Their original notices are collected into packaged `licenses/crates/`; PaperCraft's vector model, editing and cubic geometry remain original Rust.

Figma import adapts FigCraft's generic Kiwi decoder from [revision e04c5d2](https://github.com/storytold/figcraft/tree/e04c5d2274329587b9213f27ad8f88708ea77748) under MIT. The [copyright and license notice](licenses/figcraft-MIT.txt) are retained and included in packaged project notices. PaperCraft supplies its own bounded archive reader and document mapping.

Other dependencies retain their own licenses in their packages. `Cargo.lock` and `package-lock.json` identify the versions used; this file is not a complete distribution-license inventory. No Paper assets or source are included in application code.

Release archives include dependency notices, package metadata, pinned upstream notice references, Rust standard-library notices, and font/icon licenses in `licenses/` (inside `PaperCraft.app/Contents/Resources/` on macOS). These materials retain their own terms. Original PaperCraft code, documentation and the generated logo are separately offered under the root MIT license. The earlier Crumbs v0.1.0 distribution retains its published MIT-or-Apache-2.0 grant.
