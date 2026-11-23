const assert = require('node:assert/strict');
const { execFile, spawn } = require('node:child_process');
const { once } = require('node:events');
const { mkdtemp, rm } = require('node:fs/promises');
const { createServer } = require('node:http');
const { tmpdir } = require('node:os');
const path = require('node:path');
const { test } = require('node:test');
const { promisify } = require('node:util');
const runFile = promisify(execFile);
const { setTimeout: delay } = require('node:timers/promises');

const root = path.resolve(__dirname, '../../..');
const examples = path.join(root, 'examples/express-basic');
const lab = path.join(root, 'test');
const collectorBin = process.env.XRAY_TEST_COLLECTOR ?? path.join(root, 'target/debug/xray-collector');
const secret = 'fixture-value-not-for-telemetry';

function launch(command, args, cwd, env) {
  const child = spawn(command, args, { cwd, env: { ...process.env, ...env }, stdio: ['ignore', 'pipe', 'pipe'] });
  const state = { output: '', result: undefined };
  for (const stream of [child.stdout, child.stderr]) {
    stream.setEncoding('utf8');
    stream.on('data', chunk => { state.output = (state.output + chunk).slice(-32_768); });
  }
  const done = new Promise((resolve, reject) => {
    child.once('error', reject);
    child.once('exit', (code, signal) => {
      state.result = { code, signal };
      resolve(state.result);
    });
  });
  // Retain spawn errors for the awaiting caller even if readiness fails first.
  void done.catch(() => {});
  return {
    async locate(pattern) {
      return waitFor(() => {
        if (state.result) throw new Error(`Process exited: ${state.output}`);
        return state.output.match(pattern)?.[1];
      }, () => state.output);
    },
    async stop() {
      if (!state.result && child.pid !== undefined) child.kill('SIGTERM');
      try {
        const result = await withTimeout(done, 8000);
        assert.equal(result.code, 0, state.output);
      } finally {
        if (!state.result && child.pid !== undefined) child.kill('SIGKILL');
      }
    },
  };
}

async function withTimeout(operation, milliseconds) {
  let timer;
  try {
    return await Promise.race([operation, new Promise((_, reject) => {
      timer = setTimeout(() => reject(new Error('process shutdown timed out')), milliseconds);
    })]);
  } finally {
    clearTimeout(timer);
  }
}

async function waitFor(check, detail = () => '') {
  const deadline = Date.now() + 12_000;
  while (Date.now() < deadline) {
    const result = await check();
    if (result) return result;
    await delay(50);
  }
  throw new Error(`Timed out: ${detail()}`);
}

