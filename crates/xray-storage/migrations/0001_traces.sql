CREATE TABLE resources (
    id INTEGER PRIMARY KEY,
    schema_url TEXT NOT NULL,
    payload TEXT NOT NULL,
    service_name TEXT,
    UNIQUE (schema_url, payload)
);

CREATE TABLE traces (
    trace_id BLOB PRIMARY KEY CHECK (length(trace_id) = 16),
    start_ns INTEGER NOT NULL,
    end_ns INTEGER NOT NULL,
    first_seen_ns INTEGER NOT NULL,
    last_seen_ns INTEGER NOT NULL,
    span_count INTEGER NOT NULL DEFAULT 0,
    missing_parent_count INTEGER NOT NULL DEFAULT 0,
    has_error INTEGER NOT NULL DEFAULT 0
) WITHOUT ROWID;

CREATE TABLE spans (
    trace_id BLOB NOT NULL REFERENCES traces(trace_id) ON DELETE CASCADE,
    span_id BLOB NOT NULL CHECK (length(span_id) = 8),
    parent_span_id BLOB CHECK (parent_span_id IS NULL OR length(parent_span_id) = 8),
    resource_id INTEGER NOT NULL REFERENCES resources(id),
    start_ns INTEGER NOT NULL,
    end_ns INTEGER NOT NULL CHECK (end_ns >= start_ns),
    has_error INTEGER NOT NULL,
    scope_schema_url TEXT NOT NULL,
    scope_payload TEXT NOT NULL,
    payload TEXT NOT NULL,
    PRIMARY KEY (trace_id, span_id)
) WITHOUT ROWID;

CREATE INDEX traces_by_start ON traces(start_ns DESC, trace_id DESC);
CREATE INDEX traces_by_error_start ON traces(has_error, start_ns DESC, trace_id DESC);
CREATE INDEX traces_by_last_seen ON traces(last_seen_ns, trace_id);
CREATE INDEX spans_by_trace_start ON spans(trace_id, start_ns, span_id);
CREATE INDEX spans_by_resource ON spans(resource_id, trace_id);
CREATE INDEX resources_by_service ON resources(service_name);
