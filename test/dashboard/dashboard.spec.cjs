const { test, expect } = require('@playwright/test');
const { spawn, execFileSync } = require('node:child_process');
const fs = require('node:fs/promises');
const path = require('node:path');
const os = require('node:os');
const { currentTarget } = require('../../packages/cli/lib/platform.cjs');

const root = path.resolve(__dirname, '../..');
const stage = path.resolve(root, process.env.XRAY_NPM_STAGE ?? 'npm-cli-stage');
const fixtures = path.resolve(root, 'target/dashboard-playwright/fixtures');
const screenshots = path.resolve(root, 'target/dashboard-playwright/screenshots');
const example = path.resolve(root, 'examples/express-basic');
const knownTrace = '52000000000000000000000000000001';
const knownParent = 'e400000000000001';
let consumer;
let launcher;

function processFor(command, args, cwd, environment = {}) {
  const child = spawn(command, args, { cwd, env: { ...process.env, ...environment }, stdio: ['ignore', 'pipe', 'pipe'] });
  let output = '';
  let result;
  let failed;
  for (const stream of [child.stdout, child.stderr]) {
    stream.setEncoding('utf8');
    stream.on('data', data => { output = (output + data).slice(-32_768); });
  }
  child.once('error', error => { failed = error; });
  const exited = new Promise(resolve => child.once('exit', (code, signal) => resolve(result = { code, signal })));
  return {
    async match(pattern) {
      let found;
      await expect.poll(() => {
        if (result || failed) throw new Error(`Child exited: ${failed ?? result.code}\n${output}`);
        found = output.match(pattern)?.[1];
        return found ?? '';
      }, { timeout: 15000, message: `Waiting for ${pattern}` }).not.toBe('');
      return found;
    },
    async close() {
      if (result || failed || child.pid === undefined) return;
      child.kill('SIGTERM');
      let timer;
      try {
        await Promise.race([exited, new Promise(resolve => {
          timer = setTimeout(() => {
            if (!result) child.kill('SIGKILL');
            resolve();
          }, 8500);
        })]);
        await exited;
      } finally {
        clearTimeout(timer);
      }
    },
  };
}

async function start(command, database) {
  const child = processFor(process.execPath, [launcher, command, '--no-open',
    '--collector-port', '0', '--query-port', '0', '--db-path', database], consumer,
    { REQUEST_XRAY_SINK_TIMEOUT_MS: '60000' });
  try {
    return {
      process: child,
      url: await child.match(/Local trace view: (http:\/\/127\.0\.0\.1:\d+\/)/),
      otlp: await child.match(/OTLP\/HTTP listener at (http:\/\/127\.0\.0\.1:\d+\/v1\/traces)/),
    };
  } catch (error) {
    await child.close();
    throw error;
  }
}

test.beforeAll(async () => {
  consumer = await fs.mkdtemp(path.join(os.tmpdir(), 'xray-dashboard-npm-'));
  await fs.writeFile(path.join(consumer, 'package.json'), '{"name":"dashboard-consumer","private":true}');
  const manifest = JSON.parse(await fs.readFile(path.join(stage, 'cli/package.json'), 'utf8'));
  const platform = currentTarget();
  if (!platform) throw new Error('No dashboard test platform package for this host');
  const npm = process.platform === 'win32' ? 'npm.cmd' : 'npm';
  const env = { ...process.env, npm_config_cache: path.join(consumer, 'npm-cache') };
  execFileSync(npm, ['install', '--offline', '--ignore-scripts', '--no-audit', '--no-fund',
    path.join(stage, `request-xray-${manifest.version}.tgz`),
    path.join(stage, `request-xray-cli-${platform.key}-${manifest.version}.tgz`),
  ], { cwd: consumer, env, encoding: 'utf8', shell: process.platform === 'win32' });
  launcher = path.join(consumer, 'node_modules/request-xray/bin/request-xray.cjs');
  await fs.mkdir(screenshots, { recursive: true });
});

test.afterAll(async () => {
  if (consumer) await fs.rm(consumer, { recursive: true, force: true });
});

