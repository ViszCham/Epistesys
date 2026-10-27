# Epistesys verification limits and residual conditions

## 日本語

この文書は`6.3.2-alpha.3`の未観測・未実証範囲を、次に必要な確認と対応付けます。実装経路の存在、宣言profileの局所検証、研究効果、配布、実host観測は別の状態です。[用語と境界](system-architecture.md)と[alpha.3検証記録](../validation/epr-release-verification-2026-10-27.md)を参照してください。

### 残存条件

| 領域 | 現在の境界 | 必要な確認 |
| --- | --- | --- |
| 研究効果 | hallucination containment、制約保持、長期整合性、risk–coverage、運用範囲の拡張は未実証 | 独立gold、未使用holdout、同等予算、反復・欠測・劣化を含む評価 |
| 構文・意味 | controlled grammar外の完全理解は対象外。exact source roundtripは意味的等価性ではない | 対応文法別の独立評価、configured parser/modelの実観測 |
| Codingの要求抽出と受入条件 | 完了判定は抽出・登録済み要求と計画に限定。未抽出要求や受入oracleの不足が残りうる | 独立goldによる要求recall、target bindingとoracle妥当性、偽保留・誤受入の評価 |
| 権限・証拠 | 外部receiptとcaller/host trust rootに依存。local署名は実hostや人間の独立性を証明しない | issuer/key provenance、失効・scope・snapshotの確認 |
| Coding・build | 宣言profileの検証・修復・再開を実装。任意build world、opaque build script/proc macro、OS sandboxは未保証 | dependency/build条件と実行環境の個別検証 |
| Codingの修復と接続観測 | 既存Rust fileに呼出側候補を適用。CLIは1件のrequest、SDK上限は4回。接続証拠は登録run/target/経路に限定 | 候補作成主体と各試行、追加経路、受入条件を個別に確認する |
| 再開・永続化 | journalと別管理headの不一致を検出。joint rollback、同一administrator改変、全OSのdirectory fsyncは範囲外 | 運用上のhead分離、key保護、OS別durability試験 |
| GPU | local materializationと数値経路を実装。parity、readback、timing、overlap、occupancy、device-lossは別観測 | 対象backend・device・workload別の測定 |
| Media・外部解析 | source/backend/model/license/method/budgetの条件付き候補・観測 | parser、vision、ASR等の登録・license・receipt・実backend試験 |
| 出力・host | candidate、send、sink、replayのadapterを実装。実callback／deliveryはPendingHostObservation | fresh host pickup、stage receipt、実sink、revocationの観測 |
| 配布 | source-only。sourceから生成したbinaryと配布監査済みbinaryは異なる | absolute build path監査、別cwd起動、clean-machine条件、配布検証 |
| 補助監査 | local tests・hash・Assurance statusは限定範囲の材料 | 対象revision、未監査単位、独立runtime観測の確認 |

### 前身の評価を現alphaへ移転しない

[GB-CC75](research/gb-cc75-study.md)は前身の探索的観測です。主要65 QID／採点済み167対応ペアへの選択、機構を含むJudge、未確定の生成設定・予算、未同梱raw回答という制約があります。公開CSVの再計算は独立再採点や再実験ではありません。[方法論監査](research/gb-cc75-adversarial-audit.md)の実験課題を文書変更だけで解決したとはしません。

初回alpha.1はv6.3.1のcloneでした。alpha.2はDGCLのcontrolled grammar→Program IR→TL→要求別completion、実Cargo観測、署名evidence、実ファイル修復、journal/head付き再開、構造蒸留の下流接続、consumer negotiationを追加しています。検証範囲は[DGCL実行契約](dgcl-operating-profile-and-closure.md)に限定します。

### 状態・数値の解釈

- `Clarify`、parse defect、Program IR／host output未bindingは、入力や証拠の未解決状態です。
- world doctorの2,048 evaluationはlocal materializationであり、2,048 LLM呼出しではありません。
- schema適合は証拠の真正性を、GPU利用率はoccupancyを、source hashは意味的正確性を表しません。
- commit、push、tag、plugin install、host reloadは、解析・検証結果から自動的には許可されません。

ResearchEvaluation=`PendingNoCorpus`とhost観測=`PendingHostObservation`を維持します。今後はsource/build条件、schema互換性、receipt/replay分離、独立評価をそれぞれ再確認し、Epistesys-7の計画と現alphaの成果を分離します。

### alpha.3でも残る境界

