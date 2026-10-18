# Third-party notices

The native UI uses Phosphor Icons through `egui-phosphor` 0.14.0. Preserve both notices when distributing the icon font or the integration:

- [Phosphor Icons MIT license](licenses/phosphor-icons-MIT.txt), from the [official icon repository](https://github.com/phosphor-icons/core/blob/main/LICENSE).
- [egui-phosphor MIT license](licenses/egui-phosphor-MIT.txt), copied from the installed crate.

REA is a development tool. Its copied workflow retains the upstream license in `.agents/skills/reverse-engineer-anything/LICENSE`.

macOS Quit handling uses `craft-quit` 0.1.0 from [craft-libs revision 82a24b0](https://github.com/storytold/craft-libs/tree/82a24b006eefa036b43c09a67fb9a3fbd17ffe63/crates/quit), under its MIT option. The [copyright and MIT notice](licenses/craft-libs-MIT.txt) are retained. Its native menu dependency `muda` and other transitive dependencies retain their package licenses and locked versions. No Craft application branding or assets are included.

Other dependencies retain their own licenses in their packages. `Cargo.lock` and `package-lock.json` identify the versions used; this file is not a complete distribution-license inventory. No Paper assets or source are included in application code.

The macOS prerelease includes dependency notices, package metadata, upstream notice references, Rust standard-library notices, and font/icon licenses in `PaperCraft.app/Contents/Resources/licenses/`. These materials retain their own terms. Original PaperCraft code, documentation and the generated logo are separately offered under the root MIT license. The earlier Crumbs v0.1.0 distribution retains its published MIT-or-Apache-2.0 grant.
