import { context, ProxyTracerProvider, SpanStatusCode, trace, type Attributes, type Span } from '@opentelemetry/api';
import { W3CTraceContextPropagator } from '@opentelemetry/core';
import { OTLPTraceExporter } from '@opentelemetry/exporter-trace-otlp-proto';
import { ExpressInstrumentation } from '@opentelemetry/instrumentation-express';
import { HttpInstrumentation } from '@opentelemetry/instrumentation-http';
import { UndiciInstrumentation } from '@opentelemetry/instrumentation-undici';
import { resourceFromAttributes } from '@opentelemetry/resources';
import { NodeSDK } from '@opentelemetry/sdk-node';
import {
  AlwaysOffSampler, AlwaysOnSampler, BatchSpanProcessor, ParentBasedSampler, TraceIdRatioBasedSampler,
  type SpanProcessor,
} from '@opentelemetry/sdk-trace';

import { safeAttributes, safeName, sanitizeResource, sanitizeSpan } from './privacy.js';

export interface ExportOptions {
  collectorEndpoint?: string;
  maxQueueSize?: number;
  maxExportBatchSize?: number;
  scheduledDelayMs?: number;
  exportTimeoutMs?: number;
}

export interface StartXRayOptions extends ExportOptions {
  serviceName?: string;
  serviceVersion?: string;
  environment?: string;
  capture?: { http?: boolean; express?: boolean; fetch?: boolean };
  sampling?: { mode: 'parentbased_traceidratio'; probability: number };
  provider?: 'standalone' | 'use-existing';
  shutdownTimeoutMs?: number;
}

export interface XRayHandle {
  readonly ownsProvider: boolean;
  /** Returns false when the operation fails or exceeds its deadline. */
  flush(): Promise<boolean>;
  shutdown(): Promise<boolean>;
}

const tracerName = 'request-xray';
const initialDelegate = new ProxyTracerProvider().getDelegate();
let active: XRayHandle | undefined;

function providerExists(): boolean {
  const provider = trace.getTracerProvider();
  return !(provider instanceof ProxyTracerProvider) || provider.getDelegate() !== initialDelegate;
}

function boundedInteger(name: string, value: number, maximum: number): number {
  if (!Number.isInteger(value) || value < 1 || value > maximum) {
    throw new RangeError(`${name} must be an integer from 1 through ${maximum}`);
  }
  return value;
}

function endpoint(options: ExportOptions): string {
  const value = options.collectorEndpoint ?? process.env.REQUEST_XRAY_COLLECTOR_ENDPOINT
    ?? process.env.OTEL_EXPORTER_OTLP_TRACES_ENDPOINT ?? 'http://127.0.0.1:4318/v1/traces';
  const url = new URL(value);
  if (!['http:', 'https:'].includes(url.protocol) || url.username || url.password || url.search || url.hash) {
    throw new TypeError('collectorEndpoint must be an HTTP(S) trace URL without credentials, query or fragment');
  }
  return url.href;
}

/** Attach this during construction of an existing OpenTelemetry provider. */
export function createXRaySpanProcessor(options: ExportOptions = {}): SpanProcessor {
  const url = endpoint(options);
  const maxQueueSize = boundedInteger('maxQueueSize', options.maxQueueSize ?? 256, 2048);
  const batchSize = boundedInteger('maxExportBatchSize', options.maxExportBatchSize ?? Math.min(32, maxQueueSize), maxQueueSize);
  const timeout = boundedInteger('exportTimeoutMs', options.exportTimeoutMs ?? 2000, 30_000);
  const delay = boundedInteger('scheduledDelayMs', options.scheduledDelayMs ?? 500, 30_000);
  const exporter = new OTLPTraceExporter({ url, timeoutMillis: timeout, concurrencyLimit: 1 });
  const batch = new BatchSpanProcessor({
    exporter: {
      export: (spans, callback) => exporter.export(spans.map(sanitizeResource), callback),
      shutdown: () => exporter.shutdown(),
      forceFlush: () => exporter.forceFlush(),
    },
    maxQueueSize,
    maxExportBatchSize: batchSize,
    scheduledDelayMillis: delay,
    exportTimeoutMillis: timeout,
  });
  return {
    onStart: (span, parent) => batch.onStart(span, parent),
    onEnd: span => batch.onEnd(sanitizeSpan(span)),
    forceFlush: () => batch.forceFlush(),
    shutdown: () => batch.shutdown(),
  };
}

