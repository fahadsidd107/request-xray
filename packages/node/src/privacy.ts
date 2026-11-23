import type { Attributes, AttributeValue } from '@opentelemetry/api';
import { resourceFromAttributes } from '@opentelemetry/resources';
import type { ReadableSpan } from '@opentelemetry/sdk-trace';

const denied = /authorization|cookie|password|passwd|apikey|token|secret|credential|cardnumber|paymentcard|creditcard|privatekey|baggage|requestbody|responsebody|requestheader|responseheader|sqlparameter|dbqueryparameter|urlquery|urlfragment|exceptionmessage|exceptionstacktrace|useragent/;
const denyExact = new Set(['auth', 'body', 'dbstatement', 'dbquerytext']);

function sanitizeTarget(text: string): string {
  const target = text.split(/[?#]/, 1)[0] ?? '';
  return target.replace(/^(https?:\/\/|\/\/)[^/]*@/i, '$1');
}

export function safeName(name: string): string {
  return name.slice(0, 1024).replace(/(?:https?:\/\/|\/)[^\s]*/gi, sanitizeTarget);
}

export function safeAttributes(input: Attributes = {}): Attributes {
  const result: Attributes = Object.create(null);
  let count = 0;
  for (const key in input) {
    if (!Object.hasOwn(input, key) || key.length > 256) continue;
    const normalized = key.toLowerCase().replace(/[^a-z0-9]/g, '');
    if (!normalized || denied.test(normalized) || denyExact.has(normalized)) continue;
    const value = input[key];
    if (value === undefined) continue;
    result[key] = safeValue(value, /^(url\.|url$|http\.(url|target|route)$)/i.test(key));
    if (++count === 64) break;
  }
  return result;
}

function safeValue(value: AttributeValue, urlKey: boolean): AttributeValue {
  if (typeof value === 'string') return (urlKey ? sanitizeTarget(value.slice(0, 1024)) : safeName(value)).slice(0, 1024);
  if (Array.isArray(value)) {
    return value.slice(0, 32).map(item => typeof item === 'string'
      ? (urlKey ? sanitizeTarget(item.slice(0, 1024)) : safeName(item)).slice(0, 1024)
      : item) as AttributeValue;
  }
  return value;
}

export function sanitizeSpan(span: ReadableSpan): ReadableSpan {
  const spanContext = span.spanContext();
  return {
    kind: span.kind,
    parentSpanContext: span.parentSpanContext,
    startTime: span.startTime,
    endTime: span.endTime,
    duration: span.duration,
    ended: span.ended,
    instrumentationScope: span.instrumentationScope,
    droppedAttributesCount: span.droppedAttributesCount,
    droppedEventsCount: span.droppedEventsCount + Math.max(0, span.events.length - 32),
    droppedLinksCount: span.droppedLinksCount + Math.max(0, span.links.length - 32),
    name: safeName(span.name),
    spanContext: () => spanContext,
    attributes: safeAttributes(span.attributes),
    status: { code: span.status.code },
    // Async external resource detector attributes may still be pending at onEnd.
    // BatchSpanProcessor waits for them on the original resource.
    events: span.events.slice(0, 32).map(event => ({ ...event, name: safeName(event.name), attributes: safeAttributes(event.attributes) })),
    links: span.links.slice(0, 32).map(link => ({ ...link, attributes: safeAttributes(link.attributes) })),
    resource: span.resource,
  };
}

export function sanitizeResource(span: ReadableSpan): ReadableSpan {
  return {
    ...span,
    resource: resourceFromAttributes(safeAttributes(span.resource.attributes), { schemaUrl: span.resource.schemaUrl }),
  };
}
