import { register } from 'node:module';

register('@opentelemetry/instrumentation/hook.mjs', import.meta.url);
const { startXRay } = await import('./index.mjs');
startXRay();
