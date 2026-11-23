ALTER TABLE spans ADD COLUMN route TEXT;
ALTER TABLE spans ADD COLUMN status_code INTEGER;
UPDATE spans SET status_code = json_extract(payload, '$.status.code'),
    route = (
        SELECT json_extract(a.value, '$.value.String')
        FROM json_each(spans.payload, '$.attributes') AS a
        WHERE json_extract(a.value, '$.key') = 'http.route'
          AND json_type(a.value, '$.value.String') = 'text'
          AND json_extract(a.value, '$.value.String') <> ''
        ORDER BY a.key LIMIT 1
    );
CREATE INDEX spans_by_route ON spans(route, trace_id) WHERE route IS NOT NULL;
CREATE INDEX spans_by_trace_status ON spans(trace_id, status_code);
