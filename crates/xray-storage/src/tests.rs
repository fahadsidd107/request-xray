use super::*;
use xray_core::{
    Attribute, AttributeValue, Resource, ResourceSpans, ScopeSpans, Span, SpanId, SpanInterval,
    SpanKind, SpanStatus, StatusCode, TraceId, UnixNanos,
};

fn trace(id: u8) -> TraceId {
    TraceId::try_from([id; 16]).unwrap()
}

fn span_id(id: u8) -> SpanId {
    SpanId::try_from([id; 8]).unwrap()
}

fn span(trace_id: u8, id: u8, start: i64) -> Span {
    Span {
        trace_id: trace(trace_id),
        span_id: span_id(id),
        parent_span_id: None,
        trace_state: String::new(),
        flags: 1,
        name: format!("span-{id}"),
        kind: SpanKind::SERVER,
        interval: SpanInterval::new(
            UnixNanos::try_from(start).unwrap(),
            UnixNanos::try_from(start + 1).unwrap(),
        )
        .unwrap(),
        attributes: vec![Attribute {
            key: "sample".into(),
            value: Some(AttributeValue::Double(f64::from_bits(
                0x7ff8_0000_0000_0001,
            ))),
        }],
        dropped_attributes_count: 0,
        events: vec![],
        dropped_events_count: 0,
        links: vec![],
        dropped_links_count: 0,
        status: None,
    }
}

fn batch(service: &str, spans: Vec<Span>) -> Vec<ResourceSpans> {
    vec![ResourceSpans {
        resource: Some(Resource {
            attributes: vec![Attribute {
                key: "service.name".into(),
                value: Some(AttributeValue::String(service.into())),
            }],
            dropped_attributes_count: 0,
            entity_refs: vec![],
        }),
        schema_url: "resource-schema".into(),
        scope_spans: vec![ScopeSpans {
            scope: None,
            schema_url: "scope-schema".into(),
            spans,
        }],
    }]
}

#[tokio::test]
async fn reopen_preserves_spans_and_late_parent_updates_summary() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("traces.sqlite");
    let repo = Repository::open(&path).await.unwrap();
    let mut child = span(1, 2, 20);
    child.parent_span_id = Some(span_id(1));
    child.status = Some(SpanStatus {
        code: StatusCode::ERROR,
        message: String::new(),
    });
    repo.write_batch(batch("worker", vec![child]))
        .await
        .unwrap();
    let initial = repo.get_trace(trace(1)).await.unwrap().unwrap();
    assert_eq!(initial.span_count, 1);
    assert!(initial.has_missing_parents());
    assert!(initial.has_error);

    repo.close().await;
    let repo = Repository::open(&path).await.unwrap();
    repo.write_batch(batch("api", vec![span(1, 1, 10)]))
        .await
        .unwrap();
    let summary = repo.get_trace(trace(1)).await.unwrap().unwrap();
    assert_eq!(
        (
            summary.start_ns,
            summary.end_ns,
            summary.span_count,
            summary.missing_parent_count
        ),
        (10, 21, 2, 0)
    );
    assert_eq!(summary.first_seen_ns, initial.first_seen_ns);
    assert!(summary.last_seen_ns >= initial.last_seen_ns);
    let first = repo.list_spans(trace(1), None, 1).await.unwrap().remove(0);
    assert_eq!(first.span.span_id, span_id(1));
    assert_eq!(first.scope_schema_url, "scope-schema");
    assert!(
        matches!(first.span.attributes[0].value, Some(AttributeValue::Double(value)) if value.to_bits() == 0x7ff8_0000_0000_0001)
    );
    let next = repo
        .list_spans(trace(1), Some((10, span_id(1))), 1)
        .await
        .unwrap();
    assert_eq!(next[0].span.span_id, span_id(2));
    let page = repo
        .list_traces(
            TraceFilter {
                service: Some("worker"),
                has_error: Some(true),
                since_ns: Some(10),
                until_ns: Some(11),
                ..Default::default()
            },
            10,
        )
        .await
        .unwrap();
    assert_eq!(page.traces, vec![summary]);
    assert!(repo
        .list_traces(
            TraceFilter {
                service: Some("worker' OR 1=1 --"),
                ..Default::default()
            },
            10
        )
        .await
        .unwrap()
        .traces
        .is_empty());
}

#[tokio::test]
async fn cursor_tie_break_and_first_writer_wins_across_connections() {
    let dir = tempfile::tempdir().unwrap();
    let repo = Repository::open(dir.path().join("db")).await.unwrap();
    let first = batch("api", vec![span(1, 1, 20), span(2, 2, 20), span(3, 3, 5)]);
    let (left, right) = tokio::join!(repo.write_batch(first.clone()), repo.write_batch(first));
    assert_eq!(
        left.as_ref().unwrap().inserted_spans + right.as_ref().unwrap().inserted_spans,
        3
    );
    assert_eq!(
        left.unwrap().duplicate_spans + right.unwrap().duplicate_spans,
        3
    );
    let conflict = repo
        .write_batch(batch("different", vec![span(1, 1, 50)]))
        .await
        .unwrap();
    assert_eq!(
        conflict,
        WriteReport {
            conflicting_spans: 1,
            ..Default::default()
        }
    );

    let page = repo.list_traces(TraceFilter::default(), 1).await.unwrap();
    assert_eq!(page.traces[0].trace_id, trace(2));
    let next = repo
        .list_traces(
            TraceFilter {
                after: page.next,
                ..Default::default()
            },
            10,
        )
        .await
        .unwrap();
    assert_eq!(
        next.traces
            .iter()
            .map(|row| row.trace_id)
            .collect::<Vec<_>>(),
        [trace(1), trace(3)]
    );
    assert_eq!(next.traces[0].start_ns, 20);
    assert!(matches!(
        repo.list_traces(TraceFilter::default(), 0).await,
        Err(StorageError::InvalidPageSize)
    ));
}

