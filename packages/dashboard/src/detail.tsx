import { useEffect, useMemo, useRef, useState, type CSSProperties, type KeyboardEvent } from 'react';
import { traceDetails, type Attribute, type Span } from './api';
import { Badge, Icon } from './components';
import { clock, duration, flatten, kind, service, tree, type Row } from './trace-model';

const rowHeight = 41;
const overscan = 6;
type Details = Awaited<ReturnType<typeof traceDetails>>;

export function Detail({ id, setConnection }: { id: string; setConnection: (ready: boolean | null) => void }) {
  const [result, setResult] = useState<Details>();
  const [error, setError] = useState('');
  const [count, setCount] = useState(0);
  const [refresh, setRefresh] = useState(0);
  useEffect(() => {
    const controller = new AbortController();
    setError('');
    setCount(0);
    traceDetails(id, controller.signal, setCount).then(data => {
      setResult(data);
      setConnection(true);
    }).catch((failure: unknown) => {
      if (!controller.signal.aborted) setError(failure instanceof Error ? failure.message : 'Could not load this trace.');
    });
    return () => controller.abort();
  }, [id, refresh, setConnection]);
  const trace = result?.trace;
  return <>
    <a className="back-link" href="#/"><Icon name="back" /> All traces</a>
    <div className="page-heading trace-heading">
      <div><h1>Trace detail</h1><p className="mono trace-full-id">{id}</p></div>
      <button className="button" onClick={() => setRefresh(value => value + 1)}><Icon name="refresh" /> Refresh trace</button>
    </div>
    {error && <p className="notice" role="alert"><Icon name="warning" />{error}</p>}
    {!result && !error && <div className="panel empty" role="status">Loading trace{count ? ` · ${count.toLocaleString()} spans loaded` : '…'}</div>}
    {trace && result && <>
      <dl className="trace-meta panel">
        <div><dt>Duration</dt><dd className="mono" title={`${trace.durationNs} ns`}>{duration(trace.durationNs)}</dd></div>
        <div><dt>Spans</dt><dd>{trace.spanCount.toLocaleString()}</dd></div>
        <div><dt>Services</dt><dd>{trace.serviceCount.toLocaleString()}</dd></div>
        <div><dt>Status</dt><dd><Badge status={trace.status} /></dd></div>
        <div><dt>Last observed</dt><dd><time dateTime={clock(trace.completeness.lastSeenAtUnixNs)}>{clock(trace.completeness.lastSeenAtUnixNs).replace('T', ' ')}</time></dd></div>
      </dl>
      {trace.completeness.missingParentCount > 0 && <p className="notice" data-testid="missing-parent-indicator"><Icon name="warning" />{trace.completeness.missingParentCount} missing {trace.completeness.missingParentCount === 1 ? 'parent' : 'parents'} reported in storage.</p>}
      {(result.cap || trace.spanCount !== result.spans.length) && <p className="notice" role="status"><Icon name="warning" />{result.cap === 'count' ? 'Showing up to 10,000 spans. ' : result.cap === 'payload' ? 'Stopped at the dashboard’s 64 MiB span payload budget. ' : ''}{result.spans.length.toLocaleString()} spans loaded / {trace.spanCount.toLocaleString()} in the trace summary. Refresh to pick up late spans.</p>}
      <Waterfall result={result} />
    </>}
  </>;
}