test('installed dashboard renders exact spans from a real instrumented request', async ({ page }) => {
  const live = await start('dev', path.join(consumer, 'live.sqlite'));
  const applications = [];
  try {
    const browserErrors = [];
    page.on('pageerror', error => browserErrors.push(error.message));
    page.on('console', message => {
      if (message.type() === 'error' && message.text().includes('Content Security Policy')) browserErrors.push(message.text());
    });
    await page.goto(live.url);
    await expect(page.getByTestId('empty-state')).toBeVisible();
    const shared = { PORT: '0', REQUEST_XRAY_COLLECTOR_ENDPOINT: live.otlp };
    const downstream = processFor(process.execPath, ['--require', '@request-xray/node/register', 'downstream.cjs'], example,
      { ...shared, OTEL_SERVICE_NAME: 'dashboard-downstream' });
    applications.push(downstream);
    const downstreamUrl = await downstream.match(/Listening on (http:\/\/127\.0\.0\.1:\d+)/);
    const upstream = processFor(process.execPath, ['--import', '@request-xray/node/register', 'app.mjs'], example,
      { ...shared, OTEL_SERVICE_NAME: 'dashboard-upstream', DOWNSTREAM_URL: downstreamUrl });
    applications.push(upstream);
    const upstreamUrl = await upstream.match(/Listening on (http:\/\/127\.0\.0\.1:\d+)/);
    const response = await fetch(`${upstreamUrl}/orders/42`, {
      headers: { traceparent: `00-${knownTrace}-${knownParent}-01` }, signal: AbortSignal.timeout(5000),
    });
    expect(response.status).toBe(200);
    expect((await response.json()).id).toBe('42');

    const expected = async () => {
      const result = await fetch(new URL(`api/v1/traces/${knownTrace}/spans`, live.url));
      if (!result.ok) return [];
      return (await result.json()).items;
    };
    await expect.poll(async () => (await expected()).filter(row => row.kind === 2).length).toBe(3);
    const exact = (await expected()).find(row => row.name === 'orders.lookup');
    expect(exact).toBeTruthy();
    await page.reload();
    await expect(page.getByTestId('trace-table')).toContainText(knownTrace);
    const service = page.getByRole('textbox', { name: 'Service', exact: true });
    await service.fill('nonexistent-fixture-service');
    await page.getByRole('button', { name: 'Apply filters' }).click();
    await expect(page.getByTestId('empty-state')).toContainText('No matching traces');
    await service.fill('dashboard-upstream');
    await page.getByRole('button', { name: 'Apply filters' }).click();
    await expect(page.getByTestId('trace-table')).toContainText(knownTrace);
    await page.getByTestId('trace-table').locator(`a[href*="${knownTrace}"]`).focus();
    await page.keyboard.press('Enter');
    await expect(page.getByTestId('span-tree')).toBeVisible();
    await expect(page.getByTestId('missing-parent-indicator')).toBeVisible();
    const row = page.getByTestId('span-row').filter({ hasText: 'orders.lookup' });
    await row.getByRole('button', { name: /^Inspect orders\.lookup/ }).focus();
    await page.keyboard.press('Enter');
    await expect(page.getByTestId('selected-span-inspector')).toContainText(exact.spanId);
    await expect(page.getByTestId('span-start-ns')).toHaveText(exact.startedAtUnixNs);
    await expect(page.getByTestId('span-duration-ns')).toHaveText(exact.durationNs);
    await page.getByRole('button', { name: 'Zoom in' }).click();
    await expect(page.getByRole('button', { name: 'Zoom out' })).toBeEnabled();
    const pan = page.getByRole('slider', { name: 'Pan timeline' });
    await pan.focus();
    await page.keyboard.press('End');
    await expect(pan).toHaveValue('1000');
    await page.keyboard.press('Home');
    await page.getByRole('button', { name: 'Zoom out' }).click();
    await page.getByRole('button', { name: 'Collapse all' }).click();
    await expect(page.getByRole('button', { name: 'Expand all' })).toBeEnabled();
    await page.getByRole('button', { name: 'Expand all' }).click();
    expect(browserErrors).toEqual([]);
    await page.screenshot({ path: path.join(screenshots, 'live-trace.png') });
  } finally {
    await Promise.all(applications.reverse().map(application => application.close()));
    await live.process.close();
  }
});

