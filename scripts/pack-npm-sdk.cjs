'use strict';

const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { execFileSync } = require('node:child_process');
const { createHash } = require('node:crypto');

const root = path.resolve(__dirname, '..');
assert.ok(process.argv[2], 'Usage: node scripts/pack-npm-sdk.cjs <existing output directory>');
const out = path.resolve(process.argv[2]);
const source = path.join(root, 'packages/node');
const stage = path.join(out, 'sdk');
const manifest = require(path.join(source, 'package.json'));
const cli = require('../packages/cli/package.json');
assert.equal(manifest.version, cli.version);
const files = ['dist', 'index.d.mts', 'index.mjs', 'register.cjs', 'register.mjs'];
fs.mkdirSync(stage);
for (const file of files) fs.cpSync(path.join(source, file), path.join(stage, file), { recursive: true });
fs.copyFileSync(path.join(root, 'LICENSE'), path.join(stage, 'LICENSE'));
fs.copyFileSync(path.join(root, 'docs/NODE_SDK.md'), path.join(stage, 'README.md'));
const { private: _private, scripts: _scripts, devDependencies: _dev, ...runtime } = manifest;
fs.writeFileSync(path.join(stage, 'package.json'), JSON.stringify({
  ...runtime, files, license: cli.license, repository: cli.repository,
}, null, 2) + '\n');

function pack(args) {
  return JSON.parse(execFileSync('npm', ['pack', '--ignore-scripts', '--json', ...args], {
    cwd: stage, encoding: 'utf8',
  }))[0];
}

const plan = pack(['--dry-run']);
const packed = pack(['--pack-destination', out]);
assert.deepEqual(packed.files, plan.files);
for (const file of ['LICENSE', 'README.md', 'dist/index.js', 'dist/index.d.ts', ...files.slice(1)]) {
  assert.ok(plan.files.some(entry => entry.path === file), `Missing SDK file ${file}`);
}
const bytes = fs.readFileSync(path.join(out, packed.filename));
const sha256 = createHash('sha256').update(bytes).digest('hex');
fs.writeFileSync(path.join(out, 'sdk.sha256'), `${sha256}  ${packed.filename}\n`);
fs.writeFileSync(path.join(out, 'sdk-pack.json'), JSON.stringify(plan, null, 2) + '\n');
console.log(`Packed ${manifest.name}@${manifest.version} as ${packed.filename}`);
