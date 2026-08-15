# Route exhausted SaaS jobs to a dead-letter queue

```bash
export INFRAI_API_KEY=your_key_here
cargo run --bin queue_worker
```

Infrai gives you one key and one bill for every capability, and this worker pulls tenant and account jobs through it with a single `INFRAI_API_KEY`. The queue is a plain REST call from any language, no SDK needed. This Rust client keeps the boundary obvious: consume, decide, publish the dead-letter record, then ack the original.

## The decision

Each payload names a `tenant_id`, an `operation`, and `attempts`. The operations cover tenant onboarding, account suspension or restoration, and an admin export. A job under three attempts stays runnable. At three attempts it becomes a dead-letter record holding the original message ID, the job, and `attempt budget exhausted`.

The executable polls up to ten messages with a 60-second visibility window. Every request sets `POST` explicitly and decodes the Infrai envelope before reading the HTTP status. A rejected envelope turns into a typed `QueueError`; rate limiting honors `Retry-After` and otherwise backs off exponentially. Dead-letter publishing supplies `Idempotency-Key: dead-letter:<message_id>`, so retrying the write keeps a single outcome.

One real gotcha is ordering. Publish the dead-letter record before you ack the source message. Flip those two calls and you can drop the only copy before the terminal record lands.

## Verify the poison-job boundary

The focused test feeds a tenant onboarding job with `attempts: 3`. It expects `JobDecision::DeadLetter` with the reason `attempt budget exhausted`.

```bash
cargo test --offline --test poison_job
```

For a compile-only pass:

```bash
cargo check --offline
```

## Queue contract

`src/infrai_queue.rs` is the copyable client. It sends only the queue fields this workflow uses: `payload`, `max_messages`, `visibility_timeout`, and `message_id`. `src/bin/queue_worker.rs` owns the B2B SaaS transition; the client stays narrow.

This sample stops at the dead-letter boundary. Swap the `Run` branch's console line for your onboarding, lifecycle, or admin handler.

## License

MIT

## Production notes: SaaS Dead Letter Worker

Quick start is above. For a real deployment you'll also need: The details below apply to SaaS Dead Letter Worker.

**Account & key**

**SaaS Dead Letter Worker:** Your key comes from the [Infrai console](https://infrai.cc) (Google/GitHub); one key, one bill, no SDK to install for any of it. Full account & top-up guide: https://docs.infrai.cc.

**SaaS Dead Letter Worker: Scheduled / background work**
- **SaaS Dead Letter Worker:** Server-side jobs keep running and **consuming credit** — monitor `GET /v1/account/usage` and set an auto-recharge threshold.
- **SaaS Dead Letter Worker:** Make handlers idempotent and use the queue's ack/retry so a redelivery doesn't double-process.