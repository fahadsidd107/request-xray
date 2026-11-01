/** Preloaded before any NestJS/http imports. XRAY_MODE=off|otel|xray. */
import { NodeSDK } from '@opentelemetry/sdk-node';
import { getNodeAutoInstrumentations } from '@opentelemetry/auto-instrumentations-node';
import { OTLPTraceExporter } from '@opentelemetry/exporter-trace-otlp-http';
const mode = (process.env.XRAY_MODE || 'off').toLowerCase();
const service = process.env.OTEL_SERVICE_NAME || 'request-xray-lab';
const url = process.env.XRAY_COLLECTOR_URL || 'http://127.0.0.1:4318/v1/traces';
if (mode === 'otel') {
  const sdk = new NodeSDK({
    serviceName: service,
    traceExporter: new OTLPTraceExporter({ url }),
    instrumentations: [getNodeAutoInstrumentations({
      '@opentelemetry/instrumentation-fs': { enabled: false }
    })]
  });
  sdk.start();
  console.log(`[lab:${service}] OTLP instrumentation enabled -> ${url}`);
  const shutdown = () => { void sdk.shutdown().finally(() => process.exit(0)); };
  process.once('SIGTERM', shutdown);
  process.once('SIGINT', shutdown);
} else if (mode === 'xray') {
  // The project SDK is not released yet. Require the real local build explicitly.
  // Never silently fall back: that would produce misleading integration results.
  let mod: { startXRay?: (opts: Record<string, unknown>) => unknown };
  try { mod = require('@request-xray/node') as typeof mod; }
  catch { throw new Error('XRAY_MODE=xray requires a built and resolvable @request-xray/node; see README.md'); }
  if (typeof mod.startXRay !== 'function') {
    throw new Error('@request-xray/node must export startXRay(options) per DOCUMENTATION.md');
  }
  const result = mod.startXRay({ serviceName: service, collectorEndpoint: url, capture: { http: true, database: true } });
  if (result && typeof (result as Promise<unknown>).then === 'function') {
    throw new Error('Async startXRay is unsupported by this preload adapter. Provide a synchronous bootstrap API or adjust the preload before testing.');
  }
  console.log(`[lab:${service}] Request X-Ray SDK activated`);
} else if (mode !== 'off') {
  throw new Error(`Unknown XRAY_MODE=${mode}; expected off, otel or xray`);
}
