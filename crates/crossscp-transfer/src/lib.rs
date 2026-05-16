// SPDX-License-Identifier: AGPL-3.0-or-later

//! Transfer queue domain scaffold.
//!
//! The runtime transfer engine will become async after the Tokio/cancellation
//! design is finalized. This crate starts with deterministic queue semantics
//! that can be tested without protocol or UI dependencies.

use std::collections::VecDeque;
use std::fmt;
use std::time::SystemTime;

/// Stable identifier for transfer jobs within a queue instance.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TransferJobId(u64);

impl TransferJobId {
    #[must_use]
    pub fn as_u64(self) -> u64 {
        self.0
    }
}

/// Transfer direction from the point of view of the local user.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransferDirection {
    Upload,
    Download,
    LocalCopy,
}

/// Overwrite behavior for destination conflicts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OverwriteMode {
    Ask,
    Always,
    Never,
    IfNewer,
    Resume,
}

/// User-visible transfer options shared by protocol adapters.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransferOptions {
    pub overwrite_mode: OverwriteMode,
    pub preserve_timestamps: bool,
    pub preserve_permissions: bool,
    pub create_missing_directories: bool,
}

impl Default for TransferOptions {
    fn default() -> Self {
        Self {
            overwrite_mode: OverwriteMode::Ask,
            preserve_timestamps: true,
            preserve_permissions: false,
            create_missing_directories: true,
        }
    }
}

/// Metadata needed to decide conflict behavior without tying this crate to a
/// concrete filesystem implementation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TransferFileMetadata {
    pub size: u64,
    pub modified: Option<SystemTime>,
}

/// Decision produced when evaluating a destination conflict.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OverwriteDecision {
    CopyFromStart,
    ResumeFrom(u64),
    Skip,
    Prompt,
    Unsupported,
}

/// Decide how to handle a destination path using transfer options and metadata.
#[must_use]
pub fn decide_overwrite(
    mode: OverwriteMode,
    source: TransferFileMetadata,
    destination: Option<TransferFileMetadata>,
) -> OverwriteDecision {
    let Some(destination) = destination else {
        return OverwriteDecision::CopyFromStart;
    };

    match mode {
        OverwriteMode::Ask => OverwriteDecision::Prompt,
        OverwriteMode::Always => OverwriteDecision::CopyFromStart,
        OverwriteMode::Never => OverwriteDecision::Skip,
        OverwriteMode::IfNewer => match (source.modified, destination.modified) {
            (Some(source_modified), Some(destination_modified))
                if source_modified > destination_modified =>
            {
                OverwriteDecision::CopyFromStart
            }
            (Some(_), Some(_)) => OverwriteDecision::Skip,
            _ => OverwriteDecision::Unsupported,
        },
        OverwriteMode::Resume => {
            if destination.size < source.size {
                OverwriteDecision::ResumeFrom(destination.size)
            } else if destination.size == source.size {
                OverwriteDecision::Skip
            } else {
                OverwriteDecision::Unsupported
            }
        }
    }
}

/// A queued transfer job.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransferJob {
    pub id: TransferJobId,
    pub direction: TransferDirection,
    pub source: String,
    pub destination: String,
    pub options: TransferOptions,
    pub state: TransferState,
}

/// Lifecycle state for a transfer job.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TransferState {
    Queued,
    Running,
    Paused,
    Completed,
    Failed(String),
    Cancelled,
}

/// Progress event emitted by future transfer executors.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransferProgress {
    pub job_id: TransferJobId,
    pub bytes_done: u64,
    pub bytes_total: Option<u64>,
    pub current_file: Option<String>,
}

/// Deterministic FIFO queue for transfer jobs.
#[derive(Debug, Default)]
pub struct TransferQueue {
    next_id: u64,
    jobs: VecDeque<TransferJob>,
}

impl TransferQueue {
    #[must_use]
    pub fn new() -> Self {
        Self {
            next_id: 1,
            jobs: VecDeque::new(),
        }
    }

    pub fn enqueue(
        &mut self,
        direction: TransferDirection,
        source: impl Into<String>,
        destination: impl Into<String>,
        options: TransferOptions,
    ) -> TransferJobId {
        let id = TransferJobId(self.next_id);
        self.next_id += 1;
        self.jobs.push_back(TransferJob {
            id,
            direction,
            source: source.into(),
            destination: destination.into(),
            options,
            state: TransferState::Queued,
        });
        id
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.jobs.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.jobs.is_empty()
    }

    #[must_use]
    pub fn get(&self, id: TransferJobId) -> Option<&TransferJob> {
        self.jobs.iter().find(|job| job.id == id)
    }

    #[must_use]
    pub fn next_queued(&self) -> Option<&TransferJob> {
        self.jobs
            .iter()
            .find(|job| job.state == TransferState::Queued)
    }

    pub fn start_next(&mut self) -> Option<TransferJobId> {
        let job = self
            .jobs
            .iter_mut()
            .find(|job| job.state == TransferState::Queued)?;
        job.state = TransferState::Running;
        Some(job.id)
    }

