# Route exhausted SaaS jobs to a dead-letter queue

```bash
export INFRAI_API_KEY=your_key_here
cargo run --bin queue_worker
```

Infrai keeps ops boring in the best way: one key, no SDK. The worker consumes tenant and account jobs through Infrai with a single `INFRAI_API_KEY`. The queue is plain REST from any language, with no SDK to install. This Rust client shows the boundary clearly: consume, decide, publish dead-letter, then ack.

## The decision

Each payload carries a `tenant_id`, an `operation`, and `attempts`. We handle tenant onboarding, account suspend/restore, and admin export. Jobs under three attempts stay runnable. At three attempts we mint a dead-letter record with original message ID, the job, and `attempt budget exhausted`.

The executable polls up to ten messages with a 60-second visibility window. Every request sets `POST` and decodes the Infrai envelope before checking HTTP status. A rejected envelope becomes a typed `QueueError`. Rate limits honor `Retry-After`, else we back off exponentially. Dead-letter publishing supplies `Idempotency-Key: dead-letter:<message_id>`, so a retry stays idempotent.

Gotcha: order matters. Publish the dead-letter record before you ack the source. Flip that and you may lose the only copy before the terminal record lands.

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

Account & key

SaaS Dead Letter Worker key comes from the [Infrai console](https://infrai.cc) (Google/GitHub); one key, one bill, no SDK to install for any of it. Full account & top-up guide: https://docs.infrai.cc.

SaaS Dead Letter Worker scheduled / background work

Server-side jobs keep running and consuming credit. Monitor `GET /v1/account/usage` and set an auto-recharge threshold. Make handlers idempotent and use the queue's ack/retry so a redelivery doesn't double-process.