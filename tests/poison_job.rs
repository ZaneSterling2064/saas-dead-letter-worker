use saas_dead_letter_worker::dead_letter_policy::{
    decide, JobDecision, SaaSJob, SaaSOperation,
};

#[test]
fn third_failure_moves_tenant_onboarding_to_dead_letter() {
    let job = SaaSJob {
        tenant_id: "tenant_acme".to_owned(),
        operation: SaaSOperation::TenantOnboarding,
        attempts: 3,
    };

    assert_eq!(
        decide(&job, 3),
        JobDecision::DeadLetter {
            reason: "attempt budget exhausted"
        }
    );
}

