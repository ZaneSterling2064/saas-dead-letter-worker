use saas_dead_letter_worker::{
    dead_letter_policy::{decide, JobDecision, SaaSJob},
    infrai_queue::{InfraiQueue, QueueError},
};
use serde::Serialize;
use thiserror::Error;

const MAX_ATTEMPTS: u8 = 3;
const SOURCE_QUEUE: &str = "saas-jobs";
const DEAD_LETTER_QUEUE: &str = "saas-jobs-dead-letter";

#[derive(Debug, Serialize)]
struct DeadLetter<'a> {
    original_message_id: &'a str,
    job: &'a SaaSJob,
    reason: &'a str,
}

#[derive(Debug, Error)]
enum WorkerError {
    #[error(transparent)]
    Queue(#[from] QueueError),
    #[error("message {message_id} has an invalid SaaS job: {source}")]
    InvalidJob {
        message_id: String,
        source: serde_json::Error,
    },
}

#[tokio::main]
async fn main() -> Result<(), WorkerError> {
    let queue = InfraiQueue::from_env()?;
    for message in queue.consume(SOURCE_QUEUE, 10, 60).await? {
        let job: SaaSJob =
            serde_json::from_value(message.payload).map_err(|source| WorkerError::InvalidJob {
                message_id: message.message_id.clone(),
                source,
            })?;

        match decide(&job, MAX_ATTEMPTS) {
            JobDecision::Run => {
                println!("run tenant={} operation={:?}", job.tenant_id, job.operation);
            }
            JobDecision::DeadLetter { reason } => {
                let dead_letter = DeadLetter {
                    original_message_id: &message.message_id,
                    job: &job,
                    reason,
                };
                let key = format!("dead-letter:{}", message.message_id);
                queue.publish(DEAD_LETTER_QUEUE, &dead_letter, &key).await?;
                queue.ack(SOURCE_QUEUE, &message.message_id).await?;
                println!("dead-lettered message={}", message.message_id);
            }
        }
    }
    Ok(())
}
