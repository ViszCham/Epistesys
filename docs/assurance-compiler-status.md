# Epistesys Assurance-Compiler review status / Assurance-Compiler監査状態

## 日本語

### EPRの限定source監査：昇格前の記録

2026-10-27 JSTの[EPR観測記録](../validation/epr-assurance-observations-2026-10-27.json)は、初期の中間観測を`observations`に残し、後続の5単位を`current_source_review`として区別している。後者のsource commitは`ea4dfcdfffac90261305b711d374ed2ae383e6a6`である。これは`6.3.2-alpha.2`から次の候補を検査している段階の記録であり、alpha.3への昇格・配布・host activationの成功記録ではない。

対象は、それぞれ16KiB以下の完全なRust source単位である。ファイル単位の選択は、その依存module、Cargo設定、全repositoryの解析が完了したことを意味しない。

| unit | UTF-8 bytes | status | diagnostics | evidence |
| --- | ---: | --- | ---: | --- |
| `lc631-process-supervisor/src/bounded.rs` | 16,346 | `blocked_with_diagnostics` | 9 | `missing_or_unknown` |
| `lc631-process-supervisor/src/unix.rs` | 6,017 | `blocked_with_diagnostics` | 6 | `missing_or_unknown` |
| `lc631-process-supervisor/src/filesystem.rs` | 8,463 | `blocked_with_diagnostics` | 13 | `missing_or_unknown` |
| `lc631-analysis/src/production_connection.rs` | 15,635 | `needs_evidence` | 0 | `missing_or_unknown` |
| `lc631-cli/src/effect_error.rs` | 5,567 | `blocked_with_diagnostics` | 8 | `missing_or_unknown` |

4単位は`blocked_with_diagnostics`、1単位は`needs_evidence`である。diagnosticsが0でも、証明・意味的正しさ・接続完成・承認にはならない。各単位のSHA-256、input/graph digest、replay reference、未充足のevidence状態はJSON記録に保持している。doctorはadapter facadeのhealth観測として一度実行されたもので、sourceや実行の正しさを保証しない。

C3の時刻修正後には、`lc631-analysis/src/file_repair.rs`の完全な13,817-byte単位も追加で検査した。`clock_followup_review`はcommit `4ec1f3f2b41bc3d775f11cc3e84e580d80fa8610`、SHA-256 `aa1defd76f3d4b5cf1462e92a349fdaf7a084e679c5a413f5d0d6837d0d861b7`へ束縛され、結果は`blocked_with_diagnostics`、diagnostics=2、evidence=`missing_or_unknown`である。大きな`repair_loop.rs`を断片化して監査済みとはしていない。Windows／Linuxの実修復・時刻試験は別の実行証拠であり、補助監査の欠落を消さない。

上の結果は**記録されたsource bytesにのみ**対応する。その後の修正、version metadata更新、配布物、別のcommitへ自動的に移転しない。最終snapshotではsource hashの一致を再確認し、変わった単位を過去の結果で監査済みと扱わない。source変更後の最終release判定、Cargo実行、OS別観測、remote CI、packageとhostの確認は別の証拠である。現在の観測を、それらの完了へ先回りして昇格させない。

### EPI-05の履歴：6.3.2-alpha.1

以下は初回clone時の履歴であり、現在のsourceへの再監査結果ではない。EPI-05でAssurance-Compiler doctorを一度実行し、adapter healthは`ok=true`、deferred capabilitiesは`none`だった。次の16KiB以下の完全なRust単位をsource textとして監査した。

| unit | status | diagnostics | evidence |
| --- | --- | ---: | --- |
| `lc631-receipt-kernel/src/lib.rs` | `blocked_with_diagnostics` | 27 | `missing_or_unknown` |
| `lc631-core/src/lib.rs` | `blocked_with_diagnostics` | 11 | `missing_or_unknown` |
| `lc631-world/src/lib.rs` | `blocked_with_diagnostics` | 9 | `missing_or_unknown` |
| `lc631-wire/src/lib.rs` | `blocked_with_diagnostics` | 5 | `missing_or_unknown` |

これらは固定された当時のsource textに対するbounded static reviewである。status、diagnostics、evidence gapをそのまま保持し、証明、compiler truth、runtime safety、repository-wide coverage、host activation、commit/push許可、release approvalへ昇格させない。EPI-05では`lc631-cli`、`lc631-analysis`、`lc631-media`、`lc631-host`、`lc631-accelerator`の大きな単位、Cargo全体、外部backend、配布binaryは監査範囲外だった。後続のEPRで一部のsource単位を選択しても、それらの全体範囲が閉じたことにはならない。Cargo check/test/clippy/buildの成功は別の実行証拠である。