test('CJS and ESM Express propagate real spans, NestJS starts, and collector outage is isolated', { timeout: 45_000 }, async () => {
  const temp = await mkdtemp(path.join(tmpdir(), 'xray-node-e2e-'));
  const processes = [];
  let relay;
  const start = (...args) => {
    const process = launch(...args);
    processes.push(process);
    return process;
  };
  try {
    const collector = start(collectorBin, [], root, {
      REQUEST_XRAY_OTLP_LISTEN: '127.0.0.1:0',
      REQUEST_XRAY_QUERY_LISTEN: '127.0.0.1:0',
      REQUEST_XRAY_DB_PATH: path.join(temp, 'traces.sqlite'),
    });
    const otlp = await collector.locate(/OTLP\/HTTP listener at (http:\/\/127\.0\.0\.1:\d+\/v1\/traces)/);
    const query = await collector.locate(/Query API at (http:\/\/127\.0\.0\.1:\d+\/api\/v1)/);
    const wireBodies = [];
    relay = createServer(async (request, response) => {
      try {
        const chunks = [];
        for await (const chunk of request) chunks.push(chunk);
        const payload = Buffer.concat(chunks);
        assert.equal(request.headers['content-type'], 'application/x-protobuf');
        wireBodies.push(payload);
        const result = await fetch(otlp, {
          method: 'POST', body: payload, headers: { 'content-type': request.headers['content-type'] },
          signal: AbortSignal.timeout(3000),
        });
        response.writeHead(result.status, { 'content-type': result.headers.get('content-type') });
        response.end(Buffer.from(await result.arrayBuffer()));
      } catch {
        response.writeHead(503).end();
      }
    });
    relay.listen(0, '127.0.0.1');
    await once(relay, 'listening');
    const collectorEndpoint = `http://127.0.0.1:${relay.address().port}/v1/traces`;
    const common = { PORT: '0', REQUEST_XRAY_COLLECTOR_ENDPOINT: collectorEndpoint, OTEL_EXPORTER_OTLP_COMPRESSION: 'none' };
    const downstream = start(process.execPath, ['--require', '@request-xray/node/register', 'downstream.cjs'], examples, {
      ...common, OTEL_SERVICE_NAME: 'sdk-downstream',
    });
    const downstreamUrl = await downstream.locate(/Listening on (http:\/\/127\.0\.0\.1:\d+)/);
    const upstream = start(process.execPath, ['--import', '@request-xray/node/register', 'app.mjs'], examples, {
      ...common, DOWNSTREAM_URL: downstreamUrl, OTEL_SERVICE_NAME: 'sdk-upstream',
    });
    const upstreamUrl = await upstream.locate(/Listening on (http:\/\/127\.0\.0\.1:\d+)/);
    const traceId = '1234567890abcdef1234567890abcdef';
    const parentId = '1234567890abcdef';
    const response = await fetch(`${upstreamUrl}/orders/42?token=${secret}`, {
      headers: { traceparent: `00-${traceId}-${parentId}-01`, authorization: `Bearer ${secret}`, cookie: `session=${secret}` },
      signal: AbortSignal.timeout(5000),
    });
    assert.equal(response.status, 200);
    const result = await response.json();
    assert.equal(result.id, '42');
    assert.equal(result.activeSpanPreserved, true);
    assert.match(result.traceparent, new RegExp(`^00-${traceId}-[0-9a-f]{16}-01$`));

    const page = await waitFor(async () => {
      const response = await fetch(`${query}/traces/${traceId}/spans`, { signal: AbortSignal.timeout(3000) });
      if (response.status === 404) return undefined;
      assert.equal(response.status, 200);
      const body = await response.json();
      const services = body.items.map(span => span.resource?.attributes.find(attr => attr.key === 'service.name')?.value.value);
      return services.filter(service => service === 'sdk-downstream').length >= 2
        && body.items.filter(span => span.kind === 3).length >= 2 && body.items.some(span => span.name === 'orders.lookup')
        ? body : undefined;
    });
    const spans = page.items;
    const inbound = spans.find(span => span.kind === 2 && span.parentSpanId === parentId);
    assert.ok(inbound, 'expected inbound SERVER span with remote W3C parent');
    assert.ok(spans.some(span => span.route === '/orders/:id'), JSON.stringify(spans.map(span => [span.name, span.route])));
    const manual = spans.find(span => span.name === 'orders.lookup');
    assert.ok(manual.events.some(event => event.name === 'orders.lookup.complete'));
    const clients = spans.filter(span => span.kind === 3 && span.parentSpanId === manual.spanId);
    assert.equal(clients.length, 2);
    for (const client of clients) {
      assert.ok(spans.some(span => span.kind === 2 && span.parentSpanId === client.spanId), client.spanId);
    }
    assert.ok(spans.some(span => span.attributes.some(attribute => /http\.(status_code|response\.status_code)/.test(attribute.key)
      && attribute.value.value === '200')));
    assert.ok(wireBodies.length > 0);
    assert.ok(wireBodies.every(body => !body.includes(secret)), 'SDK leaked a header, body or query value before collector redaction');

    const nestEnv = { XRAY_MODE: 'xray', XRAY_COLLECTOR_URL: collectorEndpoint };
    const nestDownstream = start(process.execPath, ['--require', './dist/instrumentation/register.js', 'dist/downstream.js'], lab, {
      ...nestEnv, LAB_DOWNSTREAM_PORT: '0', OTEL_SERVICE_NAME: 'sdk-nestjs-downstream',
    });
    const nestDownstreamUrl = await nestDownstream.locate(/Downstream on (http:\/\/127\.0\.0\.1:\d+)/);
    const nest = start(process.execPath, ['--require', './dist/instrumentation/register.js', 'dist/main.js'], lab, {
      ...nestEnv, LAB_PORT: '0', LAB_DOWNSTREAM_PORT: new URL(nestDownstreamUrl).port, OTEL_SERVICE_NAME: 'sdk-nestjs',
    });
    const nestUrl = await nest.locate(/Checkout app on (http:\/\/127\.0\.0\.1:\d+)/);
    const nestResponse = await fetch(`${nestUrl}/orders/checkout`, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ items: ['sku-1'], total: 5, mode: 'success' }),
      signal: AbortSignal.timeout(5000),
    });
    assert.equal(nestResponse.status, 201);
    await nestResponse.arrayBuffer();
    await Promise.all([nest.stop(), nestDownstream.stop()]);
    const nestTraces = await fetch(`${query}/traces?service=sdk-nestjs`, { signal: AbortSignal.timeout(3000) }).then(response => response.json());
    assert.ok(nestTraces.items.some(trace => trace.serviceCount === 2 && trace.spanCount >= 4),
      'NestJS shutdown must flush HTTP and manual spans from both services');

    await runFile(process.execPath, ['-e', `
      const { trace } = require('@opentelemetry/api');
      const { TracerProvider } = require('@opentelemetry/sdk-trace');
      const { resourceFromAttributes } = require('@opentelemetry/resources');
      const { createXRaySpanProcessor, startXRay, withSpan } = require('./dist');
      const provider = new TracerProvider({
        resource: resourceFromAttributes({ 'service.name': 'sdk-existing' }),
        spanProcessors: [createXRaySpanProcessor()],
      });
      trace.setGlobalTracerProvider(provider);
      startXRay({ provider: 'use-existing' });
      withSpan('existing-provider', () => {}).then(() => provider.shutdown())
        .catch(error => { console.error(error); process.exitCode = 1; });
    `], { cwd: path.resolve(__dirname, '..'), env: { ...process.env, ...common }, timeout: 8000 });
    const existing = await fetch(`${query}/traces?service=sdk-existing`, { signal: AbortSignal.timeout(3000) }).then(response => response.json());
    assert.equal(existing.items[0].spanCount, 1, 'existing provider must export through the attached processor');

    await collector.stop();
    const duringOutage = await fetch(`${upstreamUrl}/orders/43`, { signal: AbortSignal.timeout(5000) });
    assert.equal(duringOutage.status, 200);
    assert.equal((await duringOutage.json()).id, '43');
  } finally {
    const results = await Promise.allSettled(processes.reverse().map(process => process.stop()));
    if (relay?.listening) {
      relay.closeAllConnections();
      await new Promise(resolve => relay.close(resolve));
    }
    await rm(temp, { recursive: true, force: true });
    for (const result of results) if (result.status === 'rejected') throw result.reason;
  }
});
