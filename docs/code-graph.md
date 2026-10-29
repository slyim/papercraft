# Local code graph

Graphify is development tooling, not an application dependency or a second source of truth. Query it before broad cross-file searches, then read the cited Rust and relevant tests. Documentation can describe planned rather than implemented behavior.

## Setup and rebuild

Run from the repository root. The local setup was verified with `graphifyy 0.9.74` installed through uv:

```sh
uv tool install 'graphifyy==0.9.74'
graphify extract . --code-only --cargo
graphify cluster-only . --no-label
```

`--code-only` performs local structural extraction without an API key or semantic-model calls. `--no-label` prevents model-backed community naming. A deterministic rebuild can replace the initial curated community names with `Community N`; names are navigation aids, not stable identifiers. `cluster-only` generates the HTML and report as well as clustering. Rerun both commands after source changes; Graphify's manifest/cache handles unchanged code. Do not pass `--force` to hide an extraction failure or an unexpected graph shrink.

Outputs are local and Git-ignored under `graphify-out/`:

- `graph.json`: persistent, queryable graph.
- `graph.html`: interactive visualization; `open graphify-out/graph.html` on macOS.
- `GRAPH_REPORT.md`: hubs, communities, cohesion, inferred links and extraction cost.
- `manifest.json`: indexed-file hashes used for refreshes.

No background watcher, Git hook, hosted upload, global graph merge or MCP-server configuration is installed. This keeps rebuilding explicit and avoids altering other agents' configuration.

## Queries

```sh
graphify query 'pen path points creates' --budget 2000
graphify query 'example canvas drag nudge' --budget 2000
graphify explain 'Document'
graphify explain 'Editor'
graphify diagnose multigraph
graphify benchmark
```

Choose exact labels or vocabulary present in `graph.json`; there is no synonym/stemming guarantee. Follow `source_file` / `source_location`, not a generic similarly named `Frame` or `run()` in another file. A truncated query is incomplete; narrow it or increase its budget. Missing paths are not proof of missing application behavior. This graph is undirected: an arrow in a rendered report does **not** establish call direction.

## Coverage, privacy and limitations

`.graphifyignore` excludes research (including all private evidence), vendored skills, generated output, assets/fonts, license payloads and the npm lockfile. Normal `.gitignore` exclusions also apply. Keep these exclusions when changing extraction scope; do not send proprietary artifacts or credentials to a semantic backend.

The initial build indexed **37 code/tooling files**, producing **1,147 nodes, 2,770 edges and 46 named communities**. The initial corpus scan also detected 22 documents; **their semantics were intentionally not extracted**. Rust source, integration tests, verification examples and packaging/research tooling code are represented. Cargo introspection emitted one package node and no dependency edges; this is not a complete resolved-dependency graph. Graphify's sensitive-file filter skipped `tests/fixtures/tokens-v2.papercraft`; the fixture remains intact and covered by Rust tests.

The initial graph reflects the **dirty working tree on `97e833e`**, including pending selection/canvas slice 1b, not that commit alone. HEAD equality does not prove freshness. Refresh after editing, pulling or switching branches, and inspect indexed-file hashes when provenance matters.

**Graph health warning:** a separate raw AST diagnostic found 110 dangling-endpoint edges before placeholder resolution, 17 external-reference edges and 85 undirected same-endpoint collapses (including 32 exact duplicate edges). The built graph has no dangling endpoints or self-loops, but placeholder resolution and deduplication do not prove complete Rust symbol resolution. Initial raw diagnostics are retained locally in `graphify-out/HEALTH.txt`. Inferred calls and report arrows must be confirmed in source; do not treat the graph as a compiler call graph or runtime evidence.

Extraction used **0 external-LLM input/output tokens**. This excludes the host-agent planning session. `graphify benchmark` reported an estimated 18.3× reduction against its generic full-corpus-query baseline; that is a tool estimate, not a measured saving for this task.

The original implementation handoff is [Pen and editable workspace](pen-workspace-handoff.md); completion proof is in [Pen/workspace verification](pen-workspace-verification.md).

After the authored-Pen/Frame implementation, code-only extraction and clustering were refreshed: **42 code/tooling files, 1,288 nodes, 3,063 edges, 66 communities**, with no external model calls. The added vector/Pen modules and regressions are indexed. Communities and inferred call edges remain navigation aids, not behavioral proof; the earlier raw-health numbers above describe the initial extraction only.