## English

### Bounded EPR source review: a pre-promotion record

The [EPR observation record](../validation/epr-assurance-observations-2026-10-27.json), dated 2026-10-27 JST, retains initial intermediate observations in `observations` and separates five later units in `current_source_review`. The latter records source commit `ea4dfcdfffac90261305b711d374ed2ae383e6a6`. This is a record of examining the next candidate from `6.3.2-alpha.2`, not successful alpha.3 promotion, distribution, or host activation.

Each selected complete Rust source unit is at most 16 KiB. Selecting a complete file does not mean its dependent modules, Cargo configuration, or the whole repository have been analyzed completely.

| unit | UTF-8 bytes | status | diagnostics | evidence |
| --- | ---: | --- | ---: | --- |
| `lc631-process-supervisor/src/bounded.rs` | 16,346 | `blocked_with_diagnostics` | 9 | `missing_or_unknown` |
| `lc631-process-supervisor/src/unix.rs` | 6,017 | `blocked_with_diagnostics` | 6 | `missing_or_unknown` |
| `lc631-process-supervisor/src/filesystem.rs` | 8,463 | `blocked_with_diagnostics` | 13 | `missing_or_unknown` |
| `lc631-analysis/src/production_connection.rs` | 15,635 | `needs_evidence` | 0 | `missing_or_unknown` |
| `lc631-cli/src/effect_error.rs` | 5,567 | `blocked_with_diagnostics` | 8 | `missing_or_unknown` |

Four units are `blocked_with_diagnostics`; one is `needs_evidence`. Zero diagnostics do not establish proof, semantic correctness, connection completion, or approval. The JSON record retains each unit's SHA-256, input/graph digests, replay reference, and unresolved evidence state. Doctor ran once as an adapter-facade health observation; it does not guarantee source or execution correctness.

After the C3 clock correction, the complete 13,817-byte `lc631-analysis/src/file_repair.rs` unit was also reviewed. `clock_followup_review` binds commit `4ec1f3f2b41bc3d775f11cc3e84e580d80fa8610` and SHA-256 `aa1defd76f3d4b5cf1462e92a349fdaf7a084e679c5a413f5d0d6837d0d861b7`; its status is `blocked_with_diagnostics`, diagnostics=2, evidence=`missing_or_unknown`. The larger `repair_loop.rs` was not fragmented into a purported complete review. Actual Windows/Linux repair/clock tests are separate execution evidence and do not erase advisory-review gaps.

These results apply **only to the recorded source bytes**. They do not transfer automatically to later fixes, version-metadata changes, distribution artifacts, or another commit. Recheck source hashes at the final snapshot, and do not treat changed units as reviewed using earlier results. Final post-change release decisions, Cargo execution, OS-specific observations, remote CI, package checks, and host observations require separate evidence. Do not preemptively promote the recorded observations into their completion.

### EPI-05 history: 6.3.2-alpha.1

The following is initial-clone history, not a re-review of current source. EPI-05 ran Assurance-Compiler doctor once; adapter health was `ok=true` with `deferred_capabilities=none`. The following complete Rust units no larger than 16 KiB were reviewed as source text.

| unit | status | diagnostics | evidence |
| --- | --- | ---: | --- |
| `lc631-receipt-kernel/src/lib.rs` | `blocked_with_diagnostics` | 27 | `missing_or_unknown` |
| `lc631-core/src/lib.rs` | `blocked_with_diagnostics` | 11 | `missing_or_unknown` |
| `lc631-world/src/lib.rs` | `blocked_with_diagnostics` | 9 | `missing_or_unknown` |
| `lc631-wire/src/lib.rs` | `blocked_with_diagnostics` | 5 | `missing_or_unknown` |

These are bounded static reviews of the source text pinned at that time. Preserve status, diagnostics, and evidence gaps exactly; do not promote them into proof, compiler truth, runtime safety, repository-wide coverage, host activation, commit/push permission, or release approval. Larger `lc631-cli`, `lc631-analysis`, `lc631-media`, `lc631-host`, and `lc631-accelerator` units, the full Cargo workspace, external backends, and distribution binaries were outside EPI-05's review. Selecting some source units in the later EPR review does not close those larger scopes. Cargo check/test/clippy/build success is separate execution evidence.
