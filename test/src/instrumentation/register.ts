/** Preloaded before any NestJS/http imports. XRAY_MODE=off|otel|xray. */
import { NodeSDK } from '@opentelemetry/sdk-node';
import { getNodeAutoInstrumentations } from '@opentelemetry/auto-instrumentations-node';
import { OTLPTraceExporter } from '@opentelemetry/exporter-trace-otlp-proto';
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
  const { startXRay } = require('@request-xray/node') as typeof import('@request-xray/node');
  const sdk = startXRay({ serviceName: service, collectorEndpoint: url });
  const shutdown = () => { void sdk.shutdown().finally(() => process.exit(0)); };
  process.once('SIGTERM', shutdown);
  process.once('SIGINT', shutdown);
  console.log(`[lab:${service}] Request X-Ray SDK activated`);
} else if (mode !== 'off') {
  throw new Error(`Unknown XRAY_MODE=${mode}; expected off, otel or xray`);
}
