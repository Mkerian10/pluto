# Orchestration Layer

## Separate from the Language

Infrastructure orchestration is **not** part of the Pluto language. It is a separate tool/framework built on top of Pluto.

## Rationale

Including infrastructure management in the language itself would turn Pluto into a DevOps/K8s system rather than a programming language. The language should stay focused on application logic and distributed communication.

However, the orchestration layer can leverage Pluto's strengths (whole-program analysis, typed errors, static verification) because it's built with Pluto.

## Scope

The orchestration layer would handle:

- Provisioning infrastructure (databases, clusters, queues)
- Deploying Pluto apps onto infrastructure
- Scaling policies
- Managing non-Pluto workloads (e.g., compiling and deploying Go services, starting EMR clusters)
- Environment configuration (binding `APIDatabase` to a concrete Postgres instance)
- Geographic placement decisions

## Relationship to the Language

The Pluto language provides:
- Bracket deps and ambient deps that the orchestration layer must satisfy
- Type contracts that the orchestration layer must respect
- The app as the deployable unit

The orchestration layer provides:
- Concrete implementations for injected dependencies
- Infrastructure lifecycle management
- Scaling and placement decisions

## Vision

The long-term vision is a system that can manage entire distributed platforms — Pluto apps, non-Pluto services, databases, clusters — all with the type safety and static verification that Pluto provides. But this is a separate project built on Pluto, not Pluto itself.