    pub fn set_state(
        &mut self,
        id: TransferJobId,
        state: TransferState,
    ) -> Result<(), TransferQueueError> {
        let job = self
            .jobs
            .iter_mut()
            .find(|job| job.id == id)
            .ok_or(TransferQueueError::UnknownJob(id))?;
        job.state = state;
        Ok(())
    }

    pub fn remove_finished(&mut self) -> usize {
        let original_len = self.jobs.len();
        self.jobs.retain(|job| {
            !matches!(
                job.state,
                TransferState::Completed | TransferState::Failed(_) | TransferState::Cancelled
            )
        });
        original_len - self.jobs.len()
    }
}

/// Queue operation errors.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransferQueueError {
    UnknownJob(TransferJobId),
}

impl fmt::Display for TransferQueueError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownJob(id) => write!(formatter, "unknown transfer job {}", id.as_u64()),
        }
    }
}

impl std::error::Error for TransferQueueError {}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use super::{
        decide_overwrite, OverwriteDecision, OverwriteMode, TransferDirection,
        TransferFileMetadata, TransferJobId, TransferOptions, TransferQueue, TransferQueueError,
        TransferState,
    };

    #[test]
    fn queue_assigns_stable_incrementing_ids() {
        let mut queue = TransferQueue::new();

        let first = queue.enqueue(
            TransferDirection::Upload,
            "a",
            "b",
            TransferOptions::default(),
        );
        let second = queue.enqueue(
            TransferDirection::Download,
            "c",
            "d",
            TransferOptions::default(),
        );

        assert_eq!(first.as_u64(), 1);
        assert_eq!(second.as_u64(), 2);
        assert_eq!(queue.len(), 2);
    }

    #[test]
    fn start_next_selects_first_queued_job() {
        let mut queue = TransferQueue::new();
        let first = queue.enqueue(
            TransferDirection::Upload,
            "a",
            "b",
            TransferOptions::default(),
        );
        let second = queue.enqueue(
            TransferDirection::Upload,
            "c",
            "d",
            TransferOptions::default(),
        );

        assert_eq!(queue.start_next(), Some(first));
        assert_eq!(
            queue.get(first).expect("first job exists").state,
            TransferState::Running
        );
        assert_eq!(
            queue.get(second).expect("second job exists").state,
            TransferState::Queued
        );
    }

    #[test]
    fn next_queued_peeks_without_changing_state() {
        let mut queue = TransferQueue::new();
        let first = queue.enqueue(
            TransferDirection::Upload,
            "a",
            "b",
            TransferOptions::default(),
        );

        let next = queue.next_queued().expect("queued job exists");

        assert_eq!(next.id, first);
        assert_eq!(next.state, TransferState::Queued);
    }

    #[test]
    fn remove_finished_keeps_active_jobs() {
        let mut queue = TransferQueue::new();
        let completed = queue.enqueue(
            TransferDirection::Upload,
            "a",
            "b",
            TransferOptions::default(),
        );
        let running = queue.enqueue(
            TransferDirection::Upload,
            "c",
            "d",
            TransferOptions::default(),
        );

        queue
            .set_state(completed, TransferState::Completed)
            .expect("completed job exists");
        queue
            .set_state(running, TransferState::Running)
            .expect("running job exists");

        assert_eq!(queue.remove_finished(), 1);
        assert_eq!(queue.len(), 1);
        assert!(queue.get(running).is_some());
    }

    #[test]
    fn setting_state_for_unknown_job_returns_error() {
        let mut queue = TransferQueue::new();
        let result = queue.set_state(TransferJobId(42), TransferState::Cancelled);

        assert_eq!(
            result,
            Err(TransferQueueError::UnknownJob(TransferJobId(42)))
        );
    }

    #[test]
    fn overwrite_decision_copies_when_destination_is_missing() {
        let source = TransferFileMetadata {
            size: 10,
            modified: None,
        };

        assert_eq!(
            decide_overwrite(OverwriteMode::Never, source, None),
            OverwriteDecision::CopyFromStart
        );
    }

    #[test]
    fn overwrite_decision_supports_if_newer_with_timestamps() {
        let old = SystemTime::UNIX_EPOCH + Duration::from_secs(1);
        let new = SystemTime::UNIX_EPOCH + Duration::from_secs(2);
        let source = TransferFileMetadata {
            size: 10,
            modified: Some(new),
        };
        let destination = TransferFileMetadata {
            size: 5,
            modified: Some(old),
        };

        assert_eq!(
            decide_overwrite(OverwriteMode::IfNewer, source, Some(destination)),
            OverwriteDecision::CopyFromStart
        );

        assert_eq!(
            decide_overwrite(OverwriteMode::IfNewer, destination, Some(source)),
            OverwriteDecision::Skip
        );
    }

    #[test]
    fn overwrite_decision_supports_resume_when_destination_is_shorter() {
        let source = TransferFileMetadata {
            size: 10,
            modified: None,
        };
        let destination = TransferFileMetadata {
            size: 4,
            modified: None,
        };

        assert_eq!(
            decide_overwrite(OverwriteMode::Resume, source, Some(destination)),
            OverwriteDecision::ResumeFrom(4)
        );
    }
}
