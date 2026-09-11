# V631 Commit and Review Split Plan

## 日本語

この文書はcommit権限ではない。V631-00〜32の大規模diffを、後続の明示commit指示で依存順にレビュー可能な単位へ分けるためのplanである。

1. `V631-00..03`: baseline、manifest、workspace、独立CI、compat guard
2. `V631-04..06`: core closure、receipt ownership、host authority
3. `V631-07..15`: TranslationLoss witness、validator、legacy paired shadow
4. `V631-16..20`: exact world budget、distinct world、frame、WorldArena
5. `V631-21..24`: AcceleratorBroker、scheduler、completion/fault/privacy contract
6. `V631-25..30`: CubeCL GPU 7-lane実装、CPU parity、telemetry、ResidualRisk
7. `V631-31..32`: media source authority、method/backend registry、EventGraph boundary
8. `presentation`: wire、CLI、skills、schema、roadmap、implementation record

各commit候補は直前依存だけを含み、root v6.3.0 protected filesを変更しない。Cargo.lockは最初にworkspaceと一緒に導入し、後続commitではdependency変更時だけ更新する。

---

## English

This document is not commit authority. It plans dependency-ordered review units for the large V631-00 through V631-32 diff if a later user message explicitly authorizes commits.

1. `V631-00..03`: baseline, manifest, workspace, independent CI, and compatibility guard
2. `V631-04..06`: core closure, receipt ownership, and host authority
3. `V631-07..15`: TranslationLoss witnesses, validators, and legacy paired shadow
4. `V631-16..20`: exact world budget, distinct worlds, frames, and WorldArena
5. `V631-21..24`: AcceleratorBroker, scheduler, and completion/fault/privacy contracts
6. `V631-25..30`: CubeCL seven-lane GPU implementation, CPU parity, telemetry, and ResidualRisk
7. `V631-31..32`: media source authority, method/backend registry, and EventGraph boundary
8. `presentation`: wire, CLI, skills, schemas, roadmap, and implementation record

Each candidate commit contains only its immediate dependencies and leaves the protected root v6.3.0 files unchanged. Cargo.lock enters with the initial workspace and changes later only when dependencies change.
