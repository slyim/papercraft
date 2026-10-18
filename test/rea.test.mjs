import assert from 'node:assert/strict';
import { mkdtemp, mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { spawnSync } from 'node:child_process';
import { test } from 'node:test';
import { fileURLToPath } from 'node:url';
import { createPackageWithOptions } from '@electron/asar';
import { Client } from '@modelcontextprotocol/client';
import { StdioClientTransport } from '@modelcontextprotocol/client/stdio';

const root = fileURLToPath(new URL('../', import.meta.url));

test('REA research tooling connects and the analysis command preserves scoped evidence', { timeout: 120_000 }, async () => {
  const client = new Client({ name: 'papercraft-test', version: '0.1.1' });
  const fixture = await mkdtemp(join(tmpdir(), 'papercraft-rea-'));
  const outputs = [];
  try {
    await client.connect(new StdioClientTransport({ command: process.execPath, args: [join(root, 'node_modules/rea-agents/scripts/rea.mjs'), 'mcp'], cwd: root }));
    assert.equal(client.getServerVersion().version, '6.7.0');
    const names = (await client.listTools()).tools.map(tool => tool.name);
    for (const name of ['analyze_javascript_application', 'inspect_analysis_view', 'trace_application_feature']) {
      assert.ok(names.includes(name), `Missing REA tool: ${name}`);
    }
    const source = join(fixture, 'source');
    await mkdir(join(source, 'src'), { recursive: true });
    await writeFile(join(source, 'package.json'), JSON.stringify({ name: 'rea-fixture', version: '1.0.0', main: 'src/main.ts' }));
    await writeFile(join(source, 'src/main.ts'), "import { ipcMain } from 'electron';\nipcMain.handle('example:ping', () => 'pong');\n");
    await writeFile(join(source, 'src/preload.ts'), "import { contextBridge, ipcRenderer } from 'electron';\ncontextBridge.exposeInMainWorld('example', { ping: () => ipcRenderer.invoke('example:ping') });\n");
    await writeFile(join(source, '.env.production'), 'UNSELECTED_ENV_SENTINEL=must-not-appear-in-evidence');
    await mkdir(join(source, 'native'));
    await writeFile(join(source, 'native/helper'), 'original helper bytes');
    const asar = join(fixture, 'fixture.asar');
    await createPackageWithOptions(source, asar, { unpack: '**/native/helper' });
    await writeFile(join(asar + '.unpacked', 'native/helper'), 'changed helper bytes');
    const rejected = spawnSync(process.execPath, ['scripts/analyze-paper.mjs', asar], { cwd: root, encoding: 'utf8', timeout: 30_000 });
    const rejectedOutput = rejected.stdout.match(/Evidence: (.+)\/evidence\.json/)?.[1];
    if (rejectedOutput) outputs.push(rejectedOutput);
    assert.equal(rejected.status, 1, 'An integrity mismatch must fail by default');
    const failure = JSON.parse(await readFile(join(rejectedOutput, 'evidence.json'), 'utf8'));
    assert.equal(failure.code, 'artifact_integrity_mismatch');
    const run = spawnSync(process.execPath, ['scripts/analyze-paper.mjs', asar, '--record-integrity-mismatch'], { cwd: root, encoding: 'utf8', timeout: 90_000 });
    const output = run.stdout.match(/Evidence: (.+)\/evidence\.json/)?.[1];
    if (output) outputs.push(output);
    assert.equal(run.status, 0, run.stderr + run.stdout);
    assert.ok(output);
    const raw = await readFile(join(output, 'evidence.json'), 'utf8');
    const evidence = JSON.parse(raw);
    const manifest = JSON.parse(await readFile(join(output, 'manifest.json'), 'utf8'));
    assert.match(evidence.evidence_id, /^ev_[a-f0-9]{64}$/);
    assert.match(manifest.sha256, /^[a-f0-9]{64}$/);
    assert.equal(manifest.integrity_policy, 'record-and-continue');
    assert.equal(evidence.normalized_result.integrity_contradictions[0].trust, 'observed-untrusted');
    assert.deepEqual(manifest.source_paths, ['package.json', 'src/main.ts', 'src/preload.ts']);
    assert.ok(raw.includes('example:ping'), 'Static IPC observation missing');
    assert.ok(!raw.includes('must-not-appear-in-evidence'), 'Unselected environment content was read');
  } finally {
    await client.close();
    await rm(fixture, { recursive: true, force: true });
    for (const output of outputs) await rm(output, { recursive: true, force: true });
  }
});