function Waterfall({ result }: { result: Details }) {
  const partial = Boolean(result.cap) || result.trace.spanCount !== result.spans.length;
  const graph = useMemo(() => tree(result.spans, partial), [result.spans, partial]);
  const [collapsed, setCollapsed] = useState<ReadonlySet<string>>(new Set());
  const [selected, setSelected] = useState(result.spans[0]?.spanId);
  const [zoom, setZoom] = useState(1);
  const [pan, setPan] = useState(0);
  const viewport = useRef<HTMLDivElement>(null);
  const [scroll, setScroll] = useState(0);
  const [height, setHeight] = useState(492);
  const rows = useMemo(() => flatten(graph.roots, collapsed), [graph.roots, collapsed]);
  const picked = graph.nodes.get(selected ?? '') ?? graph.roots[0];
  const origin = BigInt(result.trace.startedAtUnixNs);
  const total = BigInt(result.trace.durationNs) || 1n;
  const range = total / BigInt(zoom) || 1n;
  const start = (total - range) * BigInt(pan) / 1000n;
  const first = Math.max(0, Math.min(rows.length - 1, Math.floor(scroll / rowHeight)) - overscan);
  const end = Math.min(rows.length, first + Math.ceil(height / rowHeight) + overscan * 2);
  const ticks = [0, 1, 2, 3, 4].map(n => duration(start + range * BigInt(n) / 4n));

  useEffect(() => {
    const current = viewport.current;
    if (!current) return;
    const observer = new ResizeObserver(() => setHeight(current.clientHeight));
    observer.observe(current);
    return () => observer.disconnect();
  }, []);

  function toggle(id: string) {
    setCollapsed(items => {
      const edited = new Set(items);
      if (edited.has(id)) edited.delete(id);
      else edited.add(id);
      return edited;
    });
  }

  function keyboard(event: KeyboardEvent<HTMLDivElement>) {
    if (event.target !== event.currentTarget) return;
    const current = Math.max(0, rows.findIndex(row => row.node.span.spanId === picked?.span.spanId));
    if (event.key === 'ArrowLeft' || event.key === 'ArrowRight') {
      const row = rows[current];
      if (row?.node.children.length && collapsed.has(row.node.span.spanId) === (event.key === 'ArrowRight')) toggle(row.node.span.spanId);
      event.preventDefault();
      return;
    }
    const destination = event.key === 'ArrowDown' ? current + 1 : event.key === 'ArrowUp' ? current - 1
      : event.key === 'Home' ? 0 : event.key === 'End' ? rows.length - 1 : null;
    if (destination === null) return;
    event.preventDefault();
    const index = Math.max(0, Math.min(rows.length - 1, destination));
    if (rows[index]) setSelected(rows[index].node.span.spanId);
    const top = index * rowHeight;
    const list = viewport.current;
    if (list && (top < list.scrollTop || top + rowHeight > list.scrollTop + list.clientHeight)) list.scrollTop = top;
  }

  return <div className="detail-grid">
    <section className="panel waterfall">
      <div className="section-heading"><h2>Waterfall</h2><span>{rows.length.toLocaleString()} visible / {result.spans.length.toLocaleString()} loaded</span></div>
      <div className="waterfall-controls">
        <button className="button" disabled={zoom <= 1} aria-label="Zoom out" onClick={() => setZoom(n => n / 2)}>−</button>
        <span className="mono">{zoom}×</span>
        <button className="button" disabled={zoom >= 64 || range <= 1n} aria-label="Zoom in" onClick={() => setZoom(n => n * 2)}>+</button>
        <label className="pan">Pan timeline<input type="range" min="0" max="1000" step="10" value={pan} disabled={zoom === 1} onChange={event => setPan(Number(event.target.value))} /></label>
        <button className="button" disabled={collapsed.size === 0} onClick={() => setCollapsed(new Set())}>Expand all</button>
        <button className="button" onClick={() => { setCollapsed(new Set([...graph.nodes.values()].filter(node => node.children.length).map(node => node.span.spanId))); if (viewport.current) viewport.current.scrollTop = 0; }}>Collapse all</button>
      </div>
      <div className="timeline-head">
        <span>Operation</span>
        <div className="ticks" aria-label="Relative time">{ticks.map((label, index) => <span key={index}>{label}</span>)}</div>
      </div>
      <div className="waterfall-viewport" ref={viewport} tabIndex={0} role="region" aria-label="Span waterfall. Arrow keys select a span." data-testid="span-tree" data-span-count={result.spans.length} onKeyDown={keyboard} onScroll={event => setScroll(event.currentTarget.scrollTop)}>
        <div className="rows" style={{ height: rows.length * rowHeight }}>
          {rows.slice(first, end).map((row, index) => <TimelineRow key={row.node.span.spanId}
            row={row} offset={(first + index) * rowHeight} origin={origin + start} range={range}
            selected={picked?.span.spanId === row.node.span.spanId} collapsed={collapsed.has(row.node.span.spanId)}
            onToggle={toggle} onSelect={setSelected} />)}
        </div>
        {rows.length === 0 && <div className="empty">No spans are currently stored for this trace.</div>}
      </div>
      <p className="waterfall-foot"><Icon name="warning" /> Observed data only. Sampling coverage is unknown.</p>
    </section>
    {picked && <Inspector span={picked.span} gap={picked.gap} />}
  </div>;
}

function TimelineRow({ row, offset, origin, range, selected, collapsed, onToggle, onSelect }: {
  row: Row; offset: number; origin: bigint; range: bigint; selected: boolean; collapsed: boolean;
  onToggle: (id: string) => void; onSelect: (id: string) => void;
}) {
  const { span, gap, children } = row.node;
  const from = BigInt(span.startedAtUnixNs) - origin;
  const to = BigInt(span.endedAtUnixNs) - origin;
  const left = percent(from, range);
  const width = percent(to, range) - left;
  const visible = to >= 0n && from <= range;
  const error = span.status?.code === 2;
  return <div className={`span-row${selected ? ' selected' : ''}`} style={{ top: offset, '--depth': Math.min(8, row.depth) } as CSSProperties}
    data-testid="span-row" data-span-id={span.spanId}>
    <div className="operation">
      {children.length > 0 ? <button className="toggle" aria-label={`${collapsed ? 'Expand' : 'Collapse'} ${span.name}`} aria-expanded={!collapsed} onClick={() => onToggle(span.spanId)}><Icon name={collapsed ? 'right' : 'down'} /></button> : <span className="toggle-placeholder" />}
      <button className="span-label" type="button" aria-pressed={selected} onClick={() => onSelect(span.spanId)} title={span.name}>
        <span>{span.name}</span><small>{service(span)}{gap && <span className="gap"> · {gap}</span>}</small>
      </button>
    </div>
    <button className="chart" type="button" aria-label={`Inspect ${span.name}, ${span.durationNs} ns`} title={`${span.name} · ${span.durationNs} ns`} onClick={() => onSelect(span.spanId)}>
      {visible && <span className={`bar${error ? ' bar-error' : ''}`} style={{ left: `${left}%`, width: `${width}%` }} />}
    </button>
  </div>;
}

