//! Source-bound review and a consuming approval → checked-plan transition.
//!
//! `ApprovedChange` and `ApprovedDeployment` cannot be deserialized or directly
//! constructed. They prove the checks below ran, not that input came from a
//! trusted GUI, that dom0 granted authority, or that a deployment succeeded.

use crate::{Error, Result, identity::*, wire::string_enum};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "ScopeWire", into = "ScopeWire")]
pub struct Scope {
    allowed_vms: Vec<VmName>,
    allow_reboot: bool,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ScopeWire {
    allowed_vms: Vec<VmName>,
    allow_reboot: bool,
}

impl Scope {
    pub fn new(allowed_vms: Vec<VmName>, allow_reboot: bool) -> Result<Self> {
        if allowed_vms.is_empty() || allowed_vms.len() > 4096 {
            return Err(Error::InvalidField {
                field: "approval scope",
                expected: "1–4096 VM names",
            });
        }
        Ok(Self {
            allowed_vms: allowed_vms
                .into_iter()
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect(),
            allow_reboot,
        })
    }

    pub fn allowed_vms(&self) -> &[VmName] {
        &self.allowed_vms
    }

    pub fn allows_reboot(&self) -> bool {
        self.allow_reboot
    }

    pub fn check(&self, plan: &DeploymentPlan) -> Result<()> {
        let outside = plan
            .affected_vms()
            .into_iter()
            .filter(|name| self.allowed_vms.binary_search(name).is_err())
            .cloned()
            .collect::<Vec<_>>();
        if !outside.is_empty() {
            return Err(Error::ScopeExceeded { names: outside });
        }
        if !self.allow_reboot && plan.reboot_requirement() != RebootRequirement::NotRequired {
            return Err(Error::RebootNotApproved);
        }
        Ok(())
    }
}

impl TryFrom<ScopeWire> for Scope {
    type Error = Error;

