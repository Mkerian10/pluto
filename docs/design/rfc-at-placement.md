# RFC: `at` Placement Expressions — Slice 1

**Status:** Slice 1 implemented
**Author:** Matt Kerian
**Date:** 2026-09-04
**Related:** [distributed-model.md](distributed-model.md) (the model this implements), communication.md, rfc-distributed-safety.md

## Goal

Make the distributed model's central claim true in running code:

> A program can be redistributed without rewriting business logic. The `at` expression states *where computation logically belongs*; the deployment plan decides how the boundary is physically crossed.

Slice 1 delivers the smallest honest version: one binary, whose `at` boundaries lower to **either** a direct in-process call **or** the socket transport, decided by the deployment binding at startup — with the *same* compile-time boundary contract in both plans.

## Surface

### Declaring a domain dependency

```pluto
import payments

app Shop[pay: domain payments.PaymentService] {
    ...
}
```

`domain` (contextual keyword, like `remote`) marks a dependency as a **logical execution domain**: computation placed there with `at` runs against the `PaymentService` interface, wherever the plan puts it.

### Placing computation

```pluto
let result = at self.pay {
    charge(order_total)
} catch err { -1 }
```

Inside the block, bare calls resolve against the **domain's service interface** — `charge` is `PaymentService.charge`. The block is evaluated in the `pay` domain; captured values (here `order_total`) enter the domain, the result leaves it.

### The physical plan

At startup, the binding `PLUTO_DOMAIN_<SERVICE>` (e.g. `PLUTO_DOMAIN_PAYMENTSERVICE=127.0.0.1:9000`) determines the plan for each domain:

- **bound** → the boundary crosses the socket transport (wire marshaling, interface hash check, framing — the existing machinery)
- **unbound** → the domain is colocated: the DI-wired local instance is called directly, in process

Same binary. Two plans. Zero code change. (Compile-time and deploy-time plan artifacts are future work; an env binding is the slice-1 stand-in, consistent with `PLUTO_REMOTE_*`.)

## The boundary contract is plan-independent

This is the model-critical part. **Every check applies in both plans:**

- Arguments entering and the result leaving the domain must be wire-shaped (the schema-level surface: scalars, classes, enums, nullables, arrays, maps, sets). A non-transferable value is a compile error even if the domain is colocated in every deployment you currently run.
- The `at` expression is **always fallible** — the boundary contributes its failure contract (today `NetworkError`, the stand-in stage-level error) to the caller's inferred error set regardless of plan. Colocated code still handles unreachability, because the next deployment may not be colocated. This is what "logical boundaries survive colocation" means in practice.
- Version-skew protection (interface hashing) applies whenever the plan crosses a process boundary.

## Restrictions (slice 1)

- The block body is **exactly one expression: a single call** to a method of the domain's interface. General computation shipping ("run this whole block over there") requires compiling the block *into* the domain's binary — that is the system-layer/whole-program-deployment slice, not this one. The restriction is enforced with a targeted error.
- Direct method calls on a domain dep **outside** an `at` block are rejected ("computation in a domain must be placed with `at`"). The boundary must be syntactically visible.
- The colocated plan requires the domain service to be DI-constructible in this binary (its deps must exist here). When a domain is always deployed remotely, the local instance is still wired — a known slice-1 inefficiency; the plan artifact will make wiring plan-aware.
- `remote` deps keep their current behavior, unchanged. `domain` is the model-aligned replacement; migration and deprecation are follow-ups.

## Errors: layered, per the model

Transport failures surface as the stage-level error (`NetworkError` today); application code maps them to domain errors at the `catch`. Slice 1 does not yet add distinct stage-level error types (unreachable vs. deadline vs. ambiguous-completion) — that is open question 4 of distributed-model.md and stays open.

## What this slice forces us to answer (and how it answers)

| Open question (distributed-model.md) | Slice-1 answer |
|---|---|
| Domain identity | A domain is a DI dependency marked `domain`, typed by a service interface. (System-level domain declarations remain open.) |
| Value transfer | The wire surface, checked at compile time in all plans. (Handles/capabilities remain non-transferable by construction.) |
| Failure contract | The boundary is unconditionally fallible; one stage-level error type for now. |
| Plan binding | Startup env binding per domain; compile/deploy-time plan artifacts open. |

## Future slices

2. **Block shipping** — compile the block into the domain's binary as a synthesized method (requires the system layer / multi-binary compilation from one program).
3. **Stage-level error taxonomy** — unreachable / deadline / ambiguous-completion as distinct, checkable outcomes; idempotency declarations gating plan-inserted retries.
4. **Plan artifact** — a checked deployment plan replacing env bindings; plan-aware DI wiring; legal-colocation rules (isolation, secrets, capabilities).
5. **Deadlines & cancellation** — propagation semantics identical across plans.
