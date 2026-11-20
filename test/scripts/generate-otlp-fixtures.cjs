const assert = require('node:assert/strict');
const { once } = require('node:events');
const { writeFile } = require('node:fs/promises');
const { createServer } = require('node:http');
const path = require('node:path');
const { ROOT_CONTEXT, SpanKind, SpanStatusCode, trace } = require('@opentelemetry/api');
const { OTLPTraceExporter } = require('@opentelemetry/exporter-trace-otlp-proto');
const { resourceFromAttributes } = require('@opentelemetry/resources');
const { AlwaysOnSampler, InMemorySpanExporter, SimpleSpanProcessor, TracerProvider } = require('@opentelemetry/sdk-trace');

const output = process.argv[2] ?? path.resolve(__dirname, '../../tests/fixtures/otlp');
const epochSeconds = 1_793_500_000;
const time = (nanos) => [epochSeconds, nanos];

function provider(serviceName, memory, ids) {
  return new TracerProvider({
    resource: resourceFromAttributes({ 'service.name': serviceName }),
    sampler: new AlwaysOnSampler(),
    spanLimits: { attributeCountLimit: 2, attributeValueLengthLimit: Infinity },
    spanProcessors: [new SimpleSpanProcessor({ exporter: memory })],
    idGenerator: {
      generateTraceId: () => '11111111111111111111111111111111',
      generateSpanId: () => (++ids.next).toString(16).padStart(16, '0'),
    },
  });
}

async function main() {
  // Capture exactly what the official protobuf HTTP exporter sends.
  let captured;
  const server = createServer(async (request, response) => {
    if (request.method !== 'POST' || request.url !== '/v1/traces' ||
        request.headers['content-type'] !== 'application/x-protobuf') {
      response.writeHead(415).end();
      return;
    }
    const chunks = [];
    let length = 0;
    for await (const chunk of request) {
      length += chunk.length;
      if (length > 128 * 1024) {
        response.writeHead(413).end();
        return;
      }
      chunks.push(chunk);
    }
    captured = Buffer.concat(chunks);
    response.writeHead(200, { 'Content-Type': 'application/x-protobuf' }).end();
  });
  server.listen(0, '127.0.0.1');
  await once(server, 'listening');
  const exporter = new OTLPTraceExporter({
    url: `http://127.0.0.1:${server.address().port}/v1/traces`,
    compression: 'none',
    timeoutMillis: 5000,
  });
  const ids = { next: 0 };
  const memory = new InMemorySpanExporter();
  const orders = provider('orders-fixture', memory, ids);
  const payments = provider('payments-fixture', memory, ids);

  async function capture(name) {
    await Promise.all([orders.forceFlush(), payments.forceFlush()]);
    captured = undefined;
    const result = await new Promise((resolve) => exporter.export(memory.getFinishedSpans(), resolve));
    assert.equal(result.code, 0, result.error?.message);
    assert.ok(captured?.length);
    await writeFile(path.join(output, name), captured);
    memory.reset();
  }

  try {
    const tracer = orders.getTracer('fixture.orders', '1.0.0', { schemaUrl: 'https://example.test/scope/orders' });
    tracer.startSpan('single', { startTime: time(1) }, ROOT_CONTEXT).end(time(2));
    await capture('single.pb');

    const root = tracer.startSpan('checkout', { startTime: time(10), kind: SpanKind.SERVER }, ROOT_CONTEXT);
    const parent = trace.setSpan(ROOT_CONTEXT, root);
    const inventory = tracer.startSpan('inventory', { startTime: time(20) }, parent);
    const paymentTracer = payments.getTracer('fixture.payments', '1.0.0', { schemaUrl: 'https://example.test/scope/payments' });
    const payment = paymentTracer.startSpan('charge', {
      startTime: time(30),
      kind: SpanKind.CLIENT,
      links: [{ context: inventory.spanContext(), attributes: { relation: 'inventory' } }],
      attributes: { city: 'Zürich 東京', amount: 42, dropped: true },
    }, parent);
    payment.addEvent('paiement refusé', { retry: false }, time(40));
    payment.setStatus({ code: SpanStatusCode.ERROR, message: 'synthetic decline' });
    inventory.end(time(50));
    payment.end(time(60));
    root.end(time(70));
    await capture('distributed.pb');

    tracer.startSpan('large-attribute', {
      startTime: time(80),
      attributes: { value: 'x'.repeat(64 * 1024) },
    }, ROOT_CONTEXT).end(time(81));
    await capture('large-attribute.pb');
  } finally {
    await Promise.allSettled([orders.shutdown(), payments.shutdown(), exporter.shutdown()]);
    server.close();
  }
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
