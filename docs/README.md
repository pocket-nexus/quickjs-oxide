# Documentation

## Current implementation

- [Architecture](architecture.md): package boundaries, semantic owners, current
  execution model and ownership rules.
- [Implementation status](status.md): implemented behavior and the pinned
  compatibility baseline.
- [Test262](test262.md), [parity contract](parity.md) and
  [registered deviations](deviations.md): acceptance and evidence.
- [Profiling and benchmarks](profiling.md): tools, measurement contracts and
  historical reports.
- [Browser playground](playground.md): build and host boundaries.

## Active stack VM redesign

As of 2026-09-12, this work is planned and has not changed the production
engine. The selected representation is a stack VM with linear stack IR.
The PR must address issue #16's native-stack, Number execution, call storage,
local-update and PC-publication problems.

| Document | Responsibility |
| --- | --- |
| [VM plan](primitive-vm-plan.md) | Goals, architecture decisions and scope |
| [Implementation design](primitive-vm-implementation-plan.md) | State ownership, proposed code structure and algorithms |
| [10-commit plan](primitive-vm-commit-plan.md) | S01–S10 delivery order and checks at each stage |
| [Migration checklist](primitive-vm-migration.md) | Capability coverage, structure tasks and completion evidence |

## History

[Archived architecture and completed plans](archive/README.md) record earlier
source layouts and PR scopes. Reports under `docs/reports/` retain their
own measured baselines; their old priorities and validation results are not
current implementation instructions or fresh test results.
