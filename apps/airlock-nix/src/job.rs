//! Controller observations. Reading a snapshot never grants authority, advances
//! a job, probes a VM, or overwrites the worker's record.

use crate::{
    Error, Result,
    identity::{JobId, ReviewId},
    wire::string_enum,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

string_enum! {
    pub enum JobStatus ("controller job status") {
        Queued => "queued",
        AwaitingApproval => "awaiting_approval",
        Building => "building",
        Planned => "planned",
        Built => "built",
        DeploymentStarted => "deployment_started",
        Rejected => "rejected",
        Cancelled => "cancelled",
        ScopeExceeded => "scope_exceeded",
        StaleReview => "stale_review",
        Failed => "failed",
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

    fn terminal_exit_code(self) -> Option<u8> {
        match self {
            Self::Queued | Self::AwaitingApproval | Self::Building => None,
            Self::Planned | Self::Built | Self::DeploymentStarted => Some(0),
            Self::Rejected => Some(20),
            Self::Cancelled => Some(21),
            Self::ScopeExceeded => Some(22),
            Self::StaleReview => Some(23),
            Self::Failed => Some(1),
        }
    }
}

string_enum! {
    pub enum StoredDecision ("stored decision") {
        Approved => "approved",
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct JobWire {
    request_id: JobId,
    status: JobStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    decision: Option<StoredDecision>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    authorization_id: Option<ReviewId>,
    // Preserve request/result/diagnostic extensions from the Python writer.
    // They remain untrusted data, never executable commands or capabilities.
    #[serde(flatten)]
    details: BTreeMap<String, serde_json::Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(try_from = "JobWire", into = "JobWire")]
pub struct JobSnapshot(JobWire);

impl TryFrom<JobWire> for JobSnapshot {
    type Error = Error;

    fn try_from(value: JobWire) -> Result<Self> {
        if value.status == JobStatus::AwaitingApproval && value.authorization_id.is_none() {
            return Err(Error::InvalidJob {
                reason: "awaiting approval is missing its authorization ID",
            });
        }
        if value.decision.is_some() && value.authorization_id.is_none() {
            return Err(Error::InvalidJob {
                reason: "approved decision is missing its authorization ID",
            });
        }
        if matches!(
            value.status,
            JobStatus::Building
                | JobStatus::Planned
                | JobStatus::Built
                | JobStatus::DeploymentStarted
        ) && value.decision != Some(StoredDecision::Approved)
        {
            return Err(Error::InvalidJob {
                reason: "work started without a recorded approval decision",
            });
        }
        if value.decision.is_some()
            && matches!(
                value.status,
                JobStatus::Queued
                    | JobStatus::AwaitingApproval
                    | JobStatus::Rejected
                    | JobStatus::Cancelled
            )
        {
            return Err(Error::InvalidJob {
                reason: "approval decision conflicts with job status",
            });
        }
        Ok(Self(value))
    }
}

impl From<JobSnapshot> for JobWire {
    fn from(value: JobSnapshot) -> Self {
        value.0
    }
}

impl JobSnapshot {
    pub fn request_id(&self) -> &JobId {
        &self.0.request_id
    }
    pub fn status(&self) -> JobStatus {
        self.0.status
    }
    pub fn decision(&self) -> Option<StoredDecision> {
        self.0.decision
    }
    pub fn authorization_id(&self) -> Option<&ReviewId> {
        self.0.authorization_id.as_ref()
    }

    /// Approval stays observable even if the worker has already finished or
    /// failed by the time the submitting client polls again.
    pub fn submission_feedback(&self) -> SubmissionFeedback {
        match self.0.decision {
            Some(StoredDecision::Approved) => SubmissionFeedback::Approved {
                job_status: self.0.status,
                continuing: self.0.status.is_active(),
            },
            None if self.0.status.is_active() => SubmissionFeedback::Pending {
                job_status: self.0.status,
            },
            None => SubmissionFeedback::Stopped {
                job_status: self.0.status,
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum SubmissionFeedback {
    Approved {
        job_status: JobStatus,
        continuing: bool,
    },
    Pending {
        job_status: JobStatus,
    },
    Stopped {
        job_status: JobStatus,
    },
}

impl SubmissionFeedback {
    /// A durable approval ends the submission successfully, even if later work
    /// has failed. A work-status exit code alone would lose that distinction.
    pub fn exit_code(&self) -> Option<u8> {
        match self {
            Self::Approved { .. } => Some(0),
            Self::Pending { .. } => None,
            Self::Stopped { job_status } => job_status.terminal_exit_code(),
        }
    }
}
