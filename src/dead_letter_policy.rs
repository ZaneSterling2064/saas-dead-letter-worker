use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SaaSOperation {
    TenantOnboarding,
    SuspendAccount,
    RestoreAccount,
    AdminExport,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct SaaSJob {
    pub tenant_id: String,
    pub operation: SaaSOperation,
    pub attempts: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobDecision {
    Run,
    DeadLetter { reason: &'static str },
}

pub fn decide(job: &SaaSJob, max_attempts: u8) -> JobDecision {
    if job.attempts >= max_attempts {
        JobDecision::DeadLetter {
            reason: "attempt budget exhausted",
        }
    } else {
        JobDecision::Run
    }
}