    fn try_from(value: ScopeWire) -> Result<Self> {
        Self::new(value.allowed_vms, value.allow_reboot)
    }
}

impl From<Scope> for ScopeWire {
    fn from(value: Scope) -> Self {
        Self {
            allowed_vms: value.allowed_vms,
            allow_reboot: value.allow_reboot,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DeploymentTarget {
    Machine,
    Guest(GuestName),
}

/// Source plus the operation's machine/guest target. A Git revision alone does
/// not identify a guest candidate: several guests can be built from one commit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CandidateIdentity {
    pub source: SourceIdentity,
    pub target: DeploymentTarget,
}

string_enum! {
    pub enum RequestSchema ("approval request schema") {
        V1 => "airlock.ctrl-approval-request.v1",
    }
}

string_enum! {
    pub enum ResponseSchema ("approval response schema") {
        V1 => "airlock.ctrl-approval-response.v1",
    }
}

string_enum! {
    enum Family ("deployment family") {
        Machine => "machine",
        Guest => "guest",
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RequestWire {
    schema: RequestSchema,
    request_id: ReviewId,
    baseline: GitRevision,
    revision: GitRevision,
    lock_hash: LockHash,
    family: Family,
    target: Option<GuestName>,
    scope: Scope,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(try_from = "RequestWire", into = "RequestWire")]
pub struct ReviewRequest {
    request_id: ReviewId,
    baseline: GitRevision,
    candidate: CandidateIdentity,
    scope: Scope,
}

impl ReviewRequest {
    pub fn request_id(&self) -> &ReviewId {
        &self.request_id
    }
    pub fn baseline(&self) -> &GitRevision {
        &self.baseline
    }
    pub fn source(&self) -> &SourceIdentity {
        &self.candidate.source
    }
    pub fn target(&self) -> &DeploymentTarget {
        &self.candidate.target
    }
    pub fn candidate(&self) -> &CandidateIdentity {
        &self.candidate
    }
    pub fn scope(&self) -> &Scope {
        &self.scope
    }
}

impl TryFrom<RequestWire> for ReviewRequest {
    type Error = Error;

    fn try_from(value: RequestWire) -> Result<Self> {
        let target = match (value.family, value.target) {
            (Family::Machine, None) => DeploymentTarget::Machine,
            (Family::Guest, Some(name)) => DeploymentTarget::Guest(name),
            _ => {
                return Err(Error::InvalidField {
                    field: "approval target",
                    expected: "machine with no target, or guest with a VM name",
                });
            }
        };
        Ok(Self {
            request_id: value.request_id,
            baseline: value.baseline,
            candidate: CandidateIdentity {
                source: SourceIdentity {
                    revision: value.revision,
                    lock_hash: value.lock_hash,
                },
                target,
            },
            scope: value.scope,
        })
    }
}

impl From<ReviewRequest> for RequestWire {
    fn from(value: ReviewRequest) -> Self {
        let (family, target) = match value.candidate.target {
            DeploymentTarget::Machine => (Family::Machine, None),
            DeploymentTarget::Guest(name) => (Family::Guest, Some(name)),
        };
        Self {
            schema: RequestSchema::V1,
            request_id: value.request_id,
            baseline: value.baseline,
            revision: value.candidate.source.revision,
            lock_hash: value.candidate.source.lock_hash,
            family,
            target,
            scope: value.scope,
        }
    }
}

string_enum! {
    pub enum Decision ("human decision") {
        Approve => "approve",
        Reject => "reject",
        Cancelled => "cancelled",
    }
}

/// An unauthenticated wire reply, including rejections and cancellation.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewResponse {
    pub schema: ResponseSchema,
    pub request_id: ReviewId,
    pub revision: GitRevision,
    pub lock_hash: LockHash,
    pub review_fingerprint: ReviewFingerprint,
    pub decision: Decision,
    pub scope: Scope,
}

/// The caller must obtain these observations from the authoritative source and
/// deployed baseline. A candidate's parent commit is not a deployed baseline.
#[derive(Clone, Debug)]
pub struct ReviewObservation {
    pub baseline: GitRevision,
    pub baseline_state: BaselineFingerprint,
    pub candidate: CandidateIdentity,
}

#[derive(Debug)]
pub struct PendingReview {
    request: ReviewRequest,
    baseline_state: BaselineFingerprint,
}

#[derive(Debug)]
pub enum ReviewOutcome {
    Approved(Box<ApprovedChange>),
    Rejected,
    Cancelled,
}

impl PendingReview {
    pub fn new(request: ReviewRequest, observed: &ReviewObservation) -> Result<Self> {
        if request.baseline != observed.baseline || request.candidate != observed.candidate {
            return Err(Error::BindingMismatch {
                field: "reviewed source, target or deployed baseline",
            });
        }
        Ok(Self {
            request,
            baseline_state: observed.baseline_state.clone(),
        })
    }

    /// Consume the pending review exactly once. Only approval produces a checked
    /// change. The human may edit scope in the GUI, as in the current protocol.
    pub fn decide(
        self,
        response: ReviewResponse,
        current: &ReviewObservation,
    ) -> Result<ReviewOutcome> {
        if response.request_id != self.request.request_id {
            return Err(Error::BindingMismatch {
                field: "review request ID",
            });
        }
        if response.revision != self.request.candidate.source.revision
            || response.lock_hash != self.request.candidate.source.lock_hash
        {
            return Err(Error::BindingMismatch {
                field: "review response source",
            });
        }
        match response.decision {
            Decision::Reject => Ok(ReviewOutcome::Rejected),
            Decision::Cancelled => Ok(ReviewOutcome::Cancelled),
            Decision::Approve => {
                let approved = ApprovedChange {
                    request: self.request,
                    baseline_state: self.baseline_state,
                    fingerprint: response.review_fingerprint,
                    scope: response.scope,
                };
                approved.recheck(current)?;
                Ok(ReviewOutcome::Approved(Box::new(approved)))
            }
        }
    }
}

#[derive(Debug)]
pub struct ApprovedChange {
    request: ReviewRequest,
    baseline_state: BaselineFingerprint,
    fingerprint: ReviewFingerprint,
    scope: Scope,
}

impl ApprovedChange {
    pub fn request(&self) -> &ReviewRequest {
        &self.request
    }
    pub fn scope(&self) -> &Scope {
        &self.scope
    }
    pub fn fingerprint(&self) -> &ReviewFingerprint {
        &self.fingerprint
    }

    /// Call immediately before any future evaluation/build or activation.
    pub fn recheck(&self, current: &ReviewObservation) -> Result<()> {
        if self.baseline_state != current.baseline_state
            || self.request.baseline != current.baseline
        {
            return Err(Error::StaleReview);
        }
        if self.request.candidate != current.candidate {
            return Err(Error::BindingMismatch {
                field: "current candidate source or target",
            });
        }
        Ok(())
    }

    pub fn bind_plan(
        self,
        current: &ReviewObservation,
        prepared: PreparedPlan,
    ) -> Result<ApprovedDeployment> {
        self.recheck(current)?;
        if prepared.candidate != self.request.candidate {
            return Err(Error::BindingMismatch {
                field: "deployment plan source or target",
            });
        }
        self.scope.check(&prepared.plan)?;
        Ok(ApprovedDeployment {
            approval: self,
            prepared,
        })
    }
}

/// A checked source/scope/plan relationship. It grants no runtime authority and
/// says nothing about build completion, activation, or reboot completion.
#[derive(Debug)]
pub struct ApprovedDeployment {
    approval: ApprovedChange,
    prepared: PreparedPlan,
}

impl ApprovedDeployment {
    pub fn approval(&self) -> &ApprovedChange {
        &self.approval
    }
    pub fn plan(&self) -> &DeploymentPlan {
        &self.prepared.plan
    }
    pub fn candidate(&self) -> &CandidateIdentity {
        &self.prepared.candidate
    }
}

/// A plan paired with the complete candidate identity obtained by its producer.
/// Runtime adapters must obtain that identity from the actual build manifest;
/// this value does not authenticate the producer or grant deployment authority.
#[derive(Debug)]
pub struct PreparedPlan {
    candidate: CandidateIdentity,
    plan: DeploymentPlan,
}

impl PreparedPlan {
    pub fn new(candidate: CandidateIdentity, plan: DeploymentPlan) -> Result<Self> {
        if plan.revision.as_ref() != Some(&candidate.source.revision)
            || plan
                .lock_hash
                .as_ref()
                .is_some_and(|hash| hash != &candidate.source.lock_hash)
        {
            return Err(Error::BindingMismatch {
                field: "deployment plan source",
            });
        }
        Ok(Self { candidate, plan })
    }

    pub fn candidate(&self) -> &CandidateIdentity {
        &self.candidate
    }
    pub fn plan(&self) -> &DeploymentPlan {
        &self.plan
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RebootRequirement {
    Unknown,
    Required,
    NotRequired,
}

/// The current plan's VM impact fields, including private-volume copy endpoints.
/// Other policy/device metadata is retained rather than silently discarded.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeploymentPlan {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    revision: Option<GitRevision>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    lock_hash: Option<LockHash>,
    changed: Vec<VmName>,
    stop: Vec<VmName>,
    start: Vec<VmName>,
    #[serde(default)]
    retire: Vec<VmName>,
    #[serde(default)]
    restore: Vec<VmName>,
    #[serde(default)]
    stage: Vec<VmName>,
    #[serde(default)]
    affected: Vec<VmName>,
    #[serde(default)]
    private_copies: Vec<PrivateCopy>,
    host_reboot_required: Option<bool>,
    #[serde(flatten)]
    details: BTreeMap<String, serde_json::Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PrivateCopy {
    source: PrivateCopySource,
    target_name: VmName,
    #[serde(flatten)]
    details: BTreeMap<String, serde_json::Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PrivateCopySource {
    domain: VmName,
    #[serde(flatten)]
    details: BTreeMap<String, serde_json::Value>,
}

impl PrivateCopy {
    pub fn source(&self) -> &PrivateCopySource {
        &self.source
    }
    pub fn target_name(&self) -> &VmName {
        &self.target_name
    }
    pub fn details(&self) -> &BTreeMap<String, serde_json::Value> {
        &self.details
    }
}

impl PrivateCopySource {
    pub fn domain(&self) -> &VmName {
        &self.domain
    }
    pub fn details(&self) -> &BTreeMap<String, serde_json::Value> {
        &self.details
    }
}

impl DeploymentPlan {
    pub fn revision(&self) -> Option<&GitRevision> {
        self.revision.as_ref()
    }
    pub fn lock_hash(&self) -> Option<&LockHash> {
        self.lock_hash.as_ref()
    }
    pub fn changed(&self) -> &[VmName] {
        &self.changed
    }
    pub fn stop(&self) -> &[VmName] {
        &self.stop
    }
    pub fn start(&self) -> &[VmName] {
        &self.start
    }
    pub fn retire(&self) -> &[VmName] {
        &self.retire
    }
    pub fn restore(&self) -> &[VmName] {
        &self.restore
    }
    pub fn stage(&self) -> &[VmName] {
        &self.stage
    }
    pub fn affected(&self) -> &[VmName] {
        &self.affected
    }
    pub fn private_copies(&self) -> &[PrivateCopy] {
        &self.private_copies
    }
    pub fn details(&self) -> &BTreeMap<String, serde_json::Value> {
        &self.details
    }

    pub fn affected_vms(&self) -> BTreeSet<&VmName> {
        let mut names = [
            &self.changed,
            &self.stop,
            &self.start,
            &self.retire,
            &self.restore,
            &self.stage,
            &self.affected,
        ]
        .into_iter()
        .flatten()
        .collect::<BTreeSet<_>>();
        for copy in &self.private_copies {
            names.insert(&copy.source.domain);
            names.insert(&copy.target_name);
        }
        names
    }

    pub fn reboot_requirement(&self) -> RebootRequirement {
        match self.host_reboot_required {
            Some(true) => RebootRequirement::Required,
            Some(false) => RebootRequirement::NotRequired,
            None => RebootRequirement::Unknown,
        }
    }
}