alpha.3は以下の不具合対策と明示的な制限を追加します。局所回帰の成功を全gate成功、研究効果、全host強制制御や欠陥ゼロへ昇格させません。alpha.2の履歴的検証とalpha.3の検証は別に記録します。

| 対象 | 修正範囲と、なお残る条件 |
| --- | --- |
| Authorityとreceipt | 元の検証済みsource/span/parent/issuerを保持し、current receiptを`[issued, expires)`で検査する。caller/clock、key custody、実host origin、失効情報自体の真正性は別契約 |
| 文書領域 | pin済みCommonMark解析はcode/quote等の権限source化を抑止するeligibility検査。完全な指示意味理解でもGrant発行でもない |
| ProductionConnection | 登録direct driverの直接public API呼出しとpin済みassertionに限定。generic `--lib`、推移的call graph、全利用経路、独立oracle、hermeticityの証明ではない |
| Snapshotと資源 | Rust/Cargo入力列挙・source snapshot・streaming/temp/output quotaを持つ。非Rust include、opaque script、環境、全外部依存は完全なbuild closureへ含まれず、上限超過や不足は明示失敗／未知のまま |
| Filesystem | Windowsの祖先handle固定と排他的file、Linuxの同一owner・access ACLなしprofileとatomic replacement、private backupを扱う。他Unix更新はUnsupported。全ACL/owner変更profile、特権主体へのsandbox、全OS durabilityは未保証 |
| 期限切れ履歴 | authenticな履歴の`historical_head_matched`とcurrent権限を分離。fresh権限・独立binding・全validation後に明示renewalする。joint rollback対策や期限切れ権限の復元ではない |
| Memory | `memory_limit_kind`でconfigured Windows Job-wide committed-memory制限と他profileの未観測を区別する。単一processの観測、temp quota、有限出力を任意の集約RSS保証へ変換しない |
| GraphとGPU | `token_bindings`によるtoken/syntax/span対応とCPU/GPU summary・top-16 parityを検査する。意味同値、全vector同値、occupancy／hardware saturationは別対象 |
| Media→TL | source/backend/model/method provenanceとkind/family整合性を改善するが、method-level semantic validation、source-rootの意味、同定・感情・意図、完全な文字起こしは未検証 |
| 旧consumer | JSON field維持は全decoderの互換保証ではない。`historical_head_matched`とrenewal filenameを知らない旧readerは拒否しうる。current authorityへの暗黙変換はしない |

## English

This document maps unobserved/unproven scope in `6.3.2-alpha.3` to required checks. Implementation presence, declared-profile local verification, research effects, distribution, and real-host observation are distinct states. See [terms/boundaries](system-architecture.md) and [alpha.3 verification](../validation/epr-release-verification-2026-10-27.md).

### Residual conditions

| Area | Current boundary | Required checks |
| --- | --- | --- |
| Research effects | Hallucination containment, constraint preservation, long-horizon integrity, risk–coverage, and operating-range expansion are unproven | Independent gold, unused holdouts, matched budgets, repetitions/missingness/regressions |
| Syntax/semantics | Complete interpretation outside controlled grammar is excluded; exact roundtrip is not semantic equivalence | Independent grammar-specific evaluation and observed configured parsers/models |
| Coding extraction and acceptance conditions | Completion covers extracted/registered requirements and plans; unextracted requirements and incomplete oracles may remain | Requirement recall against independent gold, target-binding/oracle validity, false-hold/false-acceptance evaluation |
| Authority/evidence | Depend on external receipts/caller-host trust roots; local signatures do not prove real-host or human independence | Issuer/key provenance, revocation, scope, snapshot verification |
| Coding/build | Implement declared-profile validation/repair/resume, not arbitrary build worlds, opaque scripts/proc macros, or OS sandbox guarantees | Individual dependency/build/environment checks |
| Coding repair and connection observations | Apply caller candidates to existing Rust files; CLI supplies 1 request, SDK ceiling is 4 attempts; connection evidence covers registered run/target/route | Individually inspect candidate authorship/attempts, additional paths, and acceptance conditions |
| Resume/persistence | Detect journal/separate-head mismatch; joint rollback, same-administrator mutation, and universal directory fsync are excluded | Operational head separation, key custody, OS-specific durability tests |
| GPU | Implement local materialization/numeric paths; parity/readback/timing/overlap/occupancy/device-loss are separate observations | Backend/device/workload-specific measurement |
| Media/external analysis | Conditional source/backend/model/license/method/budget-bound candidates/observations | Parser/vision/ASR registration, licensing, receipts, actual backend tests |
| Output/host | Implement candidate/send/sink/replay adapters; real callbacks/delivery remain PendingHostObservation | Fresh pickup, stage receipts, real sinks, revocation observations |
| Distribution | Source-only; locally generated and distribution-audited binaries differ | Absolute build-path audits, other-cwd startup, clean-machine conditions, distribution checks |
| Advisory audits | Local tests/hashes/Assurance statuses are bounded material | Revision, unreviewed units, independent runtime observations |

