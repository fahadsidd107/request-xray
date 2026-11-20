//! SQLite persistence for validated, redacted trace batches.

mod read;
mod retention;
mod write;

pub use read::{StoredSpan, TraceCursor, TraceFilter, TracePage, TraceSummary};
pub use retention::{Retention, RetentionReport};
pub use write::WriteReport;

use std::{
    error::Error,
    fmt,
    path::Path,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use sqlx::{
    migrate::Migrator,
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous},
    SqlitePool,
};

static MIGRATIONS: Migrator = sqlx::migrate!();

#[derive(Debug)]
pub enum StorageError {
    Io(std::io::Error),
    Sql(sqlx::Error),
    Migration(sqlx::migrate::MigrateError),
    Payload(serde_json::Error),
    InvalidId(xray_core::InvalidId),
    InvalidPageSize,
}

impl fmt::Display for StorageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Io(_) => "SQLite file could not be opened",
            Self::Sql(_) => "SQLite operation failed",
            Self::Migration(_) => "SQLite migration failed",
            Self::Payload(_) => "stored telemetry payload could not be processed",
            Self::InvalidId(_) => "invalid stored trace identifier",
            Self::InvalidPageSize => "page size must be between 1 and 100",
        })
    }
}

impl Error for StorageError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Sql(error) => Some(error),
            Self::Migration(error) => Some(error),
            Self::Payload(error) => Some(error),
            Self::InvalidId(error) => Some(error),
            Self::InvalidPageSize => None,
        }
    }
}

impl From<sqlx::Error> for StorageError {
    fn from(error: sqlx::Error) -> Self {
        Self::Sql(error)
    }
}

impl From<serde_json::Error> for StorageError {
    fn from(error: serde_json::Error) -> Self {
        Self::Payload(error)
    }
}

#[derive(Clone)]
pub struct Repository {
    pool: SqlitePool,
}

impl Repository {
    pub async fn open(path: impl AsRef<Path>) -> Result<Self, StorageError> {
        let path = path.as_ref();
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            let path = path.to_path_buf();
            // Match the async filesystem behavior of the database connection.
            tokio::task::spawn_blocking(move || {
                std::fs::OpenOptions::new()
                    .create(true)
                    .truncate(false)
                    .write(true)
                    .mode(0o600)
                    .open(path)
            })
            .await
            .map_err(|error| StorageError::Io(std::io::Error::other(error)))?
            .map_err(StorageError::Io)?;
        }
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .foreign_keys(true)
            .pragma("auto_vacuum", "FULL")
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Full)
            .busy_timeout(Duration::from_secs(2));
        let pool = SqlitePoolOptions::new()
            .max_connections(4)
            .connect_with(options)
            .await?;
        // Files created by older builds can keep their existing vacuum mode.
        MIGRATIONS
            .run(&pool)
            .await
            .map_err(StorageError::Migration)?;
        Ok(Self { pool })
    }

    pub async fn close(&self) {
        self.pool.close().await;
    }
}

fn now_ns() -> i64 {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    i64::try_from(elapsed.as_nanos()).unwrap_or(i64::MAX)
}

#[cfg(test)]
mod tests;
