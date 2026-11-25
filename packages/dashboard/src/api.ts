import { createQueryClient, type components, type operations } from '@request-xray/query-client';

type Schema = components['schemas'];
export type Trace = Schema['Trace'];
export type Span = Schema['Span'];
export type Attribute = Schema['Attribute'];
export type Filters = NonNullable<operations['listTraces']['parameters']['query']>;

const api = createQueryClient(window.location.origin);
const maxSpans = 10_000;
const maxSpanPayloadBytes = 64 * 1024 * 1024;

interface Reply<T> {
  data?: T;
  error?: Schema['Error'];
  response: Response;
}

function data<T>(reply: Reply<T>): T {
  if (reply.data !== undefined) return reply.data;
  const detail = reply.error?.error;
  throw new Error(detail ? `${detail.message} (${detail.requestId})` : `Query API returned HTTP ${reply.response.status}.`);
}

export async function overview(signal: AbortSignal) {
  const [health, stats, services] = await Promise.all([
    api.GET('/api/v1/health', { signal }),
    api.GET('/api/v1/stats', { signal }),
    api.GET('/api/v1/services', { params: { query: { limit: 20 } }, signal }),
  ]);
  return { health: data(health), stats: data(stats), services: data(services) };
}

export function traces(filters: Filters, cursor: string | undefined, signal: AbortSignal) {
  return api.GET('/api/v1/traces', { params: { query: { ...filters, cursor, limit: 25 } }, signal }).then(data);
}

export async function traceDetails(id: string, signal: AbortSignal, progress: (count: number) => void) {
  const path = { id };
  const trace = data(await api.GET('/api/v1/traces/{id}', { params: { path }, signal }));
  const items = new Map<string, Span>();
  const seenCursors = new Set<string>();
  let cursor: string | undefined;
  let limit = 100;
  let payloadBytes = 0;
  do {
    const reply = await api.GET('/api/v1/traces/{id}/spans', {
      params: { path, query: { cursor, limit } }, signal,
    });
    if (reply.response.status === 413 && limit > 1) {
      limit = Math.max(1, Math.floor(limit / 2));
      continue;
    }
    const page = data(reply);
    const length = Number(reply.response.headers.get('content-length'));
    payloadBytes += length > 0 ? length : new Blob([JSON.stringify(page)]).size;
    if (payloadBytes > maxSpanPayloadBytes) {
      return { trace, spans: [...items.values()], cap: 'payload' as const };
    }
    for (const span of page.items) items.set(span.spanId, span);
    progress(items.size);
    if (!page.nextCursor) return { trace, spans: [...items.values()], cap: null };
    if (seenCursors.has(page.nextCursor)) throw new Error('The query API repeated a span cursor. Refresh the trace to try again.');
    seenCursors.add(page.nextCursor);
    cursor = page.nextCursor;
    limit = Math.min(limit, maxSpans - items.size);
  } while (items.size < maxSpans);
  return { trace, spans: [...items.values()], cap: 'count' as const };
}
