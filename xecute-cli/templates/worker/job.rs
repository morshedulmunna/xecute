//! Job payload and handler for the __FEATURE__ worker.

use xecute::jobs::{JobContext, JobError};

/// Payload submitted to the __FEATURE__ background queue.
pub struct __TYPE__Job {
    /// Application record identifier this job should process.
    pub record_id: u64,
}

/// Process one __FEATURE__ job.
pub(crate) async fn handle(job: JobContext<__TYPE__Job>) -> Result<(), JobError> {
    let _ = (job.id, job.attempt, job.payload.record_id);
    Ok(())
}
