# DGCL Wire Compatibility / DGCL wire互換性

## 日本語

### 目的とclaim境界

この文書は、EPIA2 alpha.2作業で追加されたDGCL wire surfaceと、従来のv1 field/schemaをどのように保つかを整理する。schema fixtureへの適合は、すべての旧consumer実装が未知field/enumを受け入れることの証明ではない。strict decoderが未知fieldを拒否するか、consumerごとの確認が必要である。

public fieldの削除・renameは行わない。新しい意味は独立したschema version、optional additive field、または明示的なcompatibility projectionで導入する。既存fieldの意味を別の段階・権限・証拠へ読み替えない。

### 維持するv1 surface

以下のlegacy fixtureをDraft 2020-12 schemaで検証する。

- `fixtures/dgcl-legacy-pipeline-v1.json` — pipeline v1の旧required fieldだけを含む。新`implementation_closure` propertyはoptionalであり、旧shapeを要求しない。
- `fixtures/dgcl-legacy-connection-plan-v1.json` — `coding_requirement_id`、`coding_task_id`、target bindingなどの新action-scoped fieldを含まない旧connection plan。
- `fixtures/dgcl-legacy-connection-report-v1.json` — `connection_graph`および`runtime_trace`を含まない旧connection report。
- `fixtures/dgcl-checkpoint-v1-legacy.json` — event phase fieldのないcheckpoint v1。
- `fixtures/dgcl-checkpoint-v2-action-started.json` — phase discriminatorを必須にしたcheckpoint v2。

`CodingConnectionPlan`の新action-binding fieldはserde defaultでoptionalである。`CodingConnectionReport.connection_graph/runtime_trace`とpipeline closureはadditiveで、v1 schemaのrequired fieldへ追加しない。旧consumer向けschema compatibilityは確認するが、Rustの古い`deny_unknown_fields` decoder互換までは確認済みとしない。

### checkpoint v1とv2

`epistesys-dgcl-checkpoint.v1`のwire serializationは従来のfield集合を保つ。`event_kind=legacy_unclassified`はv1 serialization時に出力せず、旧recordのdeserialize時だけdefaultとして扱う。新しいevent phaseを使うcheckpointは`epistesys-dgcl-checkpoint.v2`にし、`event_kind`を必須にする。

v2 phaseはActionStarted、ActionObserved、CandidatePrepared、CandidateSendAttempted、SinkDeliveryObservedを分ける。`delivery`はstageに付随するoutcomeのままで、host deliveryやcompletionを意味するfieldへ読み替えない。v1/v2を同一checkpoint chain内で混在させない。legacy historyを新phase chainへ移す場合は新しいledger/headを用意し、旧chainを暗黙に再解釈しない。

### 新しい独立surface

Standalone candidate、repair loop、lifecycle lease、trusted checkpoint head、host output stage ledgerはそれぞれ別schema/shapeを持つ。Standalone candidateは正確なbytesとtop-level schema versionを保存するが、Requirementごとの意味実現を`NotObserved`のまま示し、hostのprecommit/post-send/sink/durable-replayを勝手にverifiedへ昇格しない。

Repair reportのtimeout後`may_have_applied`は、最後にreceipt検証済みsnapshotと実filesystemが異なる可能性を示す。RepairLoopの候補やadapter署名はhost filesystem confinement、Codex callback、hard process cancellationの証明ではない。

Host output stageはPreCommitCandidate → PostSendObserved → SinkDeliveryObservedの順序を別々のstage-scoped receiptで検証する。これはcontract adapterであり、実hostがreceiptを発行したrun、実sink delivery、durable replayと同一ではない。

### 検証状況

追加adapterは`epistesys-dgcl-consumer-view.v1/v2`を明示選択する。strict v1 decoderはv2・未知field・trueのcompletion/send/commit boolを拒否する。v2はreported stateを表示するだけでpermissionを生成しない。package finalizerの`consumer_views`は両viewを提供する。Program IRの`structural_distillation`、finalizationの`snapshot/consumer_views`、repairの`journal`、RepairRequestの`repository_root_digest`、validationの`closure_claims`はadditiveである。package resumeは別schema `epistesys-dgcl-package-resume.v1`を使う。

