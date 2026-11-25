'use strict';

const assert = require('node:assert/strict');
const { spawn, spawnSync } = require('node:child_process');
const { createHash } = require('node:crypto');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { setTimeout: wait } = require('node:timers/promises');
const { currentTarget, targets } = require('../packages/cli/lib/platform.cjs');

const root = path.resolve(__dirname, '..');
const stage = path.resolve(process.argv[2] ?? '');
assert.ok(process.argv[2], 'Usage: node scripts/smoke-npm-cli.cjs <staging directory>');
const target = currentTarget();
assert.ok(target);
const project = fs.mkdtempSync(path.join(os.tmpdir(), 'xray-npm-smoke-'));
const nodeBin = path.dirname(process.execPath);
const isolatedBin = path.join(project, 'runtime-bin');
fs.mkdirSync(isolatedBin);
if (process.platform !== 'win32') fs.symlinkSync(process.execPath, path.join(isolatedBin, 'node'));
const runtimePath = process.platform === 'win32'
  ? [nodeBin, path.join(process.env.SystemRoot, 'System32')].join(path.delimiter)
  : isolatedBin;
const env = Object.fromEntries(Object.entries(process.env).filter(([key]) =>
  !/^(REQUEST_XRAY_|NODE_OPTIONS$|NODE_PATH$|PATH$|NPM_CONFIG_)/i.test(key)));
const childEnv = { ...env, PATH: runtimePath, npm_config_cache: path.join(project, 'npm-cache') };
if (process.platform !== 'win32') childEnv.npm_config_script_shell = '/bin/sh';
const npmEntry = [
  path.join(nodeBin, 'node_modules/npm/bin/npm-cli.js'),
  path.join(nodeBin, '../lib/node_modules/npm/bin/npm-cli.js'),
  path.join(nodeBin, '../share/nodejs/npm/bin/npm-cli.js'),
  path.join(nodeBin, 'npm'),
].find(file => fs.existsSync(file));
assert.ok(npmEntry, 'npm must be installed alongside Node');

function run(args, { cwd = project, env = childEnv, code = 0 } = {}) {
  const result = spawnSync(process.execPath, args, { cwd, env, encoding: 'utf8', timeout: 30_000 });
  assert.equal(result.status, code, result.error?.message ?? `${result.stdout}\n${result.stderr}`);
  return result;
}

function pack(name) {
  const args = [npmEntry, 'pack', '--ignore-scripts', '--json'];
  const options = { cwd: path.join(stage, name) };
  const [plan] = JSON.parse(run([...args, '--dry-run'], options).stdout);
  const [packed] = JSON.parse(run([...args, '--pack-destination', stage], options).stdout);
  assert.deepEqual(packed.files, plan.files);
  assert.ok(plan.files.some(file => file.path === 'LICENSE'));
  fs.writeFileSync(path.join(stage, `${name}-pack.json`), JSON.stringify(plan, null, 2) + '\n');
  return path.join(stage, packed.filename);
}

async function eventually(check) {
  const deadline = Date.now() + 10_000;
  while (Date.now() < deadline) {
    const result = await check();
    if (result) return result;
    await wait(50);
  }
  throw new Error('Timed out waiting for local process state');
}

function start(shim, command) {
  const child = spawn(process.execPath, [shim, command, '--no-open', '--collector-port', '0', '--query-port', '0'],
    { cwd: project, env: childEnv, stdio: ['ignore', 'pipe', 'pipe'] });
  let output = '';
  for (const stream of [child.stdout, child.stderr]) {
    stream.setEncoding('utf8');
    stream.on('data', chunk => { output += chunk; });
  }
  let status;
  let error;
  child.once('error', failure => { error = failure; });
  const done = new Promise(resolve => child.once('exit', (code, signal) => resolve(status = { code, signal })));
  return {
    async ready() {
      try {
        return await eventually(() => {
          if (error || status) throw new Error(String(error ?? status.code));
          const otlp = output.match(/OTLP\/HTTP listener at (http:\/\/127\.0\.0\.1:(\d+)\/v1\/traces)/);
          const view = output.match(/Local trace view: (http:\/\/127\.0\.0\.1:(\d+)\/)/);
          return otlp && view && { endpoint: otlp[1], port: otlp[2], view: view[1], queryPort: view[2] };
        });
      } catch (error) {
        throw new Error(`${error.message}\n${output}`);
      }
    },
    async stop() {
      if (status || error) return;
      child.kill('SIGTERM');
      try {
        await eventually(() => status);
        if (process.platform !== 'win32') assert.equal((await done).code, 0, output);
      } finally {
        if (!status) child.kill('SIGKILL');
      }
    },
  };
}

