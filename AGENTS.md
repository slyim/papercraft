# PaperCraft

- Current stage: implement the first native PaperCraft editor from the recorded Paper research, then expand feature by feature using `docs/implementation-process.md`.
- All application code must be Rust. Use egui/eframe for the native interface and `egui-phosphor` for Phosphor Icons. Existing JavaScript is research tooling only.
- REA 6.7.0 is installed as a development dependency. Run it with `npm run rea -- <command>`.
- Use the project REA skill at `.agents/skills/reverse-engineer-anything/SKILL.md` for shipped-artifact analysis.
- Inspect Paper at `/Applications/Paper.app`; keep raw evidence under the ignored `research/private/` directory.
- Document observations, inferences, and unknowns separately. Cite artifact hashes, Evidence IDs, and source paths.
- Keep implementation original. Use documented behavior and evidence to write the alternative; do not add Paper source, branding, fonts, or assets to application code.
- Keep research evidence separate from original application code. The user has selected Rust and egui and authorized the first implementation slice; broader Paper parity still requires feature-specific evidence and scope.
- Use the local Graphify code graph before broad cross-file exploration, then confirm results in source/tests. See `docs/code-graph.md` for privacy exclusions, coverage, health warnings and the key-free rebuild/query commands. Generated `graphify-out/` stays ignored; it is not behavior proof.
- The bounded editable-Frame/authored-Pen update is implemented; current proof and limitations are in `docs/pen-workspace-verification.md`, with the v4 contract in `docs/format-v4.md`. `docs/pen-workspace-handoff.md` retains the original plan/source findings. Main publication and local macOS installation require their separate exact-commit/checksum gates.
- Use concise, factual, impersonal communication.
