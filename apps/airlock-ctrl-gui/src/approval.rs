//! Trusted controller launch protocol. No submission client can select a decision.
use crate::model::Review;
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use std::{
    path::Path,
    sync::{Arc, Mutex},
};

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Scope {
    pub allowed_vms: Vec<String>,
    pub allow_reboot: bool,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub schema: String,
    pub request_id: String,
    pub baseline: String,
    pub revision: String,
    pub lock_hash: String,
    pub family: String,
    pub target: Option<String>,
    pub scope: Scope,
}

#[derive(Clone, Serialize)]
pub struct Response {
    pub schema: &'static str,
    pub request_id: String,
    pub revision: String,
    pub lock_hash: String,
    pub review_fingerprint: String,
    pub decision: &'static str,
    pub scope: Scope,
}

pub type Reply = Arc<Mutex<Option<Response>>>;

pub const REBOOT_NOTICE: &str =
    "May be required. Confirmed after building; you choose when to restart.";

impl Request {
    pub fn load(path: &Path, review: &Review) -> Result<Self> {
        ensure!(
            std::fs::metadata(path)?.len() <= 65536,
            "approval request is too large"
        );
        let request: Self = serde_json::from_slice(&std::fs::read(path)?)?;
        ensure!(
            request.schema == "airlock.ctrl-approval-request.v1",
            "unsupported approval request"
        );
        ensure!(
            request.request_id.len() == 64
                && request.request_id.bytes().all(|b| b.is_ascii_hexdigit()),
            "invalid request ID"
        );
        ensure!(
            request.baseline == review.baseline.commit
                && request.revision == review.candidate.commit
                && Some(&request.lock_hash) == review.lock_hash.as_ref(),
            "approval request differs from reviewed source"
        );
        ensure!(
            matches!(request.family.as_str(), "machine" | "guest"),
            "invalid approval family"
        );
        ensure!(
            (request.family == "machine" && request.target.is_none())
                || (request.family == "guest" && request.target.as_deref().is_some_and(valid_name)),
            "invalid approval target"
        );
        ensure!(
            !request.scope.allowed_vms.is_empty()
                && request.scope.allowed_vms.len() <= 4096
                && request
                    .scope
                    .allowed_vms
                    .iter()
                    .all(|name| valid_name(name)),
            "invalid permitted VM names"
        );
        Ok(request)
    }

    pub fn response(&self, review: &Review, decision: &'static str, scope: Scope) -> Response {
        Response {
            schema: "airlock.ctrl-approval-response.v1",
            request_id: self.request_id.clone(),
            revision: self.revision.clone(),
            lock_hash: self.lock_hash.clone(),
            review_fingerprint: review.fingerprint.clone(),
            decision,
            scope,
        }
    }
}

pub fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b))
}

pub struct Session {
    pub request: Request,
    pub allowed_vms: String,
    pub reply: Reply,
}

impl Session {
    pub fn scope(&self) -> Option<Scope> {
        let names = self
            .allowed_vms
            .split([',', ' ', '\n'])
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .collect::<std::collections::BTreeSet<_>>();
        (!names.is_empty() && names.len() <= 4096 && names.iter().all(|name| valid_name(name)))
            .then(|| Scope {
                allowed_vms: names.into_iter().collect(),
                // Approve covers the reviewed change, including a required
                // reboot. Activation still leaves reboot timing to the user.
                allow_reboot: true,
            })
    }

    pub fn finish(&self, review: &Review, decision: &'static str) {
        let scope = if decision == "approve" {
            self.scope()
        } else {
            Some(self.request.scope.clone())
        };
        if let Some(scope) = scope {
            *self.reply.lock().unwrap() = Some(self.request.response(review, decision, scope));
        }
    }

    pub fn copy_text(&self, base_report: &str) -> Result<String> {
        let mut report = base_report.to_owned();
        report.push_str("\n## Approval scope\n\n");
        report.push_str(&serde_json::to_string_pretty(&serde_json::json!({
            "request_id": self.request.request_id, "family": self.request.family,
            "target": self.request.target, "scope": self.scope(),
            "system_reboot": REBOOT_NOTICE,
            "purpose": "Approval permits evaluation, building and deployment within this scope, including changes that require a reboot. The final deployment plan is not yet known."
        }))?);
        report.push('\n');
        Ok(report)
    }
}
