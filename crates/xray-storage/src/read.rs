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
    pub trace_id: Option<TraceId>,
    pub route: Option<&'a str>,
    pub status: Option<TraceStatus>,
    pub min_duration_ns: Option<i64>,
    pub max_duration_ns: Option<i64>,
    pub service: Option<&'a str>,
    pub has_error: Option<bool>,
    pub since_ns: Option<i64>,
    pub until_ns: Option<i64>,
    pub after: Option<TraceCursor>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TraceStatus {
    Unset,
    Ok,
    Error,
}

#[derive(Debug, FromRow)]
pub struct ServiceSummary {
    pub name: String,
    pub span_count: i64,
    pub trace_count: i64,
}

#[derive(Debug, FromRow)]
pub struct StorageStats {
    pub trace_count: i64,
    pub span_count: i64,
    pub service_count: i64,
    pub error_trace_count: i64,
}

#[derive(Debug, Eq, PartialEq)]
pub struct TraceSummary {
    pub trace_id: TraceId,
    pub start_ns: i64,
    pub end_ns: i64,
    pub first_seen_ns: i64,
    pub last_seen_ns: i64,
    pub span_count: i64,
    pub service_count: i64,
    pub missing_parent_count: i64,
    pub has_error: bool,
    pub status: TraceStatus,
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
    service_count: i64,
    missing_parent_count: i64,
    has_error: bool,
    observed_ok: bool,
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
            service_count: row.service_count,
            missing_parent_count: row.missing_parent_count,
            has_error: row.has_error,
            status: if row.has_error {
                TraceStatus::Error
            } else if row.observed_ok {
                TraceStatus::Ok
            } else {
                TraceStatus::Unset
            },
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

const OBSERVED_OK: &str =
    "EXISTS (SELECT 1 FROM spans WHERE spans.trace_id = traces.trace_id AND status_code = 1)";
const TRACES: &str = "SELECT traces.*, EXISTS (SELECT 1 FROM spans WHERE spans.trace_id = traces.trace_id AND status_code = 1) AS observed_ok,
    (SELECT COUNT(DISTINCT r.service_name) FROM spans s JOIN resources r ON r.id = s.resource_id
     WHERE s.trace_id = traces.trace_id AND r.service_name <> '') AS service_count FROM traces";

impl TryFrom<SpanRow> for StoredSpan {
    type Error = StorageError;
    fn try_from(row: SpanRow) -> Result<Self, Self::Error> {
        Ok(Self {
            resource: serde_json::from_str(&row.resource)?,
            resource_schema_url: row.resource_schema_url,
            scope: serde_json::from_str(&row.scope)?,
            scope_schema_url: row.scope_schema_url,
            span: serde_json::from_str(&row.span)?,
        })
    }
}

impl Repository {
    pub async fn get_trace(&self, id: TraceId) -> Result<Option<TraceSummary>, StorageError> {
        let row = sqlx::query_as::<_, TraceRow>(&format!("{TRACES} WHERE trace_id = ?"))
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
        let mut query = QueryBuilder::<Sqlite>::new(format!("{TRACES} WHERE 1=1"));
        if let Some(id) = filter.trace_id {
            query
                .push(" AND trace_id = ")
                .push_bind(id.as_bytes().to_vec());
        }
        if let Some(route) = filter.route {
            query
                .push(" AND trace_id IN (SELECT trace_id FROM spans WHERE route = ")
                .push_bind(route)
                .push(")");
        }
        if let Some(status) = filter.status {
            match status {
                TraceStatus::Error => {
                    query.push(" AND has_error = 1");
                }
                TraceStatus::Ok => {
                    query.push(" AND has_error = 0 AND ").push(OBSERVED_OK);
                }
                TraceStatus::Unset => {
                    query.push(" AND has_error = 0 AND NOT ").push(OBSERVED_OK);
                }
            }
        }
        if let Some(minimum) = filter.min_duration_ns {
            query.push(" AND end_ns - start_ns >= ").push_bind(minimum);
        }
        if let Some(maximum) = filter.max_duration_ns {
            query.push(" AND end_ns - start_ns <= ").push_bind(maximum);
        }
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

    pub async fn ping(&self) -> Result<(), StorageError> {
        sqlx::query_scalar::<_, i64>("SELECT 1 FROM traces LIMIT 1")
            .fetch_optional(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn stats(&self) -> Result<StorageStats, StorageError> {
        Ok(sqlx::query_as(
            "SELECT COUNT(*) AS trace_count, COALESCE(SUM(span_count), 0) AS span_count,
             COALESCE(SUM(has_error), 0) AS error_trace_count,
             (SELECT COUNT(DISTINCT service_name) FROM resources WHERE service_name <> '') AS service_count FROM traces"
        ).fetch_one(&self.pool).await?)
    }

    pub async fn list_services(
        &self,
        after: Option<&str>,
        limit: u32,
    ) -> Result<(Vec<ServiceSummary>, Option<String>), StorageError> {
        validate_limit(limit)?;
        let mut rows = sqlx::query_as::<_, ServiceSummary>(
            "SELECT r.service_name AS name, COUNT(*) AS span_count, COUNT(DISTINCT s.trace_id) AS trace_count
             FROM resources r JOIN spans s ON s.resource_id = r.id
             WHERE r.service_name <> '' AND (? IS NULL OR r.service_name > ?)
             GROUP BY r.service_name ORDER BY r.service_name LIMIT ?"
        ).bind(after).bind(after).bind(limit + 1).fetch_all(&self.pool).await?;
        let next = if rows.len() > limit as usize {
            rows.pop();
            rows.last().map(|row| row.name.clone())
        } else {
            None
        };
        Ok((rows, next))
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
        span_query(SPAN_COLUMNS, trace, after, limit)
            .build_query_as::<SpanRow>()
            .fetch_all(&self.pool)
            .await?
            .into_iter()
            .map(StoredSpan::try_from)
            .collect()
    }
    /// Read one page and a lookahead marker from a consistent read snapshot.
    pub async fn span_page(
        &self,
        trace: TraceId,
        after: Option<(i64, SpanId)>,
        limit: u32,
        max_stored_bytes: i64,
    ) -> Result<Option<(Vec<StoredSpan>, bool)>, StorageError> {
        validate_limit(limit)?;
        let mut tx = self.pool.begin().await?;
        let exists: bool =
            sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM traces WHERE trace_id = ?)")
                .bind(trace.as_bytes().as_slice())
                .fetch_one(&mut *tx)
                .await?;
        if !exists {
            return Ok(None);
        }
        let sizes: Vec<i64> = span_query(
            "length(CAST(s.payload AS BLOB)) + length(CAST(s.scope_payload AS BLOB)) +
             length(CAST(s.scope_schema_url AS BLOB)) + length(CAST(r.payload AS BLOB)) +
             length(CAST(r.schema_url AS BLOB))",
            trace,
            after,
            limit + 1,
        )
        .build_query_scalar()
        .fetch_all(&mut *tx)
        .await?;
        let mut bytes = 0_i64;
        let mut count = 0;
        for size in sizes.iter().take(limit as usize) {
            if size.saturating_add(bytes) > max_stored_bytes {
                if count == 0 {
                    return Err(StorageError::PageTooLarge);
                }
                break;
            }
            bytes += size;
            count += 1;
        }
        let rows = span_query(SPAN_COLUMNS, trace, after, count)
            .build_query_as::<SpanRow>()
            .fetch_all(&mut *tx)
            .await?
            .into_iter()
            .map(StoredSpan::try_from)
            .collect::<Result<Vec<_>, _>>()?;
        tx.commit().await?;
        let more = rows.len() < sizes.len();
        Ok(Some((rows, more)))
    }
}

const SPAN_COLUMNS: &str = "r.payload AS resource, r.schema_url AS resource_schema_url,
     s.scope_payload AS scope, s.scope_schema_url, s.payload AS span";

fn span_query(
    columns: &'static str,
    trace: TraceId,
    after: Option<(i64, SpanId)>,
    limit: u32,
) -> QueryBuilder<'static, Sqlite> {
    let mut query = QueryBuilder::new("SELECT ");
    query
        .push(columns)
        .push(" FROM spans s JOIN resources r ON r.id = s.resource_id WHERE s.trace_id = ")
        .push_bind(trace.as_bytes().to_vec());
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
}

fn validate_limit(limit: u32) -> Result<(), StorageError> {
    if (1..=100).contains(&limit) {
        Ok(())
    } else {
        Err(StorageError::InvalidPageSize)
    }
}
