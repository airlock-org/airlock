//! Controller observations. Reading a snapshot never grants authority, advances
//! a job, probes a VM, or overwrites the worker's record.

use crate::{
    Error, Result,
    identity::{JobId, ReviewId},
    wire::string_enum,
};
use serde::{Deserialize, Serialize, Serializer, ser::SerializeStruct};
use std::collections::BTreeMap;

string_enum! {
    pub enum JobStatus ("controller job status") {
        Queued => "queued", AwaitingApproval => "awaiting_approval",
        Building => "building", Planned => "planned", Built => "built",
        DeploymentStarted => "deployment_started", Rejected => "rejected",
        Cancelled => "cancelled", ScopeExceeded => "scope_exceeded",
        StaleReview => "stale_review", Failed => "failed",
    }
}

impl JobStatus {
    pub fn is_active(self) -> bool {
        match self {
            Self::Queued | Self::AwaitingApproval | Self::Building => true,
            Self::Planned
            | Self::Built
            | Self::DeploymentStarted
            | Self::Rejected
            | Self::Cancelled
            | Self::ScopeExceeded
            | Self::StaleReview
            | Self::Failed => false,
        }
    }
}

string_enum! {
    pub enum PendingStatus ("pending job status") {
        Queued => "queued", AwaitingApproval => "awaiting_approval",
    }
}

string_enum! {
    pub enum ApprovedStatus ("approved job status") {
        Building => "building", Planned => "planned", Built => "built",
        DeploymentStarted => "deployment_started", ScopeExceeded => "scope_exceeded",
        StaleReview => "stale_review", Failed => "failed",
    }
}

string_enum! {
    pub enum StopReason ("stopped job status") {
        Rejected => "rejected", Cancelled => "cancelled", ScopeExceeded => "scope_exceeded",
        StaleReview => "stale_review", Failed => "failed",
    }
}

impl From<PendingStatus> for JobStatus {
    fn from(value: PendingStatus) -> Self {
        match value {
            PendingStatus::Queued => Self::Queued,
            PendingStatus::AwaitingApproval => Self::AwaitingApproval,
        }
    }
}

impl From<ApprovedStatus> for JobStatus {
    fn from(value: ApprovedStatus) -> Self {
        match value {
            ApprovedStatus::Building => Self::Building,
            ApprovedStatus::Planned => Self::Planned,
            ApprovedStatus::Built => Self::Built,
            ApprovedStatus::DeploymentStarted => Self::DeploymentStarted,
            ApprovedStatus::ScopeExceeded => Self::ScopeExceeded,
            ApprovedStatus::StaleReview => Self::StaleReview,
            ApprovedStatus::Failed => Self::Failed,
        }
    }
}

impl From<StopReason> for JobStatus {
    fn from(value: StopReason) -> Self {
        match value {
            StopReason::Rejected => Self::Rejected,
            StopReason::Cancelled => Self::Cancelled,
            StopReason::ScopeExceeded => Self::ScopeExceeded,
            StopReason::StaleReview => Self::StaleReview,
            StopReason::Failed => Self::Failed,
        }
    }
}

impl StopReason {
    pub fn exit_code(self) -> u8 {
        match self {
            Self::Rejected => 20,
            Self::Cancelled => 21,
            Self::ScopeExceeded => 22,
            Self::StaleReview => 23,
            Self::Failed => 1,
        }
    }
}

impl TryFrom<JobStatus> for StopReason {
    type Error = Error;

    fn try_from(status: JobStatus) -> Result<Self> {
        match status {
            JobStatus::Rejected => Ok(Self::Rejected),
            JobStatus::Cancelled => Ok(Self::Cancelled),
            JobStatus::ScopeExceeded => Ok(Self::ScopeExceeded),
            JobStatus::StaleReview => Ok(Self::StaleReview),
            JobStatus::Failed => Ok(Self::Failed),
            JobStatus::Queued
            | JobStatus::AwaitingApproval
            | JobStatus::Building
            | JobStatus::Planned
            | JobStatus::Built
            | JobStatus::DeploymentStarted => Err(Error::InvalidJob {
                reason: "job status does not describe a stopped request",
            }),
        }
    }
}

