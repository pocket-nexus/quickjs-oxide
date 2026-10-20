# Promises and jobs

Promise reactions, `await` resumptions, dynamic `import()` and `FinalizationRegistry` callbacks do not run immediately. They are queued as jobs on the runtime's job queue, and the embedder decides when the queue runs. Nothing runs a job on its own: `eval`, `call` and `execute_module` return as soon as their synchronous part is done.

## The job queue

A runtime has one first-in, first-out queue of jobs.

| Method | Effect |
| --- | --- |
| `Runtime::is_job_pending()` | whether a job is queued |
| `Runtime::run_pending_job()` | runs the oldest job; `PendingJobOutcome::Executed`, or `PendingJobOutcome::NoJob` when the queue was empty |
| `Runtime::execute_pending_job()` | the same as `run_pending_job` |
| `Runtime::run_jobs()` | runs jobs until the queue is empty or one throws |

`PendingJobOutcome::executed()` returns `true` for `Executed`. A job can queue further jobs, which run later in the same drain.

<!-- include: examples/jobs.rs#queue -->

## When a job throws

Most JavaScript errors inside jobs do not reach the queue. A promise handler that throws rejects the promise its `then` returned, and an `async` function that throws rejects its own promise. A job throws to the queue only when nothing turns the error into a rejection, as with a `FinalizationRegistry` cleanup callback.

When that happens, `run_pending_job` returns `Err(RuntimeError::Exception)` and leaves the thrown value as the context's pending exception. The jobs after it stay queued. `run_jobs` stops at the first throw, so a loop that should keep going calls `run_pending_job` itself:

<!-- include: examples/jobs.rs#throwing-job -->

This is how the `qjs` command drains its queue: it prints each job's exception and continues.

## Driving a promise to completion

`Runtime::promise_snapshot(&promise)` reads a promise's state without running JavaScript. It returns `None` when the object is not a promise, and otherwise a `PromiseSnapshot` with `state()` (`PromiseState::Pending`, `Fulfilled` or `Rejected`) and `result()`, the fulfillment value or rejection reason (`undefined` while pending).

To wait for one promise, run jobs until it settles:

<!-- include: examples/jobs.rs#drive -->

Calling an `async` function from Rust returns its promise, which this loop settles:

<!-- include: examples/jobs.rs#drive-call -->

When the queue is empty and the promise is still pending, no job can settle it. The engine defines no timers or I/O, so only the embedder could settle it later, for example by calling a resolve function it kept. An embedder with nothing else to wait for treats this as an error. The same loop drives the promise `execute_module` returns ([Modules](./modules.md#compiling-and-running-a-module)).

## Unhandled rejections

A rejected promise with no handler is easy to lose. The runtime tracks such promises when `Runtime::set_track_rejections(true)` is set. That follows QuickJS's `js_std_promise_rejection_tracker`:

- a promise rejected without a handler is added to the list;
- attaching a handler to it later removes it again;
- `Runtime::take_unhandled_rejections()` returns the list as `(promise, reason)` pairs, in rejection order, and empties it.

A promise that gets its handler in a later job is not unhandled, so read the list after the queue has drained:

<!-- include: examples/jobs.rs#rejections -->

The `qjs` command turns tracking on unless it is given `--no-unhandled-rejection`, and prints `Possibly unhandled promise rejection: ` and the reason for each entry once the queue is empty.

Independently of tracking, the runtime reports every rejection event to its host through `Host::promise_rejection` ([Host services](./host.md#promise-rejection-events)).

## Execution turns

`Runtime::with_execution_turn(body)` runs `body` as one synchronous run of JavaScript: an object a `WeakRef` points to stays alive until the outermost turn ends, as ECMAScript's `ClearKeptObjects` requires. Jobs run between turns. Inside a turn, `run_pending_job` returns `Err(RuntimeError::Invariant(_))` and leaves the job queued.

The complete program is [`examples/jobs.rs`](https://github.com/pocket-nexus/quickjs-oxide/blob/main/examples/jobs.rs):

```sh
cargo run --quiet --example jobs
```

## Where to go next

- [Modules](./modules.md): module evaluation and `import()` both settle through the job queue.
- [Host services](./host.md): receive rejection events in the host.
- [Errors and limits](./limits.md): exceptions and `RuntimeError`.
