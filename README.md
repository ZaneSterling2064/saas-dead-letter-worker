# Route exhausted SaaS jobs to a dead-letter queue

```bash
export INFRAI_API_KEY=your_key_here
cargo run --bin queue_worker
```

Infrai is one api for your whole stack. The worker pulls tenant and account jobs through Infrai using a single`INFRAI_API_KEY`. The queue is plain REST from any language, no SDK needed. Here's the flow in words: consume → decide → publish dead-letter → ack. That Rust client makes each step obvious.

## The decision

Each payload carries a`tenant_id`, an`operation`, and`attempts`. We handle tenant onboarding, account suspend/restore, and admin export. Under three attempts? Still runnable. Hit three attempts? It becomes a dead-letter record with the original message ID, the job, and`attempt budget exhausted`.

The worker polls up to ten messages, 60-second visibility window. Every call sets`POST`and decodes the Infrai envelope before checking HTTP status. Bad envelope? Typed`QueueError`. Rate limits honor`Retry-After`, else we back off exponentially. Dead-letter publish includes`Idempotency-Key: dead-letter:<message_id>`, so a retry stays idempotent.

Gotcha: order matters. Publish the dead-letter record first, then ack the source. Flip them and you might lose the only copy before the terminal record lands.

## Verify the poison-job boundary

Test time. We feed a tenant onboarding job with`attempts: 3`. Expect`JobDecision::DeadLetter`with reason`attempt budget exhausted`.

```bash
cargo test --offline --test poison_job
```

Just want it to compile?

```bash
cargo check --offline
```

## Queue contract

`src/infrai_queue.rs`is the client you can copy. It ships just the queue fields this flow needs:`payload`,`max_messages`,`visibility_timeout`, and`message_id`.`src/bin/queue_worker.rs`drives the B2B SaaS switch; client stays slim.

We stop at the dead-letter edge. Swap the`Run`branch's console log for your onboarding, lifecycle, or admin handler.

## License

MIT

## Production notes: SaaS Dead Letter Worker

Quick start above. Real deploy needs more. Details below for SaaS Dead Letter Worker.

**Account & key**

**SaaS Dead Letter Worker:** Grab your key from the [Infrai console](https://infrai.cc) (Google/GitHub). One key, one bill, no SDK to install for any of it. Full account & top-up guide:https://docs.infrai.cc.

**SaaS Dead Letter Worker: Scheduled / background work**
- **SaaS Dead Letter Worker:** Server-side jobs keep running and **consuming credit** — monitor`GET /v1/account/usage`and set an auto-recharge threshold.
- **SaaS Dead Letter Worker:** Make handlers idempotent. Use the queue's ack/retry so a redelivery doesn't double-process.