#[tokio::test]
async fn retention_evicts_oldest_whole_traces_and_cleans_resources() {
    let dir = tempfile::tempdir().unwrap();
    let repo = Repository::open(dir.path().join("db")).await.unwrap();
    let mut old = span(1, 1, 1);
    old.attributes.push(Attribute {
        key: "large".into(),
        value: Some(AttributeValue::String("x".repeat(128 * 1024))),
    });
    repo.write_batch(batch("old", vec![old, span(1, 3, 2)]))
        .await
        .unwrap();
    repo.write_batch(batch("recent", vec![span(2, 2, 1)]))
        .await
        .unwrap();
    sqlx::query("UPDATE traces SET last_seen_ns = last_seen_ns - 1000000000 WHERE trace_id = ?")
        .bind(trace(1).as_bytes().as_slice())
        .execute(&repo.pool)
        .await
        .unwrap();
    let report = repo
        .enforce_retention(Retention {
            max_bytes: 96 * 1024,
            ..Retention::default()
        })
        .await
        .unwrap();
    assert_eq!(report.deleted_traces, 1);
    assert!(report.live_bytes <= 96 * 1024);
    assert!(repo.get_trace(trace(1)).await.unwrap().is_none());
    assert!(repo
        .list_spans(trace(1), None, 10)
        .await
        .unwrap()
        .is_empty());
    assert!(repo.get_trace(trace(2)).await.unwrap().is_some());
    sqlx::query("UPDATE traces SET last_seen_ns = 0")
        .execute(&repo.pool)
        .await
        .unwrap();
    let report = repo.enforce_retention(Retention::default()).await.unwrap();
    assert_eq!(report.deleted_traces, 1);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM resources")
            .fetch_one(&repo.pool)
            .await
            .unwrap(),
        0
    );
}

#[tokio::test]
async fn wal_readers_observe_committed_snapshot_and_failed_batch_rolls_back() {
    let dir = tempfile::tempdir().unwrap();
    let repo = Repository::open(dir.path().join("db")).await.unwrap();
    let mode: String = sqlx::query_scalar("PRAGMA journal_mode")
        .fetch_one(&repo.pool)
        .await
        .unwrap();
    assert_eq!(mode, "wal");
    let foreign_keys: i64 = sqlx::query_scalar("PRAGMA foreign_keys")
        .fetch_one(&repo.pool)
        .await
        .unwrap();
    assert_eq!(foreign_keys, 1);

    let mut pending = repo.pool.begin_with("BEGIN IMMEDIATE").await.unwrap();
    sqlx::query(
        "INSERT INTO traces(trace_id,start_ns,end_ns,first_seen_ns,last_seen_ns) VALUES(?,0,0,0,0)",
    )
    .bind(trace(1).as_bytes().as_slice())
    .execute(&mut *pending)
    .await
    .unwrap();
    assert!(repo.get_trace(trace(1)).await.unwrap().is_none());
    pending.commit().await.unwrap();
    assert!(repo.get_trace(trace(1)).await.unwrap().is_some());

    sqlx::query("CREATE TRIGGER fail_write BEFORE INSERT ON spans WHEN NEW.start_ns = 99 BEGIN SELECT RAISE(ABORT,'test failure'); END")
        .execute(&repo.pool).await.unwrap();
    assert!(repo
        .write_batch(batch("api", vec![span(2, 1, 98), span(2, 2, 99)]))
        .await
        .is_err());
    assert!(repo.get_trace(trace(2)).await.unwrap().is_none());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM resources")
            .fetch_one(&repo.pool)
            .await
            .unwrap(),
        0
    );
}

#[tokio::test]
async fn sqlite_full_preserves_previously_committed_rows() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("db");
    Repository::open(&path).await.unwrap().close().await;
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(path)
                .foreign_keys(true),
        )
        .await
        .unwrap();
    let repo = Repository { pool };
    repo.write_batch(batch("api", vec![span(1, 1, 1)]))
        .await
        .unwrap();
    let pages: i64 = sqlx::query_scalar("PRAGMA page_count")
        .fetch_one(&repo.pool)
        .await
        .unwrap();
    sqlx::query(&format!("PRAGMA max_page_count = {pages}"))
        .execute(&repo.pool)
        .await
        .unwrap();
    let mut large = span(2, 2, 2);
    large.attributes.push(Attribute {
        key: "large".into(),
        value: Some(AttributeValue::String("x".repeat(256 * 1024))),
    });
    let Err(StorageError::Sql(sqlx::Error::Database(error))) = repo
        .write_batch(batch("api", vec![span(2, 1, 1), large]))
        .await
    else {
        panic!("expected SQLite disk-full error");
    };
    assert_eq!(error.code().as_deref(), Some("13"));
    assert!(repo.get_trace(trace(2)).await.unwrap().is_none());
    assert_eq!(
        repo.get_trace(trace(1)).await.unwrap().unwrap().span_count,
        1
    );
}
