# Academic documentation revision verification

## 日本語

### 範囲と基線

2026-10-04 JST。対象はEpistesys repositoryのREADME、Docs、文書検証script。基線commitは`77c37275d9595e527dfdac8c2767865d34ebccf2`、編集branchは`docs/academic-readability-20261004`。基線のtracked差分はなかった。無関係なworkspaceの未追跡ファイル、local benchmark runner、モデルweight、raw回答、認証情報、個人host設定をコミット対象へ追加しない。

### 編集方針と保持した内容

- 標語を操作的定義へ置き換え、入力・要求・権限・証拠・出力状態を区別した。
- READMEを対象問題、現在状態、制御機構、評価、実行、読者別導線の順へ再編した。
- 詳細はsystem architecture、execution guide、DGCL execution contractへ集約し、条件を長段落から段階別・事前条件別へ分解した。
- source/host/research/distributionの状態、候補と検証済み状態、前身の結果とalpha.2の仮説を区別した。
- 継承元文書へ履歴分類を追加。既存path、互換command/schema ID、実装観測、統計値、残存条件を保持した。
- 日本語前半・英語後半の同内容を維持し、学術的資格や機能保証を表す広告表現を追加しない。

| README構造の記述値 | 改訂前 | 改訂後 |
| --- | ---: | ---: |
| ファイルbyte | 37,724 | 20,592 |
| 文字数 | 30,332 | 15,864 |
| 非heading/table/codeの段落数 | 74 | 33 |
| 段落長中央値（文字） | 252 | 248 |
| 最長段落（文字） | 1,719 | 637 |

構造値は編集補助で、読者の理解度や読了時間を測った結果ではない。詳細を参照文書へ移したため、repository全体の情報量を単純に減らしたという指標でもない。

### 実行した検証

| 検査 | 結果／範囲 |
| --- | --- |
| GB-CC75集計 | 選択12ファイルのhash、65主要QID、167採点ペア、点推定を再計算。値の変更なし |
| 集計回帰 | 11テスト通過 |
| 文書検証 | 17文書・274リンクの言語順、数値表、metric token、相対linkを検査。意味同等性は手動でも再読 |
| Rust corroboration | 変更なしのlc631-core／lc631-wireの20テスト通過 |
| Branding／privacy | tracked Markdownの旧標語なし。改訂active文書に個人absolute host pathなし |
| 差分 | git diff --check通過。Rust implementation、schema、fixtures、version、plugin behaviorを変更しない |

RepoSeiri 1.1のrepository/research queryは、前後summary digest `sha256:fd23d664e90594d10f9740a7d5cc9436e806d442142d8be5e9ede2fc42ed441e`で一致し、9 findings、6 underclaim opportunities、0 overclaim risksを保持した。markdown/conflict coverageは`partial/limit_exceeded`であり全体監査成功とはしない。別のdocs/subtree queryはdigest `sha256:9039587ce0e0ec9a3c0ce19fb72e58be6ffabcfabd17518eb6bb8a518bc64fb7`で一致し、markdown/conflict coverageはcomplete、6 overclaim risks、linter 1 file／0 findingsだった。異なるscopeは同じ証拠として比較しない。linterの同一source-session bindingは確認できずMissing/Unverifiedとして保持する。docs-onlyで実装証拠が不足する指摘を、機能不存在や公開適性の判定へ読み替えない。

Assurance-Compiler doctorを一度実行し、完全な`lc631-wire/src/lib.rs` 6,522 byte、SHA `98a8f212055bb501d26157c02af64fded87f0a6d6d8063f84751b691e2354907`を補助監査した。statusは`blocked_with_diagnostics`、diagnostics 5、evidence `missing_or_unknown`。これはsource textに限る監査であり、Rust実装は変更していない。repository-wide safety、形式証明、host activation、merge approvalではない。

### 未実施・残存条件

