# SQLite trace storage

The native collector opens `.request-xray/traces.sqlite` by default. Start it with `cargo run --locked -p xray-collector`. The collector applies embedded SQLx migrations at startup, then listens for OTLP/HTTP. New database files are created with owner-only permissions on Unix. The query HTTP API is planned for Step 4. [Collector guide](COLLECTOR.md)

| Environment variable | Default | Meaning |
| --- | --- | --- |
| `REQUEST_XRAY_DB_PATH` | `.request-xray/traces.sqlite` | SQLite database path, including custom directories. |
| `REQUEST_XRAY_RETENTION_AGE_SECONDS` | 604800 | Maximum age of a trace since its most recent newly inserted span. |
| `REQUEST_XRAY_RETENTION_MAX_BYTES` | 524288000 | Target maximum for occupied pages in the main SQLite database. |

Retention runs at startup and every 60 seconds. Age uses collector observation time. Size eviction removes the least recently updated trace first, with trace ID as a stable tie-breaker. Eviction deletes complete trace records and their spans in one transaction, then cleans up unreferenced resources. The target can be exceeded between passes, during WAL growth, or when the target falls below the empty database footprint. Readers can delay WAL checkpointing.

## Writes and recovery

Each redacted batch runs in one `BEGIN IMMEDIATE` transaction. The database uses WAL, full synchronous writes, a two-second SQLite busy timeout, foreign keys, and up to four pooled connections. Readers can observe the last committed state while a writer holds a transaction. A write failure rolls back the batch, and the collector acknowledges valid spans only after commit. [Acknowledgment ADR](adr/0008-acknowledge-ingestion-after-commit.md)

The primary key is `(trace_id, span_id)`. An exact match of the stored canonical span and its resource and scope metadata counts as a duplicate. Attribute order is included in this comparison, and it runs after redaction. A conflicting identity keeps the first committed span and increments the conflict diagnostic. A mixed batch reports conflicts as rejected spans in its OTLP partial-success response. A batch with only invalid or conflicting spans returns HTTP 400. A failed or timed-out write returns retryable HTTP 503.

Span payloads preserve events, links, status codes, trace flags, and typed attributes, including exact double bits. Resource and scope metadata retain their schema URLs. This encoding is internal to persistence. The query API will define its own JSON contract.

## Reads and late arrivals

`Repository` provides `get_trace`, `list_traces` and `list_spans`. Trace lists support exact service-name and error filters, an inclusive lower and exclusive upper bound on the trace start time, and descending keyset pagination by `(start_ns, trace_id)`. Span pages sort by `(start_ns, span_id)` ascending. Pages contain at most 100 records. Late arrivals can change sort keys between requests, so a multirequest listing reflects a changing dataset.

Each successful insert refreshes the affected trace's time range, error flag, span count and missing-parent count. A late parent can resolve a missing-parent observation. A zero missing-parent count describes the observed data and does not establish tracing completeness.

## Verification

Run `cargo test --offline --locked -p xray-storage -p xray-collector`. The storage tests cover fresh migration and reopening, cursor ordering and filters, concurrent deduplication, late parents, retention and cascades, concurrent WAL reads, rollback, and an actual SQLite `SQLITE_FULL` result induced with a database page quota. The collector persistence test sends OTLP over loopback, kills the collector after acknowledgment, restarts it against the same database and verifies the stored rows, redaction, events, links, retry and conflict handling, and late-parent reconciliation.

The live OpenTelemetry JavaScript exporter also sent four spans to the collector with SQLite enabled. All four remained readable after shutdown and restart, and the database passed the foreign-key integrity check.