async function get(url) {
  const response = await fetch(url, { signal: AbortSignal.timeout(3000) });
  assert.equal(response.status, 200);
  return response;
}

async function main() {
  for (const [key, platform] of Object.entries(targets)) {
    assert.equal(currentTarget(platform.os, platform.cpu, '2.35').key, key);
  }
  assert.equal(currentTarget('linux', 'arm64', ''), undefined);
  assert.equal(currentTarget('linux', 'x64', '2.34'), undefined);
  assert.equal(currentTarget('win32', 'arm64'), undefined);
  assert.equal(spawnSync('cargo', ['--version'], { env: childEnv }).error?.code, 'ENOENT');
  const platformTarball = pack('platform');
  const cliTarball = pack('cli');
  fs.writeFileSync(path.join(project, 'package.json'), JSON.stringify({ name: 'fixture', private: true, type: 'module' }));
  run([npmEntry, 'install', '--offline', '--ignore-scripts', '--no-audit', '--no-fund', cliTarball, platformTarball]);
  const shim = path.join(project, 'node_modules/request-xray/bin/request-xray.cjs');
  const platformDir = path.join(project, 'node_modules', target.name);
  const installedBinary = path.join(platformDir, 'bin', target.binary);
  const native = (...args) => run([shim, ...args]);
  const checksums = [platformTarball, cliTarball].map(file =>
    `${createHash('sha256').update(fs.readFileSync(file)).digest('hex')}  ${path.basename(file)}`);
  fs.writeFileSync(path.join(stage, 'cli.sha256'), checksums.join('\n') + '\n');
  const version = require(path.join(stage, 'cli/package.json')).version;
  const launcherManifest = JSON.parse(fs.readFileSync(path.join(path.dirname(shim), '../package.json'), 'utf8'));
  const platformManifestPath = path.join(platformDir, 'package.json');
  const platformManifestText = fs.readFileSync(platformManifestPath, 'utf8');
  const platformManifest = JSON.parse(platformManifestText);
  assert.deepEqual(launcherManifest.bin, { 'request-xray': 'bin/request-xray.cjs' });
  assert.deepEqual(launcherManifest.optionalDependencies, Object.fromEntries(
    Object.keys(targets).map(key => [`@request-xray/cli-${key}`, version]),
  ));
  assert.deepEqual(platformManifest.os, [target.os]);
  assert.deepEqual(platformManifest.cpu, [target.cpu]);
  assert.deepEqual(platformManifest.libc, target.libc ? [target.libc] : undefined);
  for (const listenFlag of ['--collector-listen', '--query-listen']) {
    assert.match(run([shim, listenFlag, '0.0.0.0:0', 'config', 'validate'], { code: 2 }).stderr, /loopback/);
  }
  for (const manifest of [launcherManifest, platformManifest]) {
    assert.equal(manifest.private, undefined);
    for (const hook of ['preinstall', 'install', 'postinstall']) {
      assert.equal(manifest.scripts?.[hook], undefined);
    }
  }
  assert.match(native('--help').stdout, /Explore local OpenTelemetry traces/);
  assert.equal(native('version').stdout.trim(), `request-xray ${version}`);
  assert.equal(run([npmEntry, 'exec', '--offline', '--', 'request-xray', '--version']).stdout.trim(), `request-xray ${version}`);
  assert.match(native('init', '--dry-run').stdout, /instrumentation.mjs/);
  assert.ok(!fs.existsSync(path.join(project, 'request-xray.config.json')));
  native('init');
  native('init');
  assert.match(fs.readFileSync(path.join(project, 'request-xray.instrumentation.mjs'), 'utf8'), /@request-xray\/node\/register/);
  const config = path.join(project, 'request-xray.config.json');
  fs.writeFileSync(config, '{"version":1,"collectorListen":"127.0.0.1:4401","queryListen":"127.0.0.1:4402"}');
  assert.match(native('config', 'validate', '--file', config).stdout, /4401/);
  const precedence = run([shim, '--collector-port', '4404', 'config', 'validate'], {
    env: { ...childEnv, REQUEST_XRAY_OTLP_LISTEN: '127.0.0.1:4403' },
  });
  assert.match(precedence.stdout, /4404/);
  assert.match(run([shim, 'config', 'validate'], { env: { ...childEnv, REQUEST_XRAY_OTLP_LISTEN: '127.0.0.1:4403' } }).stdout, /4403/);
  fs.writeFileSync(config, '{"accessToken":"private-test-value"}');
  const invalid = run([shim, 'config', 'validate'], { code: 2 });
  assert.ok(!invalid.stderr.includes('private-test-value'));
  fs.writeFileSync(config, '{"version":1}');

  fs.renameSync(platformDir, path.join(project, 'displaced-platform'));
  try { assert.match(run([shim, '--version'], { code: 4 }).stderr, /Missing .*optional dependencies/); }
  finally { fs.renameSync(path.join(project, 'displaced-platform'), platformDir); }
  fs.writeFileSync(platformManifestPath, JSON.stringify({ ...platformManifest, version: '0.0.0' }));
  try { assert.match(run([shim, '--version'], { code: 4 }).stderr, /Invalid or damaged native binary/); }
  finally { fs.writeFileSync(platformManifestPath, platformManifestText); }
  const originalBinary = path.join(project, 'original-native');
  fs.renameSync(installedBinary, originalBinary);
  try {
    // Keep the original inode untouched so macOS code-signing caches still apply.
    fs.copyFileSync(originalBinary, installedBinary);
    fs.appendFileSync(installedBinary, Buffer.of(0));
    assert.match(run([shim, '--version'], { code: 4 }).stderr, /Invalid or damaged native binary/);
  } finally {
    fs.rmSync(installedBinary, { force: true });
    fs.renameSync(originalBinary, installedBinary);
  }

  const dev = start(shim, 'dev');
  let live;
  try {
    live = await dev.ready();
    const browser = await get(live.view);
    assert.match(browser.headers.get('content-security-policy'), /script-src 'self'/);
    const html = await browser.text();
    assert.match(html, /data-mode="live"/);
    const script = html.match(/<script[^>]* src="([^"]+)"/)?.[1];
    assert.ok(script);
    if (process.env.XRAY_REQUIRE_DASHBOARD === '1') assert.match(script, /^\/assets\//);
    assert.match((await get(new URL(script, live.view))).headers.get('content-type'), /javascript/);
    const flags = ['--collector-port', live.port, '--query-port', live.queryPort];
    const health = JSON.parse(native(...flags, 'doctor', '--json').stdout);
    assert.equal(health.queryReady, true);
    assert.equal(health.otlpReady, true);
    const collision = run([shim, '--collector-port', live.port, '--query-port', '0', 'serve'], { code: 3 });
    assert.match(collision.stderr, /address already in use/);
    const failedDb = path.join(project, 'must-not-be-created.sqlite');
    const queryCollision = run([shim, '--collector-port', '0', '--query-port', live.queryPort, '--db-path', failedDb, 'serve'], { code: 3 });
    assert.match(queryCollision.stderr, /query.*address already in use/);
    assert.ok(!fs.existsSync(failedDb));
    const payload = fs.readFileSync(path.join(root, 'tests/fixtures/otlp/distributed.pb'));
    assert.equal((await fetch(live.endpoint, {
      method: 'POST', headers: { 'content-type': 'application/x-protobuf' }, body: payload, signal: AbortSignal.timeout(3000),
    })).status, 200);
    const trace = JSON.parse(native(...flags, 'traces', '--json').stdout).items[0];
    assert.equal(trace.spanCount, 3);
    const inspect = JSON.parse(native(...flags, 'traces', 'show', trace.traceId, '--json').stdout);
    assert.equal(inspect.spans.items.length, 3);
    assert.match(native(...flags, 'inspect', trace.traceId, '--tree').stdout, /link /);
    assert.match(native(...flags, 'traces', 'list').stdout, new RegExp(trace.traceId));
  } finally {
    await dev.stop();
  }
  await eventually(async () => {
    try { await fetch(live.view, { signal: AbortSignal.timeout(1000) }); return false; } catch { return true; }
  });

  const demo = start(shim, 'demo');
  let demoView;
  try {
    const { view } = await demo.ready();
    demoView = view;
    assert.match(await (await get(view)).text(), /data-mode="demo"/);
    const stats = await (await get(new URL('api/v1/stats', view))).json();
    assert.equal(stats.spanCount, 3);
  } finally {
    await demo.stop();
  }
  await eventually(async () => {
    try { await fetch(demoView, { signal: AbortSignal.timeout(1000) }); return false; } catch { return true; }
  });
  console.log(`Verified installed CLI tarballs for ${target.key} without Rust on PATH`);
}

main().catch(error => {
  console.error(error);
  process.exitCode = 1;
}).finally(() => fs.rmSync(project, { recursive: true, force: true, maxRetries: 5 }));
