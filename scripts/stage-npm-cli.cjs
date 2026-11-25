'use strict';

const assert = require('node:assert/strict');
const { spawnSync } = require('node:child_process');
const { createHash } = require('node:crypto');
const fs = require('node:fs');
const path = require('node:path');
const { currentTarget, targets } = require('../packages/cli/lib/platform.cjs');
const cli = require('../packages/cli/package.json');

const root = path.resolve(__dirname, '..');
const [binaryPath, output, expectedTarget] = process.argv.slice(2);
assert.ok(binaryPath && output, 'Usage: node scripts/stage-npm-cli.cjs <native binary> <new output directory>');
const binary = path.resolve(binaryPath);
const out = path.resolve(output);
const target = currentTarget();
assert.ok(target, 'Staging requires a supported native runner');
if (expectedTarget) assert.equal(target.key, expectedTarget);
const version = spawnSync(binary, ['--version'], { encoding: 'utf8', timeout: 10000 });
assert.equal(version.status, 0, version.error?.message ?? version.stderr);
assert.equal(version.stdout.trim(), `request-xray ${cli.version}`);

fs.mkdirSync(out);
const platformDir = path.join(out, 'platform');
const cliDir = path.join(out, 'cli');
const binDir = path.join(platformDir, 'bin');
fs.mkdirSync(binDir, { recursive: true });
fs.cpSync(path.join(root, 'packages/cli'), cliDir, {
  recursive: true,
  filter: source => !['node_modules', 'dist'].includes(path.basename(source)),
});
const { private: _private, ...launcher } = cli;
const optionalDependencies = Object.fromEntries(Object.keys(targets).map(key => [`@request-xray/cli-${key}`, cli.version]));
fs.writeFileSync(path.join(cliDir, 'package.json'), JSON.stringify({ ...launcher, optionalDependencies }, null, 2) + '\n');
const packaged = path.join(binDir, target.binary);
fs.copyFileSync(binary, packaged);
if (target.os !== 'win32') {
  fs.chmodSync(packaged, 0o755);
  fs.chmodSync(path.join(cliDir, 'bin/request-xray.cjs'), 0o755);
}
const sha256 = createHash('sha256').update(fs.readFileSync(packaged)).digest('hex');
fs.writeFileSync(path.join(platformDir, 'package.json'), JSON.stringify({
  name: target.name, version: cli.version, license: cli.license, repository: cli.repository,
  os: [target.os], cpu: [target.cpu], ...(target.libc && { libc: [target.libc] }),
  exports: { './package.json': './package.json' },
  files: ['bin'],
  xrayBinary: { file: target.binary, sha256 },
}, null, 2) + '\n');
for (const folder of [platformDir, cliDir]) fs.copyFileSync(path.join(root, 'LICENSE'), path.join(folder, 'LICENSE'));
console.log(`Staged ${target.name}@${cli.version} and request-xray@${cli.version} at ${out}`);
