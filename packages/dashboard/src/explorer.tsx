import { useEffect, useState, type FormEvent } from 'react';
import { overview, traces, type Filters } from './api';
import { Badge, Icon } from './components';
import { clock, duration } from './trace-model';

type Overview = Awaited<ReturnType<typeof overview>>;
type Page = Awaited<ReturnType<typeof traces>>;
const shortDate = new Intl.DateTimeFormat(undefined, {
  month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit', second: '2-digit', hour12: false,
});

function message(error: unknown) {
  return error instanceof Error ? error.message : 'The local API could not be reached.';
}

export function Explorer({ setConnection }: { setConnection: (ready: boolean | null) => void }) {
  const [summary, setSummary] = useState<Overview>();
  const [summaryError, setSummaryError] = useState('');
  const [filters, setFilters] = useState<Filters>({});
  const [formVersion, setFormVersion] = useState(0);
  const [cursors, setCursors] = useState<(string | undefined)[]>([undefined]);
  const [page, setPage] = useState<Page>();
  const [pageError, setPageError] = useState('');
  const [refresh, setRefresh] = useState(0);
  const cursor = cursors[cursors.length - 1];

  useEffect(() => {
    const controller = new AbortController();
    setSummaryError('');
    overview(controller.signal).then(result => {
      setSummary(result);
      setConnection(true);
    }).catch(error => {
      if (controller.signal.aborted) return;
      setSummaryError(message(error));
      setConnection(false);
    });
    return () => controller.abort();
  }, [refresh, setConnection]);

  useEffect(() => {
    const controller = new AbortController();
    setPage(undefined);
    setPageError('');
    traces(filters, cursor, controller.signal).then(setPage).catch(error => {
      if (!controller.signal.aborted) setPageError(message(error));
    });
    return () => controller.abort();
  }, [cursor, filters, refresh]);

  function apply(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const data = new FormData(event.currentTarget);
    const value = (key: string) => String(data.get(key) ?? '').trim() || undefined;
    const status = value('status') as Filters['status'];
    setFilters({
      traceId: value('traceId'), service: value('service'), route: value('route'), status,
      sinceNs: value('sinceNs'), untilNs: value('untilNs'), durationMinNs: value('durationMinNs'), durationMaxNs: value('durationMaxNs'),
    });
    setCursors([undefined]);
  }

  function byService(service: string) {
    setFilters({ service });
    setFormVersion(value => value + 1);
    setCursors([undefined]);
  }

  const stats = summary?.stats;
  return <>
    <div className="page-heading">
      <div><h1>Trace explorer</h1><p>Find and inspect requests recorded by your local collector.</p></div>
      <button type="button" className="button" onClick={() => setRefresh(value => value + 1)}><Icon name="refresh" /> Refresh</button>
    </div>
    <section className="metrics" aria-label="Collector overview" aria-busy={!summary && !summaryError}>
      <Metric title="Stored traces" value={stats?.traceCount} detail={stats ? `${stats.spanCount.toLocaleString()} spans persisted` : 'SQLite repository'} />
      <Metric title="Observed services" value={stats?.serviceCount} detail="Across persisted spans" />
      <Metric title="Error traces" value={stats?.errorTraceCount} detail="At least one error span" danger />
      <div className="metric">
        <span className="metric-label">Collector</span>
        <strong className="health-value"><span className={`dot ${summaryError ? 'dot-error' : summary ? 'dot-ok' : ''}`} />{summaryError ? 'Unavailable' : summary ? 'Connected' : 'Connecting…'}</strong>
        <span className="metric-detail">{summary && !summaryError ? `Query API ${summary.health.version}` : 'Local query API'}</span>
      </div>
    </section>
    {summaryError && <p className="notice" role="alert"><Icon name="warning" /><span><strong>Collector unavailable.</strong> Start <code>request-xray dev</code> and refresh. {summaryError}</span></p>}
    <div className="explorer-grid">
      <section className="panel">
        <div className="section-heading"><h2>Traces</h2><span>{cursors.length > 1 ? `Page ${cursors.length}` : 'Most recent first'}</span></div>
        <form key={formVersion} className="filters" aria-label="Trace filters" onSubmit={apply}>
          <div className="filter-grid">
            <label>Trace ID<input name="traceId" placeholder="32-character trace ID" defaultValue={filters.traceId} maxLength={32} /></label>
            <label>Service<input name="service" placeholder="Any service" defaultValue={filters.service} maxLength={512} /></label>
            <label>Route<input name="route" placeholder="Any route" defaultValue={filters.route} maxLength={512} /></label>
            <label>Status<select name="status" defaultValue={filters.status ?? ''}><option value="">All statuses</option><option value="error">Error</option><option value="ok">OK</option><option value="unset">Unset</option></select></label>
          </div>
          <div className="filter-actions">
            <details>
              <summary>Time and duration bounds</summary>
              <div className="filter-grid extra-filters">
                <label>Starts after or at (Unix ns)<input name="sinceNs" inputMode="numeric" pattern="[0-9]+" maxLength={19} defaultValue={filters.sinceNs} /></label>
                <label>Starts before (Unix ns)<input name="untilNs" inputMode="numeric" pattern="[0-9]+" maxLength={19} defaultValue={filters.untilNs} /></label>
                <label>Minimum duration (ns)<input name="durationMinNs" inputMode="numeric" pattern="[0-9]+" maxLength={19} defaultValue={filters.durationMinNs} /></label>
                <label>Maximum duration (ns)<input name="durationMaxNs" inputMode="numeric" pattern="[0-9]+" maxLength={19} defaultValue={filters.durationMaxNs} /></label>
              </div>
            </details>
            <button className="button primary" type="submit"><Icon name="search" /> Apply filters</button>
          </div>
        </form>
        <div className="table-wrap">
          <table aria-label="Persisted traces" data-testid="trace-table">
            <thead><tr><th scope="col">Trace ID</th><th scope="col">Started</th><th scope="col">Duration</th><th scope="col">Services</th><th scope="col">Status</th></tr></thead>
            <tbody>
              {page?.items.map(trace => <tr key={trace.traceId}>
                <th scope="row"><a className="trace-id" href={`#/traces/${trace.traceId}`}>{trace.traceId}</a><small>{trace.spanCount.toLocaleString()} spans{trace.completeness.missingParentCount ? ` · ${trace.completeness.missingParentCount} missing parents` : ''}</small></th>
                <td><time dateTime={clock(trace.startedAtUnixNs)} title={clock(trace.startedAtUnixNs)}>{shortDate.format(Number(BigInt(trace.startedAtUnixNs) / 1_000_000n))}</time></td>
                <td className="mono" title={`${trace.durationNs} ns`}>{duration(trace.durationNs)}</td>
                <td>{trace.serviceCount}</td>
                <td><Badge status={trace.status} /></td>
              </tr>)}
            </tbody>
          </table>
        </div>
        {!page && !pageError && <div className="empty" role="status">Loading traces…</div>}
        {pageError && <div className="empty error-text" role="alert">{pageError}</div>}
        {page?.items.length === 0 && <div className="empty" data-testid="empty-state">
          <Icon name="traces" />
          <strong>{Object.values(filters).some(Boolean) ? 'No matching traces' : 'No traces yet'}</strong>
          <p>{Object.values(filters).some(Boolean) ? 'Adjust the filters to broaden your search.' : 'Send an instrumented request to the collector, then refresh this view.'}</p>
        </div>}
        <nav className="pagination" aria-label="Trace pages">
          <button className="button" disabled={cursors.length < 2 || !page} onClick={() => setCursors(values => values.slice(0, -1))}>Previous</button>
          <button className="button" disabled={!page?.nextCursor} onClick={() => { if (page?.nextCursor) setCursors(values => values[values.length - 1] === page.nextCursor ? values : [...values, page.nextCursor!]); }}>Next <Icon name="right" /></button>
        </nav>
      </section>
      <aside className="panel services" aria-labelledby="services-title">
        <div className="section-heading"><h2 id="services-title">Services</h2><span>{stats?.serviceCount ?? '—'}</span></div>
        {summary?.services.items.length ? <ul>
          {summary.services.items.map(service => <li key={service.name}><button type="button" onClick={() => byService(service.name)}>
            <span className="service-symbol"><Icon name="box" /></span>
            <span className="service-label">{service.name}<small>{service.traceCount} {service.traceCount === 1 ? 'trace' : 'traces'}</small></span>
            <Icon name="right" />
          </button></li>)}
        </ul> : <p className="sidebar-empty">{summary ? 'No services observed yet.' : 'Service data unavailable.'}</p>}
        {summary?.services.nextCursor && <p className="sidebar-empty">Showing the first 20 services. Filter by an exact service name to find more.</p>}
      </aside>
    </div>
  </>;
}

function Metric({ title, value, detail, danger = false }: { title: string; value: number | undefined; detail: string; danger?: boolean }) {
  return <div className="metric"><span className="metric-label">{title}</span><strong className={danger && value ? 'error-text' : undefined}>{value?.toLocaleString() ?? '—'}</strong><span className="metric-detail">{detail}</span></div>;
}
