# Third-party notices

The native UI uses Phosphor Icons through `egui-phosphor` 0.14.0. Preserve both notices when distributing the icon font or the integration:

- [Phosphor Icons MIT license](licenses/phosphor-icons-MIT.txt), from the [official icon repository](https://github.com/phosphor-icons/core/blob/main/LICENSE).
- [egui-phosphor MIT license](licenses/egui-phosphor-MIT.txt), copied from the installed crate.

REA is a development tool. Its copied workflow retains the upstream license in `.agents/skills/reverse-engineer-anything/LICENSE`.

Other dependencies retain their own licenses in their packages. `Cargo.lock` and `package-lock.json` identify the versions used; this file is not a complete distribution-license inventory. No Paper assets or source are included in application code.

The macOS prerelease includes dependency notices, package metadata, upstream notice references, Rust standard-library notices, and font/icon licenses in `Crumbs.app/Contents/Resources/licenses/`. These materials retain their own terms. Original Crumbs code and documentation are separately offered under MIT or Apache-2.0 in the repository's root license files.