### No transfer of predecessor evaluation to current alpha

[GB-CC75](research/gb-cc75-study.md) is exploratory predecessor evidence. Limitations include selection to 65 primary QIDs/167 scored pairs, mechanism-containing Judges, unresolved generator settings/budgets, and excluded raw answers. Public-CSV recomputation is neither independent regrading nor experimental replication. Documentation alone does not resolve experiments in the [methodological audit](research/gb-cc75-adversarial-audit.md).

Initial alpha.1 was a v6.3.1 clone. Alpha.2 adds controlled grammar→Program IR→TL→per-requirement completion, real Cargo observations, signed evidence, file repair, journal/head-backed resume, downstream structural distillation, and consumer negotiation. Verification remains scoped by the [DGCL contract](dgcl-operating-profile-and-closure.md).

### Interpreting states and quantities

- `Clarify`, parse defects, and unbound Program IR/host output denote unresolved input/evidence.
- World-doctor 2,048 evaluations are local materialization, not 2,048 LLM calls.
- Schema validity is not evidence authenticity; GPU utilization is not occupancy; source hashes are not semantic accuracy.
- Parsing/verification results do not automatically authorize commit/push/tag/plugin install/host reload.

Retain ResearchEvaluation=`PendingNoCorpus` and host observation=`PendingHostObservation`. Recheck source/build conditions, schemas, receipt/replay separation, and independent evaluation separately; distinguish Epistesys-7 plans from current-alpha results.

### Boundaries remaining in alpha.3

Alpha.3 adds the following defect mitigations and explicit limits. Passing bounded regressions is not elevated to all-gate success, research effects, host-wide enforcement, or zero defects. Historical alpha.2 and alpha.3 verification are recorded separately.

| Subject | Remediation scope and remaining conditions |
| --- | --- |
| Authority/receipts | Retain original verified source/span/parent/issuer and check current receipts over `[issued, expires)`. Callers/clocks, key custody, real-host origin, and authenticity of revocation information remain separate contracts |
| Document regions | Pinned CommonMark parsing checks eligibility against code/quote authority-source promotion; it is neither complete instruction interpretation nor Grant issuance |
| ProductionConnection | Limited to a registered direct driver's direct public-API call and pinned assertion; not proof from generic `--lib`, transitive call graphs, all usage paths, independent oracles, or hermeticity |
| Snapshots/resources | Bounded Rust/Cargo input enumeration, source snapshots, and streaming/temporary/output quotas exist. Non-Rust includes, opaque scripts, environments, and all external dependencies are not a complete build closure; excess/missing inputs remain explicit failures or unknowns |
| Filesystem | Windows ancestor-handle pinning/exclusive files, Linux same-owner/no-access-ACL atomic replacement, and private backups have supported profiles. Other Unix updates are Unsupported; arbitrary ACL/owner changes, privileged-principal confinement, and universal durability are unproven |
| Expired history | Separate authentic `historical_head_matched` history from current authority; explicit renewal follows fresh authority, independent binding, and all validation. This neither defeats joint rollback nor restores expired permission |
| Memory | `memory_limit_kind` distinguishes configured Windows Job-wide committed-memory limits from unobserved limits in other profiles. Single-process observations, temporary quotas, and finite output are not arbitrary aggregate RSS guarantees |
| Graph/GPU | Check token/syntax/span correspondence through `token_bindings` and CPU/GPU summary/top-16 parity. Semantic equivalence, full-vector parity, occupancy, and hardware saturation remain separate |
| Media→TL | Improve source/backend/model/method provenance and kind/family consistency; method-level semantic validation, source-root meaning, identity/emotion/intent, and perfect transcription remain unvalidated |
| Legacy consumers | Preserving JSON fields does not ensure every decoder's compatibility. Old readers may reject `historical_head_matched` and renewal filenames; no implicit conversion to current authority is allowed |
