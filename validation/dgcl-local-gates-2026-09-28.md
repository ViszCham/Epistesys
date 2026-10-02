# DGCL local validation — 2026-09-28

## 日本語

対象は`dgcl-completion`作業tree、base commit `60cc71db2ba47517bbd209894033f9b55b3708f8`、source version `6.3.2-alpha.1`です。未commitのsourceに対するローカル観測であり、release approvalではありません。

| 検査 | 観測 |
| --- | --- |
| `cargo fmt --all --check` | 成功 |
| `cargo check --workspace --locked` | 成功 |
| `cargo test --workspace --no-default-features --locked` | 成功 |
| `cargo test --workspace --locked` | 成功 |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | 成功 |
| `cargo build --release --locked` | 成功。MSVC linker stdoutの非失敗warningあり |
| `git diff --check` | 成功。WindowsのLF→CRLF予告warningは別記録 |
| 実Stanza日英混在CLI出力 | 2言語領域・2候補binding、JSON Schema Draft 2020-12に適合 |
| 実binary connection CLI出力 | `observed`、同schemaに適合。host deliveryは未観測 |
| RepoSeiri 1.1 research profile | 文書更新途中の再監査summary source-session digest `sha256:83856a8e76183023373727878ec1efc207bc503a7a875badc048b18e33565625`、linter finding 0。route解析はlocal advisory |
| finalizer事前検査のfail-first回帰 | `--receipt-root`欠落時にcheckpointが書かれる失敗を再現し、修正後に成功。`verify/finalize`は明示`--execute`なしで実行しない |

Assurance-Compiler doctorは`ok=true`でした。追加の完全module 5件はすべて16KiB以下で、`dgcl_gold.rs`、`language_worker.rs`、`dgcl_pipeline.rs`、`closure_connection.rs`が`needs_evidence`、`closure_resume.rs`が`blocked_with_diagnostics`（1件）です。これは静的補助監査に限られ、独立証拠・runtime safety・repo全体の結論ではありません。以前のbounded review結果も[実装状況](../docs/dgcl-implementation-status-2026-09-28.md)に保持します。

必須hard gateは未達です。独立裁定済み日英goldがなく、DGCL-27のholdout精度・coverageは測定不能です。最終host出力callbackは未観測で、finalizerは`hold_host_output_unbound`を返します。したがってDGCL-30/31、version昇格、release-ready、commit/push/mergeは今回成立したとは記録しません。

## English

Scope is the `dgcl-completion` working tree, base commit `60cc71db2ba47517bbd209894033f9b55b3708f8`, and source version `6.3.2-alpha.1`. These are local observations over uncommitted source, not release approval.

| Check | Observation |
| --- | --- |
| `cargo fmt --all --check` | Passed |
| `cargo check --workspace --locked` | Passed |
| `cargo test --workspace --no-default-features --locked` | Passed |
| `cargo test --workspace --locked` | Passed |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | Passed |
| `cargo build --release --locked` | Passed with a non-failing MSVC linker-stdout warning |
| `git diff --check` | Passed; Windows LF→CRLF notice is recorded separately |
| Actual mixed Japanese/English Stanza CLI output | Two language regions and two candidate bindings; valid against JSON Schema Draft 2020-12 |
| Actual binary connection CLI output | `observed` and valid against the same schema; host delivery not observed |
| RepoSeiri 1.1 research profile | During document updates, the refreshed summary source-session digest was `sha256:83856a8e76183023373727878ec1efc207bc503a7a875badc048b18e33565625` with zero linter findings. Route analysis is local advisory |
| Fail-first finalizer preflight regression | Reproduced a checkpoint write with missing `--receipt-root`, then passed after repair. `verify/finalize` do not execute without explicit `--execute` |

Assurance-Compiler doctor returned `ok=true`. Five additional complete modules were each below 16 KiB: `dgcl_gold.rs`, `language_worker.rs`, `dgcl_pipeline.rs`, and `closure_connection.rs` returned `needs_evidence`, while `closure_resume.rs` returned `blocked_with_diagnostics` (one diagnostic). This is bounded static advisory review, not independent evidence, runtime safety, or a repository-wide conclusion. Earlier bounded-review results remain in the [implementation status](../docs/dgcl-implementation-status-2026-09-28.md).

Required hard gates remain unmet. Without independently adjudicated Japanese/English gold, DGCL-27 holdout accuracy and coverage cannot be measured. The final host-output callback has not been observed, and finalization returns `hold_host_output_unbound`. Accordingly, this record does not claim DGCL-30/31, version promotion, release readiness, or completion-triggered commit/push/merge.