独立読者による認知負荷評価、研究効果の新benchmark、全dependency/host/runtimeの監査は未実施。新しいmodel呼出し、再採点、bootstrapの再生成、binary配布、plugin再install、host再起動は本編集で行わない。GitHub commit/push/mergeはユーザーの別途明示指示に基づく操作であり、この文書の検証値や低lossから許可を生成しない。

## English

### Scope and baseline

2026-10-04 JST. Scope: Epistesys README, Docs, and documentation-verification script. Baseline commit `77c37275d9595e527dfdac8c2767865d34ebccf2`; editing branch `docs/academic-readability-20261004`. The baseline had no tracked changes. Exclude unrelated workspace untracked files, local benchmark runners, model weights, raw answers, credentials, and personal host settings from commits.

### Editorial policy and preserved content

- Replace slogans with operational definitions distinguishing input/requirements/authority/evidence/output states.
- Order README by failure model, current status, mechanisms, evaluation, execution, and reader routes.
- Centralize details in architecture/execution/DGCL-contract references; decompose long paragraphs by stage/prerequisite.
- Separate source/host/research/distribution, candidate/validated state, and predecessor results/current-alpha hypotheses.
- Classify inherited records as history; retain paths, compatibility commands/schema IDs, observations, statistics, residual conditions.
- Preserve equivalent Japanese-first/English-second prose without adding credential or guarantee marketing.

| Descriptive README structure | Before | After |
| --- | ---: | ---: |
| File bytes | 37,724 | 20,592 |
| Characters | 30,332 | 15,864 |
| Non-heading/table/code paragraphs | 74 | 33 |
| Median paragraph characters | 252 | 248 |
| Maximum paragraph characters | 1,719 | 637 |

These are editorial aids, not measured reader comprehension/reading time. Moving details into references also prevents interpreting them as simple repository-wide information reduction.

### Performed verification

| Check | Result/scope |
| --- | --- |
| GB-CC75 aggregates | Recompute hashes of 12 selected files, 65 primary QIDs, 167 scored pairs, point estimates; values unchanged |
| Aggregate regression | 11 tests passed |
| Documentation | Check 17 documents/274 links for language order, numeric tables, metric tokens, relative links; manually reread semantic parity |
| Rust corroboration | 20 unchanged lc631-core/lc631-wire tests passed |
| Branding/privacy | No withdrawn slogans in tracked Markdown; no personal absolute host paths in revised active documents |
| Diff | git diff --check passed; no Rust implementation/schema/fixture/version/plugin-behavior changes |

RepoSeiri 1.1 repository/research queries share bracketing summary digest `sha256:fd23d664e90594d10f9740a7d5cc9436e806d442142d8be5e9ede2fc42ed441e`, retaining nine findings/six underclaim opportunities/zero overclaim risks. Markdown/conflict coverage is `partial/limit_exceeded`, not complete repository auditing. Separate docs/subtree queries share digest `sha256:9039587ce0e0ec9a3c0ce19fb72e58be6ffabcfabd17518eb6bb8a518bc64fb7`, complete markdown/conflict coverage, six overclaim risks, linter one file/zero findings. Different scopes are not interchangeable evidence. Linter same-source-session bindings remain Missing/Unverified. Insufficient implementation evidence in docs-only scope is not proof of absent functionality or publication fitness.

Ran Assurance-Compiler doctor once and reviewed complete `lc631-wire/src/lib.rs`, 6,522 bytes, SHA `98a8f212055bb501d26157c02af64fded87f0a6d6d8063f84751b691e2354907`. Retain `blocked_with_diagnostics`, five diagnostics, `missing_or_unknown` evidence. Source-text-only review; Rust implementation unchanged. Not repository-wide safety, formal proof, host activation, or merge approval.

### Unperformed work and residual conditions

No independent-reader cognitive-load study, new research-effect benchmark, or complete dependency/host/runtime audit. This edit performs no new model calls, regrading, uncertainty regeneration, binary distribution, plugin reinstall, or host restart. GitHub commit/push/merge relies on the user's separate explicit instruction, not on these checks or low loss.
