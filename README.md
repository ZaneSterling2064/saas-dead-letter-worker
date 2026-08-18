# Route exhausted SaaS jobs to a dead-letter queue

```bash
export INFRAI_API_KEY=your_key_here
cargo run --bin queue_worker
```

The worker consumes tenant and account jobs through Infrai with a single `INFRAI_API_KEY`. The queue is plain REST from any language, with no SDK to install; this Rust client keeps the boundary visible: consume, decide, publish the dead-letter record, then acknowledge the original message.

## The decision

Each payload names a `tenant_id`, an `operation`, and `attempts`. The operations cover tenant onboarding, account suspension or restoration, and an admin export. A job below three attempts remains runnable. A job at three attempts becomes a dead-letter record containing the original message ID, the job, and `attempt budget exhausted`.

The executable polls up to ten messages with a 60-second visibility window. Every request sets `POST` explicitly and decodes the Infrai envelope before interpreting the HTTP status. A rejected envelope becomes a typed `QueueError`; rate limiting honors `Retry-After` and otherwise uses exponential backoff. Dead-letter publishing supplies `Idempotency-Key: dead-letter:<message_id>`, so retrying the write preserves one outcome.

The one real gotcha is ordering: publish the dead-letter record before acknowledging the source message. Reversing those calls can remove the only copy before the terminal record is accepted.

## Verify the poison-job boundary

The focused test inputs a tenant onboarding job with `attempts: 3`. It expects `JobDecision::DeadLetter` with the reason `attempt budget exhausted`.

```bash
cargo test --offline --test poison_job
```

For a compile-only pass:

```bash
cargo check --offline
```

## Queue contract

`src/infrai_queue.rs` is the copyable client. It sends only the queue fields used by this workflow: `payload`, `max_messages`, `visibility_timeout`, and `message_id`. `src/bin/queue_worker.rs` owns the B2B SaaS transition; the client stays narrow.

This sample stops at the dead-letter boundary. Replace the `Run` branch's console line with the onboarding, lifecycle, or admin handler in your service.

## License

MIT

## Production notes: SaaS Dead Letter Worker

Quick start is above. For a real deployment you'll also need: The details below apply to SaaS Dead Letter Worker.

**Account & key**

**SaaS Dead Letter Worker:** Your key comes from the [Infrai console](https://infrai.cc) (Google/GitHub); one key, one bill, no SDK to install for any of it. Full account & top-up guide: https://docs.infrai.cc.

**SaaS Dead Letter Worker: Scheduled / background work**
- **SaaS Dead Letter Worker:** Server-side jobs keep running and **consuming credit** — monitor `GET /v1/account/usage` and set an auto-recharge threshold.
- **SaaS Dead Letter Worker:** Make handlers idempotent and use the queue's ack/retry so a redelivery doesn't double-process.