test('demo is labeled and shows the recorded error', async ({ page }) => {
  const demo = await start('demo', path.join(consumer, 'unused.sqlite'));
  try {
    await page.goto(demo.url);
    await expect(page.getByTestId('trace-table')).toContainText(/error/i);
    await page.route('**/api/v1/health', route => route.abort());
    await page.getByRole('button', { name: 'Refresh', exact: true }).click();
    await expect(page.getByRole('alert').filter({ hasText: 'Collector unavailable' })).toBeVisible();
    await page.unroute('**/api/v1/health');
    await page.getByRole('button', { name: 'Refresh', exact: true }).click();
    await expect(page.getByRole('alert').filter({ hasText: 'Collector unavailable' })).toHaveCount(0);
    await expect(page.getByTestId('demo-indicator')).toBeVisible();
    await expect(page.getByTestId('trace-table')).toContainText(/error/i);
    await page.screenshot({ path: path.join(screenshots, 'dashboard-desktop.png') });
    await page.setViewportSize({ width: 390, height: 844 });
    await page.emulateMedia({ reducedMotion: 'reduce' });
    await expect(page.getByTestId('demo-indicator')).toBeVisible();
    expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(391);
    await page.screenshot({ path: path.join(screenshots, 'dashboard-mobile.png') });
    await page.getByTestId('trace-table').locator('tbody a').first().click();
    const waterfall = page.getByTestId('span-tree');
    await expect(waterfall).toHaveAttribute('data-span-count', '3');
    expect((await waterfall.boundingBox()).height).toBeLessThan(160);
    await expect(page.getByTestId('selected-span-inspector')).toBeVisible();
    await page.screenshot({ path: path.join(screenshots, 'waterfall-mobile.png'), fullPage: true });
  } finally {
    await demo.process.close();
  }
});

test('waterfall benchmarks and windows 1k and 10k real persisted spans', async ({ page }) => {
  test.setTimeout(120000);
  const live = await start('dev', path.join(consumer, 'bench.sqlite'));
  const measurements = [];
  try {
    for (const [count, byte] of [[1000, '43'], [10000, '44']]) {
      const body = await fs.readFile(path.join(fixtures, `${count}.pb`));
      const response = await fetch(live.otlp, { method: 'POST', headers: { 'content-type': 'application/x-protobuf' },
        body, signal: AbortSignal.timeout(50000) });
      expect(response.status).toBe(200);
      expect((await response.arrayBuffer()).byteLength).toBe(0);
      const traceId = byte.repeat(16);
      const started = performance.now();
      await page.goto(`${live.url}#/traces/${traceId}`);
      await expect(page.getByTestId('span-tree')).toHaveAttribute('data-span-count', String(count), { timeout: 45_000 });
      const elapsedMilliseconds = Math.round(performance.now() - started);
      const mounted = await page.getByTestId('span-row').count();
      expect(mounted).toBeGreaterThan(0);
      expect(mounted).toBeLessThan(100);
      measurements.push({ spans: count, loadMilliseconds: elapsedMilliseconds, mountedRows: mounted });
    }
    const listing = await fs.readFile(path.join(fixtures, 'listing.pb'));
    expect((await fetch(live.otlp, {
      method: 'POST', headers: { 'content-type': 'application/x-protobuf' }, body: listing,
    })).status).toBe(200);
    await page.goto(live.url);
    const links = page.getByTestId('trace-table').locator('tbody a');
    await expect(links).toHaveCount(25);
    const first = await links.allTextContents();
    await page.getByRole('button', { name: 'Next' }).click();
    await expect(links).toHaveCount(3);
    const second = await links.allTextContents();
    expect(new Set([...first, ...second]).size).toBe(28);
    await page.getByRole('button', { name: 'Previous' }).click();
    await expect(links).toHaveText(first);
    console.log(`Dashboard benchmark ${JSON.stringify(measurements)}`);
    await fs.writeFile(path.join(screenshots, 'benchmark.json'), JSON.stringify(measurements, null, 2) + '\n');
  } finally {
    await live.process.close();
  }
});

test('refresh resolves a late parent and preserves skew and dropped-data evidence', async ({ page }) => {
  const live = await start('dev', path.join(consumer, 'late.sqlite'));
  const send = async name => {
    const response = await fetch(live.otlp, {
      method: 'POST', headers: { 'content-type': 'application/x-protobuf' },
      body: await fs.readFile(path.join(fixtures, name)), signal: AbortSignal.timeout(5000),
    });
    expect(response.status).toBe(200);
    expect((await response.arrayBuffer()).byteLength).toBe(0);
  };
  try {
    await send('missing-parent.pb');
    await page.goto(`${live.url}#/traces/${'45'.repeat(16)}`);
    await expect(page.getByTestId('missing-parent-indicator')).toBeVisible();
    await expect(page.getByTestId('selected-span-inspector')).toContainText(/Dropped by the exporter: 2 span attributes/);
    await expect(page.getByText(/sampling coverage is unknown/i)).toBeVisible();
    await send('late-parent.pb');
    await page.getByRole('button', { name: 'Refresh trace' }).click();
    await expect(page.getByTestId('span-tree')).toHaveAttribute('data-span-count', '2');
    await expect(page.getByTestId('missing-parent-indicator')).toHaveCount(0);
    await expect(page.getByTestId('span-tree')).toContainText(/possible clock skew/i);
  } finally {
    await live.process.close();
  }
});
