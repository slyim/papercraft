import { createHash } from 'node:crypto';
import { closeSync, mkdirSync, mkdtempSync, openSync, readFileSync, realpathSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { parseArgs } from 'node:util';
import { listPackage } from '@electron/asar';

const root = fileURLToPath(new URL('../', import.meta.url));
const { values, positionals } = parseArgs({
  options: { 'record-integrity-mismatch': { type: 'boolean', default: false } },
  allowPositionals: true
});
if (positionals.length > 1) throw new Error('Expected at most one Paper application or ASAR path.');
const target = resolve(positionals[0] ?? '/Applications/Paper.app');
const integrityPolicy = values['record-integrity-mismatch'] ? 'record-and-continue' : 'fail';
const artifact = realpathSync(target.endsWith('.app') ? join(target, 'Contents/Resources/app.asar') : target);
const sourcePaths = listPackage(artifact)
  .map(path => path.replace(/^\//, ''))
  .filter(path => path === 'package.json' || (
    /^src\/.*\.(?:[cm]?js|tsx?)$/.test(path) && !/(?:^|\/)(?:__tests__|tests)\/|\.(?:spec|test)\./.test(path)
  ))
  .sort();

if (!sourcePaths.includes('package.json') || sourcePaths.length < 2) {
  throw new Error('Expected a Paper ASAR containing package.json and packaged src files.');
}

// shortcut: packaged desktop source only; add renderer capture when investigating editor behavior.
const outputRoot = join(root, 'research/private');
mkdirSync(outputRoot, { recursive: true, mode: 0o700 });
const output = mkdtempSync(join(outputRoot, 'paper-'));
const evidencePath = join(output, 'evidence.json');
const manifest = {
  created_at: new Date().toISOString(),
  artifact,
  sha256: createHash('sha256').update(readFileSync(artifact)).digest('hex'),
  rea_version: JSON.parse(readFileSync(join(root, 'node_modules/rea-agents/package.json'), 'utf8')).version,
  integrity_policy: integrityPolicy,
  source_paths: sourcePaths,
  scope: 'Static packaged desktop source; excludes dependencies, tests, .env files and hosted editor code.'
};
writeFileSync(join(output, 'manifest.json'), JSON.stringify(manifest, null, 2) + '\n', { mode: 0o600 });
console.log(`Analyzing ${sourcePaths.length} selected files. Evidence: ${evidencePath}`);

const fd = openSync(evidencePath, 'wx', 0o600);
let result;
try {
  result = spawnSync(process.execPath, [
    join(root, 'node_modules/rea-agents/scripts/rea.mjs'),
    'analyze-javascript-application', artifact,
    ...sourcePaths.flatMap(path => ['--source-path', path]),
    '--integrity-policy', integrityPolicy,
    '--format', 'json'
  ], { cwd: root, stdio: ['ignore', fd, 'inherit'], timeout: 600_000 });
} finally {
  closeSync(fd);
}
if (result.error || result.status !== 0) {
  throw new Error(`REA failed; diagnostics retained in ${dirname(evidencePath)}`, { cause: result.error });
}
const evidence = JSON.parse(readFileSync(evidencePath, 'utf8'));
if (!evidence.evidence_id) throw new Error(`REA returned no Evidence ID: ${evidencePath}`);
console.log(`Completed: ${evidence.evidence_id}`);