legacy pipeline/connection/checkpoint fixture、新pipeline、repair request/report、standalone candidate、host stage ledgerをDraft 2020-12 schemaで局所検査する。strict Rust decoder、remote CI、実Codex host pickupは別の検証対象であり、未観測ならそのまま記録する。

## English

### Purpose and claim boundary

This document records how the DGCL wire surfaces added in the Epistesys alpha.2 work preserve prior v1 fields and schemas. Passing schema fixtures does not prove that every legacy consumer accepts unknown fields or enum values. Strict decoders may reject them, and consumer-specific behavior must be checked separately.

No public field is removed or renamed. New meanings are introduced through a separate schema version, optional additive fields, or an explicit compatibility projection. An existing field is not reinterpreted as a different stage, authority, or evidence class.

### Preserved v1 surfaces

The following legacy fixtures are validated against the Draft 2020-12 schema:

- `fixtures/dgcl-legacy-pipeline-v1.json` contains only the former required pipeline v1 fields. The new `implementation_closure` property is optional and is not required by the old shape.
- `fixtures/dgcl-legacy-connection-plan-v1.json` omits the new action-scoped fields such as `coding_requirement_id`, `coding_task_id`, and target binding.
- `fixtures/dgcl-legacy-connection-report-v1.json` omits the new `connection_graph` and `runtime_trace` properties.
- `fixtures/dgcl-checkpoint-v1-legacy.json` is a checkpoint v1 record without an event phase.
- `fixtures/dgcl-checkpoint-v2-action-started.json` is a checkpoint v2 record with the required phase discriminator.

New action-binding fields on `CodingConnectionPlan` are optional through serde defaults. `CodingConnectionReport.connection_graph/runtime_trace` and the pipeline closure are additive and are not added to the v1 schema's required fields. Schema compatibility for legacy shapes is checked, but compatibility with every old Rust `deny_unknown_fields` decoder is not claimed.

### Checkpoint v1 and v2

The `epistesys-dgcl-checkpoint.v1` wire serialization preserves the prior field set. `event_kind=legacy_unclassified` is omitted when serializing v1; it is only a default when deserializing an older record. Checkpoints using new event phases use `epistesys-dgcl-checkpoint.v2`, where `event_kind` is required.

V2 phases distinguish ActionStarted, ActionObserved, CandidatePrepared, CandidateSendAttempted, and SinkDeliveryObserved. `delivery` remains the outcome associated with a stage and is not reinterpreted as host delivery or completion. V1 and v2 records are not mixed in one checkpoint chain. Migrating legacy history to a new phase chain requires a new ledger/head and does not silently reinterpret the old chain.

### New independent surfaces

Standalone candidates, repair loops, lifecycle leases, trusted checkpoint heads, and host output stage ledgers each have their own schema/shape. A standalone candidate captures exact bytes and a top-level schema version but keeps per-requirement semantic realization `NotObserved`; it never promotes host precommit/post-send/sink/durable-replay to verified by itself.

`may_have_applied` after a repair timeout indicates that the actual filesystem may differ from the last receipt-verified snapshot. A repair proposal or adapter signature is not proof of host filesystem confinement, Codex callbacks, or hard process cancellation.

Host output stages are verified with separate stage-scoped receipts in the order PreCommitCandidate → PostSendObserved → SinkDeliveryObserved. This is a contract adapter, not the same as an observed real-host receipt, real sink delivery, or durable replay.

### Verification status

The added adapter explicitly selects `epistesys-dgcl-consumer-view.v1/v2`. The strict v1 decoder rejects v2, unknown fields, and true completion/send/commit booleans. V2 displays reported state without generating permission. Package finalization provides both under `consumer_views`. Program IR's `structural_distillation`, finalization's `snapshot/consumer_views`, repair's `journal`, RepairRequest's `repository_root_digest`, and validation's `closure_claims` are additive. Package resume uses its own `epistesys-dgcl-package-resume.v1` schema.

Legacy pipeline/connection/checkpoint fixtures, the new pipeline, repair request/report, standalone candidate, and host stage ledger are locally checked against the Draft 2020-12 schema. Strict Rust decoder behavior, remote CI, and real Codex host pickup are separate verification targets and remain explicitly unobserved when not run.
