const assert = require('node:assert/strict');
const { test } = require('node:test');
const { spawnSync } = require('node:child_process');
const { resolve } = require('node:path');

const cwd = resolve(__dirname, '..');

function run(source, directory = cwd) {
  const result = spawnSync(process.execPath, ['-e', source], { cwd: directory, encoding: 'utf8', timeout: 10000 });
  assert.equal(result.status, 0, result.error?.message ?? result.stderr);
}

test('manual helpers preserve asynchronous context and propagate application failures', () => {
  run(`
    const assert = require('node:assert/strict');
    const { startXRay, withSpan, getActiveSpan, addEvent } = require('./dist');
    const sdk = startXRay({ collectorEndpoint: 'http://127.0.0.1:0/v1/traces', capture: { http: false, express: false, fetch: false }, sampling: { mode: 'parentbased_traceidratio', probability: 1 } });
    (async () => {
      const failure = Error('application failure');
      assert.equal(await withSpan('outer', async span => {
        await Promise.resolve();
        assert.equal(getActiveSpan(), span);
        addEvent('safe', { 'item.count': 1 });
        return withSpan('inner', child => {
          assert.equal(child.spanContext().traceId, span.spanContext().traceId);
          return 7;
        });
      }), 7);
      await assert.rejects(withSpan('failure', () => { throw failure; }), error => error === failure);
      assert.equal(getActiveSpan(), undefined);
      assert.strictEqual(sdk.shutdown(), sdk.shutdown());
      await sdk.shutdown();
    })().catch(error => { console.error(error); process.exitCode = 1; });
  `);
});

test('existing providers require an explicit coexistence strategy', () => {
  run(`
    const assert = require('node:assert/strict');
    const { trace } = require('@opentelemetry/api');
    const { TracerProvider } = require('@opentelemetry/sdk-trace');
    const { startXRay, createXRaySpanProcessor } = require('./dist');
    assert.throws(() => startXRay({ provider: 'use-existing' }), /requires an initialized/);
    const processor = createXRaySpanProcessor();
    const provider = new TracerProvider({ spanProcessors: [processor] });
    assert.equal(trace.setGlobalTracerProvider(provider), true);
    assert.throws(() => startXRay(), /provider is active/);
    const borrowed = startXRay({ provider: 'use-existing' });
    assert.equal(borrowed.ownsProvider, false);
    assert.equal(trace.getTracerProvider().getDelegate(), provider);
    borrowed.shutdown().then(result => assert.equal(result, false)).then(() => provider.shutdown())
      .catch(error => { console.error(error); process.exitCode = 1; });
  `);
});

test('rejects invalid export settings before initialization', () => {
  const { createXRaySpanProcessor } = require('../dist');
  assert.throws(() => createXRaySpanProcessor({ maxQueueSize: 0 }), /maxQueueSize/);
  assert.throws(() => createXRaySpanProcessor({ maxQueueSize: 2, maxExportBatchSize: 3 }), /maxExportBatchSize/);
  assert.throws(() => createXRaySpanProcessor({ collectorEndpoint: 'http://user:password@example.test/v1/traces' }), /without credentials/);
});

test('late CommonJS startup fails explicitly after Express has loaded', () => {
  run(`
    const assert = require('node:assert/strict');
    require('express');
    const { startXRay } = require('@request-xray/node');
    assert.throws(() => startXRay(), /before Express is imported/);
  `, resolve(cwd, '../../examples/express-basic'));
});
