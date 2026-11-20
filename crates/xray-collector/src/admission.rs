use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use tokio::sync::Semaphore;

use crate::Config;

/// Admission settings for requests to the OTLP listener.
#[derive(Clone)]
pub struct Admission {
    pub max_in_flight: usize,
    pub body_timeout: Duration,
    pub sink_timeout: Duration,
}

impl Default for Admission {
    fn default() -> Self {
        Self {
            max_in_flight: 8,
            body_timeout: Duration::from_secs(10),
            sink_timeout: Duration::from_secs(10),
        }
    }
}

/// Aggregate counters for POST /v1/traces. They contain no request data or client identifiers.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Diagnostics {
    pub accepted_spans: u64,
    pub rejected_spans: u64,
    pub redacted_spans: u64,
    pub redacted_fields: u64,
    pub duplicate_spans: u64,
    pub validation_only_spans: u64,
    pub size_errors: u64,
    pub invalid_payloads: u64,
    pub admission_rejections: u64,
    pub shutdown_rejections: u64,
    pub body_timeouts: u64,
    pub write_failures: u64,
    pub write_timeouts: u64,
}

impl Diagnostics {
    pub const SOURCE_ENDPOINT: &'static str = "/v1/traces";
}

/// A sink reports commitment only after its durable transaction succeeds.
/// Validation-only mode is a separate outcome for local testing.
pub enum SinkOutcome {
    Committed { duplicate_spans: usize },
    ValidationOnly,
}

/// Shared admission and diagnostic state for one receiver.
#[derive(Clone)]
pub struct Ingestion {
    pub(crate) config: Arc<Config>,
    pub(crate) slots: Arc<Semaphore>,
    diagnostics: Arc<Mutex<Diagnostics>>,
}

impl Ingestion {
    pub fn new(config: Config) -> Self {
        let max_in_flight = config.admission.max_in_flight.min(Semaphore::MAX_PERMITS);
        Self {
            config: Arc::new(config),
            slots: Arc::new(Semaphore::new(max_in_flight)),
            diagnostics: Arc::new(Mutex::new(Diagnostics::default())),
        }
    }

    /// Stop admitting new batches while allowing admitted requests to finish.
    pub fn begin_shutdown(&self) {
        self.slots.close();
    }

    pub fn diagnostics(&self) -> Diagnostics {
        self.diagnostics.lock().expect("diagnostics lock").clone()
    }

    pub(crate) fn record(&self, update: impl FnOnce(&mut Diagnostics)) {
        update(&mut self.diagnostics.lock().expect("diagnostics lock"));
    }
}