string_enum! {
    pub enum StoredDecision ("stored decision") { Approved => "approved" }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct JobWire {
    request_id: JobId,
    status: JobStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    decision: Option<StoredDecision>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    authorization_id: Option<ReviewId>,
    // Request/result extensions remain untrusted observations, not executable
    // intents or evidence that the supplied source was actually authorized.
    #[serde(flatten)]
    details: BTreeMap<String, serde_json::Value>,
}

// A recorded approval always carries its authorization ID and a post-approval
// phase. Pending and stopped jobs cannot accidentally acquire that decision.
#[derive(Clone, Debug)]
enum JobState {
    Queued {
        authorization_id: Option<ReviewId>,
    },
    AwaitingApproval {
        authorization_id: ReviewId,
    },
    Approved {
        authorization_id: ReviewId,
        status: ApprovedStatus,
    },
    Stopped {
        authorization_id: Option<ReviewId>,
        reason: StopReason,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(try_from = "JobWire", into = "JobWire")]
pub struct JobSnapshot {
    request_id: JobId,
    state: JobState,
    details: BTreeMap<String, serde_json::Value>,
}

impl TryFrom<JobWire> for JobSnapshot {
    type Error = Error;

    fn try_from(value: JobWire) -> Result<Self> {
        let state = match value.decision {
            Some(StoredDecision::Approved) => {
                let authorization_id = value.authorization_id.ok_or(Error::InvalidJob {
                    reason: "approved decision is missing its authorization ID",
                })?;
                let status = match value.status {
                    JobStatus::Building => ApprovedStatus::Building,
                    JobStatus::Planned => ApprovedStatus::Planned,
                    JobStatus::Built => ApprovedStatus::Built,
                    JobStatus::DeploymentStarted => ApprovedStatus::DeploymentStarted,
                    JobStatus::ScopeExceeded => ApprovedStatus::ScopeExceeded,
                    JobStatus::StaleReview => ApprovedStatus::StaleReview,
                    JobStatus::Failed => ApprovedStatus::Failed,
                    JobStatus::Queued
                    | JobStatus::AwaitingApproval
                    | JobStatus::Rejected
                    | JobStatus::Cancelled => {
                        return Err(Error::InvalidJob {
                            reason: "approval decision conflicts with job status",
                        });
                    }
                };
                JobState::Approved {
                    authorization_id,
                    status,
                }
            }
            None => match value.status {
                JobStatus::Queued => JobState::Queued {
                    authorization_id: value.authorization_id,
                },
                JobStatus::AwaitingApproval => JobState::AwaitingApproval {
                    authorization_id: value.authorization_id.ok_or(Error::InvalidJob {
                        reason: "awaiting approval is missing its authorization ID",
                    })?,
                },
                JobStatus::Building
                | JobStatus::Planned
                | JobStatus::Built
                | JobStatus::DeploymentStarted => {
                    return Err(Error::InvalidJob {
                        reason: "work started without a recorded approval decision",
                    });
                }
                status @ (JobStatus::Rejected
                | JobStatus::Cancelled
                | JobStatus::ScopeExceeded
                | JobStatus::StaleReview
                | JobStatus::Failed) => {
                    let reason = StopReason::try_from(status)?;
                    JobState::Stopped {
                        authorization_id: value.authorization_id,
                        reason,
                    }
                }
            },
        };
        Ok(Self {
            request_id: value.request_id,
            state,
            details: value.details,
        })
    }
}

impl From<JobSnapshot> for JobWire {
    fn from(value: JobSnapshot) -> Self {
        let (status, decision, authorization_id) = match value.state {
            JobState::Queued { authorization_id } => (JobStatus::Queued, None, authorization_id),
            JobState::AwaitingApproval { authorization_id } => {
                (JobStatus::AwaitingApproval, None, Some(authorization_id))
            }
            JobState::Approved {
                authorization_id,
                status,
            } => (
                status.into(),
                Some(StoredDecision::Approved),
                Some(authorization_id),
            ),
            JobState::Stopped {
                authorization_id,
                reason,
            } => (reason.into(), None, authorization_id),
        };
        Self {
            request_id: value.request_id,
            status,
            decision,
            authorization_id,
            details: value.details,
        }
    }
}

impl JobSnapshot {
    pub fn request_id(&self) -> &JobId {
        &self.request_id
    }
    pub fn status(&self) -> JobStatus {
        self.submission_feedback().job_status()
    }
    pub fn decision(&self) -> Option<StoredDecision> {
        match &self.state {
            JobState::Approved { .. } => Some(StoredDecision::Approved),
            JobState::Queued { .. }
            | JobState::AwaitingApproval { .. }
            | JobState::Stopped { .. } => None,
        }
    }
    pub fn authorization_id(&self) -> Option<&ReviewId> {
        match &self.state {
            JobState::Queued { authorization_id }
            | JobState::Stopped {
                authorization_id, ..
            } => authorization_id.as_ref(),
            JobState::AwaitingApproval { authorization_id }
            | JobState::Approved {
                authorization_id, ..
            } => Some(authorization_id),
        }
    }

    /// Approval stays observable after later work finishes or fails.
    pub fn submission_feedback(&self) -> SubmissionFeedback {
        match &self.state {
            JobState::Queued { .. } => SubmissionFeedback::Pending(PendingStatus::Queued),
            JobState::AwaitingApproval { .. } => {
                SubmissionFeedback::Pending(PendingStatus::AwaitingApproval)
            }
            JobState::Approved { status, .. } => SubmissionFeedback::Approved(*status),
            JobState::Stopped { reason, .. } => SubmissionFeedback::Stopped(*reason),
        }
    }
}

/// Each category accepts only its own phases. In particular a pending failure
/// and an approved queued job cannot be constructed. Continuing is derived.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SubmissionFeedback {
    Approved(ApprovedStatus),
    Pending(PendingStatus),
    Stopped(StopReason),
}

impl SubmissionFeedback {
    pub fn job_status(self) -> JobStatus {
        match self {
            Self::Approved(status) => status.into(),
            Self::Pending(status) => status.into(),
            Self::Stopped(reason) => reason.into(),
        }
    }

    pub fn exit_code(self) -> Option<u8> {
        match self {
            Self::Approved(_) => Some(0),
            Self::Pending(_) => None,
            Self::Stopped(reason) => Some(reason.exit_code()),
        }
    }
}

impl Serialize for SubmissionFeedback {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        let approved = matches!(self, Self::Approved(_));
        let mut output =
            serializer.serialize_struct("SubmissionFeedback", if approved { 3 } else { 2 })?;
        let status = match self {
            Self::Approved(_) => "approved",
            Self::Pending(_) => "pending",
            Self::Stopped(_) => "stopped",
        };
        output.serialize_field("status", status)?;
        output.serialize_field("job_status", &self.job_status())?;
        if approved {
            output.serialize_field("continuing", &self.job_status().is_active())?;
        }
        output.end()
    }
}
