use sqlx::{FromRow, QueryBuilder, Sqlite};
use xray_core::{Resource, Scope, Span, SpanId, TraceId};

use crate::{Repository, StorageError};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TraceCursor {
    pub start_ns: i64,
    pub trace_id: TraceId,
}

#[derive(Default)]
pub struct TraceFilter<'a> {
    pub service: Option<&'a str>,
    pub has_error: Option<bool>,
    pub since_ns: Option<i64>,
    pub until_ns: Option<i64>,
    pub after: Option<TraceCursor>,
}

#[derive(Debug, Eq, PartialEq)]
pub struct TraceSummary {
    pub trace_id: TraceId,
    pub start_ns: i64,
    pub end_ns: i64,
    pub first_seen_ns: i64,
    pub last_seen_ns: i64,
    pub span_count: i64,
    pub missing_parent_count: i64,
    pub has_error: bool,
}

impl TraceSummary {
    pub fn has_missing_parents(&self) -> bool {
        self.missing_parent_count != 0
    }
}

pub struct TracePage {
    pub traces: Vec<TraceSummary>,
    pub next: Option<TraceCursor>,
}

pub struct StoredSpan {
    pub resource: Option<Resource>,
    pub resource_schema_url: String,
    pub scope: Option<Scope>,
    pub scope_schema_url: String,
    pub span: Span,
}

#[derive(FromRow)]
struct TraceRow {
    trace_id: Vec<u8>,
    start_ns: i64,
    end_ns: i64,
    first_seen_ns: i64,
    last_seen_ns: i64,
    span_count: i64,
    missing_parent_count: i64,
    has_error: bool,
}

impl TryFrom<TraceRow> for TraceSummary {
    type Error = StorageError;

    fn try_from(row: TraceRow) -> Result<Self, Self::Error> {
        Ok(Self {
            trace_id: TraceId::try_from(row.trace_id.as_slice())
                .map_err(StorageError::InvalidId)?,
            start_ns: row.start_ns,
            end_ns: row.end_ns,
            first_seen_ns: row.first_seen_ns,
            last_seen_ns: row.last_seen_ns,
            span_count: row.span_count,
            missing_parent_count: row.missing_parent_count,
            has_error: row.has_error,
        })
    }
}

#[derive(FromRow)]
struct SpanRow {
    resource: String,
    resource_schema_url: String,
    scope: String,
    scope_schema_url: String,
    span: String,
}

impl Repository {
    pub async fn get_trace(&self, id: TraceId) -> Result<Option<TraceSummary>, StorageError> {
        let row = sqlx::query_as::<_, TraceRow>("SELECT * FROM traces WHERE trace_id = ?")
            .bind(id.as_bytes().as_slice())
            .fetch_optional(&self.pool)
            .await?;
        row.map(TraceSummary::try_from).transpose()
    }

    /// Filter by inclusive lower and exclusive upper trace start time.
    pub async fn list_traces(
        &self,
        filter: TraceFilter<'_>,
        limit: u32,
    ) -> Result<TracePage, StorageError> {
        validate_limit(limit)?;
        let mut query = QueryBuilder::<Sqlite>::new("SELECT * FROM traces WHERE 1=1");
        if let Some(service) = filter.service {
            query.push(" AND EXISTS (SELECT 1 FROM resources r JOIN spans s ON s.resource_id = r.id WHERE s.trace_id = traces.trace_id AND r.service_name = ")
                .push_bind(service).push(")");
        }
        if let Some(has_error) = filter.has_error {
            query.push(" AND has_error = ").push_bind(has_error);
        }
        if let Some(since) = filter.since_ns {
            query.push(" AND start_ns >= ").push_bind(since);
        }
        if let Some(until) = filter.until_ns {
            query.push(" AND start_ns < ").push_bind(until);
        }
        if let Some(after) = filter.after {
            query
                .push(" AND (start_ns, trace_id) < (")
                .push_bind(after.start_ns)
                .push(", ")
                .push_bind(after.trace_id.as_bytes().to_vec())
                .push(")");
        }
        query
            .push(" ORDER BY start_ns DESC, trace_id DESC LIMIT ")
            .push_bind(limit + 1);
        let mut rows = query
            .build_query_as::<TraceRow>()
            .fetch_all(&self.pool)
            .await?;
        let next = if rows.len() > limit as usize {
            rows.pop();
            let row = &rows[rows.len() - 1];
            Some(TraceCursor {
                start_ns: row.start_ns,
                trace_id: TraceId::try_from(row.trace_id.as_slice())
                    .map_err(StorageError::InvalidId)?,
            })
        } else {
            None
        };
        let traces = rows
            .into_iter()
            .map(TraceSummary::try_from)
            .collect::<Result<_, _>>()?;
        Ok(TracePage { traces, next })
    }

    /// Return spans ordered by start time and span ID. Pass the last span's
    /// start and ID as the next exclusive cursor.
    pub async fn list_spans(
        &self,
        trace: TraceId,
        after: Option<(i64, SpanId)>,
        limit: u32,
    ) -> Result<Vec<StoredSpan>, StorageError> {
        validate_limit(limit)?;
        let mut query = QueryBuilder::<Sqlite>::new(
            "SELECT r.payload AS resource, r.schema_url AS resource_schema_url,
             s.scope_payload AS scope, s.scope_schema_url, s.payload AS span
             FROM spans s JOIN resources r ON r.id = s.resource_id WHERE s.trace_id = ",
        );
        query.push_bind(trace.as_bytes().as_slice());
        if let Some((start, id)) = after {
            query
                .push(" AND (s.start_ns, s.span_id) > (")
                .push_bind(start)
                .push(", ")
                .push_bind(id.as_bytes().to_vec())
                .push(")");
        }
        query
            .push(" ORDER BY s.start_ns, s.span_id LIMIT ")
            .push_bind(limit);
        query
            .build_query_as::<SpanRow>()
            .fetch_all(&self.pool)
            .await?
            .into_iter()
            .map(|row| {
                Ok(StoredSpan {
                    resource: serde_json::from_str(&row.resource)?,
                    resource_schema_url: row.resource_schema_url,
                    scope: serde_json::from_str(&row.scope)?,
                    scope_schema_url: row.scope_schema_url,
                    span: serde_json::from_str(&row.span)?,
                })
            })
            .collect()
    }
}

fn validate_limit(limit: u32) -> Result<(), StorageError> {
    if (1..=100).contains(&limit) {
        Ok(())
    } else {
        Err(StorageError::InvalidPageSize)
    }
}