function percent(value: bigint, range: bigint) {
  const bounded = value <= 0n ? 0n : value >= range ? range : value;
  return Number(bounded * 100_000n / range) / 1000;
}

function Inspector({ span, gap }: { span: Span; gap?: string }) {
  const status = span.status?.code === 2 ? 'error' : span.status?.code === 1 ? 'ok' : 'unset';
  return <aside className="panel inspector" data-testid="selected-span-inspector" aria-labelledby="inspector-title">
    <div className="section-heading"><h2 id="inspector-title">Span inspector</h2><Badge status={status} /></div>
    <div className="inspector-body">
      <h3>{span.name}</h3>
      <p className="inspector-service">{service(span)} · {kind(span)}</p>
      {gap && <p className="gap"><Icon name="warning" /> {gap}</p>}
      {span.status?.message && <p className="notice" role="note">{span.status.message}</p>}
      {span.status && ![0, 1, 2].includes(span.status.code) && <p>Status code {span.status.code}</p>}
      <dl className="span-meta">
        <div><dt>Span ID</dt><dd className="mono">{span.spanId}</dd></div>
        <div><dt>Parent span</dt><dd className="mono">{span.parentSpanId ?? 'None'}</dd></div>
        {span.route && <div><dt>Route</dt><dd>{span.route}</dd></div>}
        <div><dt>Start (Unix ns)</dt><dd className="mono" data-testid="span-start-ns" title={clock(span.startedAtUnixNs)}>{span.startedAtUnixNs}</dd></div>
        <div><dt>End (Unix ns)</dt><dd className="mono" title={clock(span.endedAtUnixNs)}>{span.endedAtUnixNs}</dd></div>
        <div><dt>Duration (ns)</dt><dd className="mono" data-testid="span-duration-ns">{span.durationNs}</dd></div>
      </dl>
      {!!(span.droppedAttributesCount || span.droppedEventsCount || span.droppedLinksCount || span.resource?.droppedAttributesCount || span.scope?.droppedAttributesCount) && <p className="notice"><Icon name="warning" /> Dropped by the exporter: {span.droppedAttributesCount} span attributes, {span.droppedEventsCount} events, {span.droppedLinksCount} links, {span.resource?.droppedAttributesCount ?? 0} resource attributes, {span.scope?.droppedAttributesCount ?? 0} scope attributes.</p>}
      <details open><summary>Attributes <span>{span.attributes.length}</span></summary><Attributes entries={span.attributes} /></details>
      <details><summary>Resource and scope</summary>
        <p>Resource schema: {span.resourceSchemaUrl || 'Unavailable'}</p><Attributes entries={span.resource?.attributes ?? []} />
        <p>Scope: {span.scope?.name || 'Unavailable'} {span.scope?.version}</p>
        <p>Scope schema: {span.scopeSchemaUrl || 'Unavailable'}</p><Attributes entries={span.scope?.attributes ?? []} />
      </details>
      <details><summary>Events <span>{span.events.length}</span></summary>
        {span.events.length ? <ul className="metadata-list">{span.events.map((event, index) => <li key={index}><strong>{event.name}</strong><small className="mono">{event.timeUnixNs} ns</small><Attributes entries={event.attributes} />{event.droppedAttributesCount > 0 && <p className="gap">{event.droppedAttributesCount} attributes dropped</p>}</li>)}</ul> : <p>No events recorded.</p>}
      </details>
      <details><summary>Links <span>{span.links.length}</span></summary>
        {span.links.length ? <ul className="metadata-list">{span.links.map((link, index) => <li key={index}><a href={`#/traces/${link.traceId}`}>{link.traceId}</a><small className="mono">{link.spanId}</small><Attributes entries={link.attributes} />{link.droppedAttributesCount > 0 && <p className="gap">{link.droppedAttributesCount} attributes dropped</p>}</li>)}</ul> : <p>No links recorded.</p>}
      </details>
    </div>
  </aside>;
}

function Attributes({ entries }: { entries: Attribute[] }) {
  return entries.length ? <dl className="attributes">
    {entries.map(({ key, value }, index) => <div key={index}><dt>{key}</dt><dd>{value === null || value.type === 'empty' ? '—' : typeof value.value === 'object' ? JSON.stringify(value.value) : String(value.value)}</dd></div>)}
  </dl> : <p>No attributes recorded.</p>;
}
