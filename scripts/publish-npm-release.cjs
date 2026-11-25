'use strict';

const assert = require('node:assert/strict');
const { createHash } = require('node:crypto');
const { execFileSync, spawnSync } = require('node:child_process');
const fs = require('node:fs');
const path = require('node:path');
const cli = require('../packages/cli/package.json');
const sdk = require('../packages/node/package.json');
const { currentTarget, targets } = require('../packages/cli/lib/platform.cjs');

const [directory, approval, ...extra] = process.argv.slice(2);
assert.ok(directory && extra.length === 0 && (!approval || approval === `v${cli.version}`),
  'Usage: node scripts/publish-npm-release.cjs <reviewed artifact directory> [approved Git tag]');
assert.equal(sdk.version, cli.version);
const registry = '--registry=https://registry.npmjs.org/';
const version = cli.version;
const platforms = Object.values(targets).map(({ os, cpu }) => currentTarget(os, cpu, '2.35'));
const license = fs.readFileSync(path.join(__dirname, '../LICENSE'), 'utf8').replaceAll('\r\n', '\n');
const names = [...platforms.map(target => target.name), sdk.name, cli.name];

const candidates = names.map(name => {
  const filename = `${name.replace(/^@/, '').replace('/', '-')}-${version}.tgz`;
  const archive = path.resolve(directory, filename);
  const read = file => execFileSync('tar', ['-xOf', archive, `package/${file}`], {
    maxBuffer: 128 * 1024 * 1024,
  });
  const integrity = 'sha512-' + createHash('sha512').update(fs.readFileSync(archive)).digest('base64');
  const manifest = JSON.parse(read('package.json'));
  assert.equal(manifest.name, name);
  assert.equal(manifest.version, version);
  assert.deepEqual(manifest.repository, cli.repository);
  assert.equal(manifest.license, cli.license);
  assert.equal(manifest.private, undefined);
  assert.equal(manifest.publishConfig, undefined);
  assert.ok(read('LICENSE').toString('utf8').replaceAll('\r\n', '\n') === license,
    `License text differs for ${name}`);
  assert.equal(manifest.scripts, undefined);
  const target = platforms.find(platform => platform.name === name);
  if (target) {
    assert.deepEqual(manifest.os, [target.os]);
    assert.deepEqual(manifest.cpu, [target.cpu]);
    assert.deepEqual(manifest.libc, target.libc ? [target.libc] : undefined);
    assert.equal(manifest.xrayBinary.file, target.binary);
    assert.equal(createHash('sha256').update(read(`bin/${target.binary}`)).digest('hex'),
      manifest.xrayBinary.sha256);
  }
  if (name === cli.name) {
    assert.deepEqual(manifest.optionalDependencies,
      Object.fromEntries(platforms.map(platform => [platform.name, version])));
  }
  return { spec: `${name}@${version}`, archive, integrity };
});

function alreadyPublished(candidate) {
  const result = spawnSync('npm', ['view', candidate.spec, 'dist.integrity', '--json', registry], {
    encoding: 'utf8', timeout: 60_000,
  });
  if (result.status !== 0) {
    if (JSON.parse(result.stdout || '{}').error?.code === 'E404') return false;
    throw new Error(result.error?.message ?? result.stderr);
  }
  if (!result.stdout.trim()) return false;
  assert.equal(JSON.parse(result.stdout), candidate.integrity, `Registry integrity differs for ${candidate.spec}`);
  return true;
}

if (approval) {
  const git = (...args) => execFileSync('git', args, { encoding: 'utf8' }).trim();
  assert.equal(git('rev-parse', `refs/tags/${approval}^{commit}`), git('rev-parse', 'HEAD'));
  assert.equal(git('status', '--porcelain'), '', 'Publish from a clean tagged checkout');
}
const plan = candidates.map(candidate => ({ candidate, exists: alreadyPublished(candidate) }));
for (const { candidate, exists } of plan) {
  console.log(`${exists ? 'Already published' : 'Pending'} ${candidate.spec}`);
  if (!approval || exists) continue;
  execFileSync('npm', ['publish', candidate.archive, '--access=public', '--provenance',
    '--ignore-scripts', registry], { stdio: 'inherit' });
  assert.ok(alreadyPublished(candidate), `Registry has not confirmed ${candidate.spec}; rerun using the same tarball`);
}
