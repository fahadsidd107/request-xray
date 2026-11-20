# 8. Acknowledge ingestion after commit

**Status:** Accepted on 2026-11-20.

## Decision

A production sink returns `SinkOutcome::Committed` only after the SQLite transaction containing the accepted spans has committed. The collector awaits that result before sending HTTP 200, including when the response reports partial success. An empty export can succeed without a transaction. An entirely invalid batch returns HTTP 400. A failed or timed-out write returns retryable HTTP 503 without a partial-success acknowledgment.

Admission uses a bounded number of in-flight requests and zero internal queue slots. Each admitted request holds its permit through decoding, redaction and the sink result. A full or draining receiver rejects new work before reading its body. A deliberate validation-only run uses a separate result and counter.

## Consequences

Commit latency is part of export latency, and slow writes occupy admission slots. The sink must avoid detached writes and handle cancellation of work that has yet to commit. A timeout, crash or disconnect can hide a commit that already happened. The SQLite writer handles retries by trace and span identity and reports duplicates after commit. Persistence, rollback and restart recovery are covered by the [storage tests](../STORAGE.md#verification).