function withinDeadline(operation: () => Promise<void>, timeoutMs: number): Promise<boolean> {
  let timer: NodeJS.Timeout;
  const expired = new Promise<false>(resolve => { timer = setTimeout(() => resolve(false), timeoutMs); });
  const completed = Promise.resolve().then(operation).then(() => true, () => false);
  return Promise.race([expired, completed]).finally(() => clearTimeout(timer));
}

export function startXRay(options: StartXRayOptions = {}): XRayHandle {
  if (active) throw new Error('Request X-Ray is already initialized in this process');
  const existing = providerExists();
  if (options.provider === 'use-existing') {
    if (!existing) throw new Error('use-existing requires an initialized OpenTelemetry tracer provider');
    active = { ownsProvider: false, flush: async () => false, shutdown: async () => false };
    return active;
  }
  if (existing) throw new Error('An OpenTelemetry tracer provider is active; configure createXRaySpanProcessor on that provider or set provider to use-existing');
  if (process.env.OTEL_SDK_DISABLED?.trim().toLowerCase() === 'true') {
    throw new Error('OTEL_SDK_DISABLED prevents Request X-Ray from initializing');
  }

  const express = options.capture?.express !== false;
  if (express && Object.keys(require.cache).some(file => /[\\/]express[\\/]index\.js$/.test(file))) {
    throw new Error('Request X-Ray must initialize before Express is imported; use the startup preload');
  }

  const serviceName = (options.serviceName ?? process.env.OTEL_SERVICE_NAME ?? 'request-xray-app').trim();
  if (!serviceName || serviceName.length > 256) throw new TypeError('serviceName must contain 1 through 256 characters');
  const shutdownTimeout = boundedInteger('shutdownTimeoutMs', options.shutdownTimeoutMs ?? 5000, 30_000);
  const probability = options.sampling?.probability ?? 1;
  if (!Number.isFinite(probability) || probability < 0 || probability > 1) {
    throw new RangeError('sampling.probability must be between 0 and 1');
  }

  const instrumentations = [
    ...(options.capture?.http === false ? [] : [new HttpInstrumentation()]),
    ...(express ? [new ExpressInstrumentation()] : []),
    ...(options.capture?.fetch === false ? [] : [new UndiciInstrumentation()]),
  ];
  const processor = createXRaySpanProcessor(options);
  const sdk = new NodeSDK({
    autoDetectResources: false,
    metricReaders: [],
    logRecordProcessors: [],
    instrumentations,
    textMapPropagator: new W3CTraceContextPropagator(),
    sampler: new ParentBasedSampler({
      root: probability === 0 ? new AlwaysOffSampler()
        : probability === 1 ? new AlwaysOnSampler() : new TraceIdRatioBasedSampler(probability),
    }),
    resource: resourceFromAttributes(safeAttributes({
      'service.name': serviceName,
      'service.version': options.serviceVersion,
      'deployment.environment.name': options.environment,
    })),
    spanProcessors: [processor],
    spanLimits: {
      attributeCountLimit: 64, attributeValueLengthLimit: 1024,
      eventCountLimit: 32, linkCountLimit: 32,
      attributePerEventCountLimit: 32, attributePerLinkCountLimit: 32,
    },
  });

  sdk.start();
  let stopping: Promise<boolean> | undefined;
  active = {
    ownsProvider: true,
    flush: () => stopping ?? withinDeadline(() => processor.forceFlush(), shutdownTimeout),
    shutdown: () => stopping ??= withinDeadline(async () => {
      for (const instrumentation of instrumentations) instrumentation.disable();
      await sdk.shutdown();
    }, shutdownTimeout),
  };
  return active;
}

export function getXRay(): XRayHandle | undefined {
  return active;
}

export function getActiveSpan(): Span | undefined {
  return trace.getSpan(context.active());
}

export async function withSpan<T>(name: string, fn: (span: Span) => T | PromiseLike<T>): Promise<T> {
  return trace.getTracer(tracerName).startActiveSpan(safeName(name), async span => {
    try {
      return await fn(span);
    } catch (error) {
      span.setStatus({ code: SpanStatusCode.ERROR });
      throw error;
    } finally {
      span.end();
    }
  });
}

export function addEvent(name: string, attrs?: Attributes): void {
  getActiveSpan()?.addEvent(safeName(name), safeAttributes(attrs));
}
