use std::time::Duration;

use sqlx::SqliteConnection;

use crate::{now_ns, Repository, StorageError};

#[derive(Clone, Copy)]
pub struct Retention {
    pub max_age: Duration,
    pub max_bytes: u64,
}

impl Default for Retention {
    fn default() -> Self {
        Self {
            max_age: Duration::from_secs(7 * 24 * 60 * 60),
            max_bytes: 500 * 1024 * 1024,
        }
    }
}

#[derive(Debug, Default, Eq, PartialEq)]
pub struct RetentionReport {
    pub deleted_traces: u64,
    pub live_bytes: u64,
}

impl Repository {
    /// Evict whole traces, oldest last ingestion first. Age uses observation
    /// time, so a newly received span with an old source clock survives.
    pub async fn enforce_retention(
        &self,
        retention: Retention,
    ) -> Result<RetentionReport, StorageError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let cutoff = now_ns()
            .saturating_sub(i64::try_from(retention.max_age.as_nanos()).unwrap_or(i64::MAX));
        let mut deleted = sqlx::query("DELETE FROM traces WHERE last_seen_ns < ?")
            .bind(cutoff)
            .execute(&mut *tx)
            .await?
            .rows_affected();
        if deleted != 0 {
            remove_orphan_resources(&mut tx).await?;
        }
        let mut live = live_bytes(&mut tx).await?;
        while live > retention.max_bytes {
            let count = sqlx::query(
                "DELETE FROM traces WHERE trace_id = (
                   SELECT trace_id FROM traces ORDER BY last_seen_ns, trace_id LIMIT 1
                 )",
            )
            .execute(&mut *tx)
            .await?
            .rows_affected();
            if count == 0 {
                break;
            }
            deleted += count;
            remove_orphan_resources(&mut tx).await?;
            live = live_bytes(&mut tx).await?;
        }
        tx.commit().await?;
        Ok(RetentionReport {
            deleted_traces: deleted,
            live_bytes: live,
        })
    }
}

async fn remove_orphan_resources(connection: &mut SqliteConnection) -> sqlx::Result<()> {
    sqlx::query("DELETE FROM resources WHERE NOT EXISTS (SELECT 1 FROM spans WHERE resource_id = resources.id)")
        .execute(connection).await?;
    Ok(())
}

async fn live_bytes(connection: &mut SqliteConnection) -> sqlx::Result<u64> {
    let (size, pages, free): (i64, i64, i64) = sqlx::query_as(
        "SELECT page_size, page_count, freelist_count FROM pragma_page_size(), pragma_page_count(), pragma_freelist_count()"
    ).fetch_one(connection).await?;
    Ok((pages - free) as u64 * size as u64)
}
