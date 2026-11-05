# Epistesys 文脈再構成・多論理探索・TL／創造性統合 改訂設計と実装ロードマップ

## 日本語

### 0. 固定状態・合意・読了範囲

- 設計日：2026-10-28 JST。基線：Epistesys 6.3.2-alpha.3、commit `1fca5165eca21011bcefd6518d3b3c9b93e17d58`、tree `fa5fffa50a4f512380b4fda2f869e0ff3ad88706`。
- 状態：**設計固定／実装未開始**。本書の完了は、以下の機能の実装完了ではない。次の製品versionは未決定であり、この作業では変更しない。
- 最新の明示回答を採用：**前提の再構成も主目的**とする。元の文脈は比較基準として不変に残すが、探索・提示を常に元文脈優先へ制限しない。必須条件を守り、損失と利得を比較し、推奨解1つ＋重要な代替解を標準にする。
- 原案の目的は、複数の実論理ソルバーを介して変換損失を測り、有用な新構造を獲得し、文脈条件付きの解を改善すること。安全管理・証跡管理はこの目的を支える基盤であり、推論・測定・探索の代用品ではない。
- 全追跡コード・テストと関連設定／schemaを分担通読：**205ファイル、2,337,622 bytes、62,905行、Rust 145ファイル**。全件読後hash一致。[読了台帳](tlcx-source-reading-ledger-2026-10-28.json)に相対path・working-tree SHA-256・担当を保存。外部依存の実装、生成binary、ignored cache、個人runtime stateはこの「既存コード全体」の対象外。通読は全経路実行・未知欠陥ゼロ・安全性証明ではない。
- 原案や非公開の背景資料そのものは本repoへ複製しない。由来・権利・公開境界は[移行契約](migration-contract.md)を維持する。
- 実装時は各sliceを負例先行→最小実装→対象検証→差分批評→出口記録の順で進める。未達の必須項目をoptionalへ変更して「完成」としない。

2026-10-29追補：ユーザー指示で実装を中断し、[TL／S_Creative測定契約v1／finite-formal-v1.4](tlcx-tl-screative-measurement-contract-v1-2026-10-29.md)を先に固定する。本書の初期状態表記は設計固定時の履歴であり、現在の状態は[進捗・再開記録](tlcx-implementation-progress-2026-10-28.md)を正本とする。測定定義・計算法・適用・独立検証・選択影響は測定契約を優先する。2026-10-29のv1.1数式監査は現行v1.3と照合し、既に修正済みのF-01〜03は回帰goldenへ、新規の区間二乗とnovelty-policy分離はv1.4追補へ反映した。仕様固定・くすり通知後はセッション全作業を停止し、明示再開まで実装へ戻らない。

### 1. 全体構成と唯一の通常経路

```text
host/user source + explicit context + current authority
  -> bounded ingress / single immutable SourceStore
  -> DeepGrammar / structured context / unresolved interpretations
  -> ContextArena: original anchor + reconstructed premises + hard invariants
  -> actual solver portfolio / evidence-aware model validation
  -> TransformWitness { D, Delta, U, G } + comparison scope
  -> contextual loss/gain frontier + one recommendation + key alternatives
       ^                 |
       |                 +-> geometric scheduler -> new solver/query/branch
       +---- discrete <-> continuous proposal / independent validation
  -> task-specific Program IR / verification or coding repair
  -> exact final rendering / output contract
  -> host precommit / post-send / sink / durable replay
```

既存通常入口は解析pipelineをJSON候補として返すところまでで、実ソルバー／S_Creative選択の通常経路はない。新しい `epistesys-solve` を追加し、facadeから同じ `lc631-runtime::solve` を呼ぶ。旧 `lc631-dgcl-run` 等のwireは互換adapterとして保持し、doctorを本番経路の代用にしない。

新crateは責務が必要な三つに限定する。

| crate／層 | 所有するもの | 所有しないもの |
| --- | --- | --- |
| 新 `lc631-substrate` | Digest32、型付きID、source pool、arena、checked binary codec、予算の値型 | solver、権限、I/O実行 |
| `lc631-core` | context/claim/decisionの共通型、既存Authority型 | scoreからのGrant生成 |
| `lc631-tldg`・`lc631-analysis` | parser／AST／文脈lowering／既存Coding検証・修復 | 任意自然言語の完全理解という主張 |
| `lc631-tl` | 四記録、比較scope、loss計測、legacy defect adapter | 探索scheduler、操作権限 |
| `lc631-world` | 文脈枝、frontier、創造候補、利得評価、幾何的な探索配分 | 現実の事実や実行許可の自動確定 |
| 新 `lc631-solvers` | SMT／ASP／有限モデル等のadapter、query/result protocol | 自由文の真理化、trusted issuer |
| `lc631-process-supervisor` | child／pipe／filesystem／tempの実所有 | 論理上の最適性 |
| 新 `lc631-runtime` | 唯一のorchestrator、予算broker、store、再開、task別consumer配線 | CLI構文やrenderer固有判断 |
| `lc631-accelerator` | 意味のある数値batchの加速、lease/fence、parity | fixtureの数値を意味損失とみなすこと |
| `lc631-media` | source/time/regionに束縛した観測候補 | 感情・意図・本人性等の無根拠な断定 |
| `lc631-host`・`lc631-receipt-kernel` | current authority、exact output、証跡真正性、replay | 仮説前提を権限へ変換すること |
| `lc631-wire`・`lc631-compat`・CLI | bounded DTO、旧schema、表示、引数と入口 | 巨大JSONを内部正本にすること |

依存方向は `CLI -> runtime -> analysis/tldg/world/tl/solvers/host/media`。solverはcore＋supervisor、worldはcore＋tl、tlはcore、core／receipt-kernelはsubstrateを参照する。wireは公開共通型から描画する。runtimeへ下位crateから逆参照しない。RunBudget／Cancellation／Reservationの契約型をsubstrateまたはsupervisorへ、JournalRead／JournalAppend／EvidenceLookup等の小さいportをcoreへ置き、runtimeが実体を所有して借用interfaceを注入する。DecisionArtifact型はcore、runtime/decisionは構築処理。world→tldg／acceleratorの逆依存は禁止し、runtime adapterで橋渡しする。TLCX-01でcargo metadataの許可依存辺試験を導入する。traitは実境界だけに限定し、solver enumと小さい具体実装を優先する。

既存の[アーキテクチャ](system-architecture.md)は現在実装の説明として残す。本書の未来機能を現在のREADMEへ実装済みとして移さない。

### 2. 文脈再構成・目的・比較の契約

`ContextRevision` は元source、親context、premise集合、domain、logic semantics、hard invariants、goal policy、比較probe集合、typed Deltaを参照する。前提には `Given/Observed/Assumed/Derived/Proposed/Refuted/Conflicted` を保持する。証拠を仮定へ変換しても元観測は消さない。

固定必須条件・実行権限・証拠真正性は探索変数にしない。変更可能な前提と、不明で確認を要する前提を明示する。変更案は新ContextIdと適用条件を持ち、元問題を解いたかのように表示しない。文脈の重み、目的、分母を変えて見かけ上の利得を得ることを禁止する。

比較は同じ `ComparisonScope`（問い・基線・尺度・単位・probe・目標policy・domain）内で行う。比較不能な枝には `Incomparable` を返す。数値を出せないことを0としない。複数解が残ればPareto集合を保持し、宣言済みのtask preferenceと安定tie-breakで推奨解を一つ選ぶ。真に比較不能なら「作業上の推奨」と比較不能理由を示し、唯一最適と主張しない。

最適性は `ModelOptimal/BestAmongEvaluated/BudgetLimited/Incomparable`。形式モデルの最適証拠、探索済候補内の優位、時間切れを区別する。創造性を採点する目的で実装変更・ネットワーク取得を勝手に実行しない。

### 3. 四記録と測定の定義

一つの変換は `TransformWitness(D, Delta, U, G)` を持つ。四つは排他的分類ではなく同一変換の四面である。意図的に必須条件を破った場合はDeltaとDの両方に記録する。

- **D**：保存義務の欠損。recon／semantic／temporal／spatialを領域別計測器で扱う。構造欠落・検証失敗・未対応を同じ値に潰さない。
- **Delta**：抽象化、置換、前提の追加／撤回、再結合、表現変更。sourceとtargetの差をtyped editとして記録し、変更の許容根拠を別途検査する。
- **U**：未検証、timeout、unsupported、校正不足、競合、比較不能を保持する。確率を根拠なく割り当てない。
- **G**：同じ基線に対する説明範囲、圧縮、橋渡し、予測、再編成、設計・転移・問題解決の利得。予測利得と実観測利得を分ける。

`Measurement<T>` は `Exact/Bounded/Estimated/Unknown/NotApplicable/Incomparable`。区間、単位、分母、検査範囲、測定器revision、evidence IDを持つ。10個中欠損1・未確定2なら、同重みの二値probeという条件下で欠損率[1/10,3/10]を返す。これは統計的信頼区間でも未抽出要求の網羅率でもない。

元のS_Creative v1/v2の項目を `CreativeAssessment` として復元し、原式のraw scoreと改訂意思決定policyを分ける。原式v2のComplexity Parityは対称なexp式のまま保持し、単純化を異なる扱いにする場合は別FormulaRevisionとする。既定JointParetoでは移行費用と意味保存を満たす単純化利得を分離し、TLを無自覚に二重計上しない。利得・新規性・親和性・移行費用・反例リスクは別componentとして保持する。既知性は固定archive相対の尺度であり、安全な標準手法の再利用を一律に禁止しない。詳細は測定契約を参照する。

失敗経路を原因別に振り分ける：欠落→局所修復、曖昧性→解釈枝、反例→前提／候補再構成、検査器不足→別検証器、反復残差→別表現、予算切れ→条件付き部分回答。経路合成はObligationIdごとの対応で行い、同じ欠損を多重加算せず、利得も重複計上しない。

### 4. 実ソルバーと幾何・蒸留

初期必須profileは、SMT（Boolean/QF_LIA）、ASP（有限ground normal rulesと明示default negation）、有限時間trace、四値の矛盾許容、有限Kripke（modal K、S5の認識、有限直観主義、明示した義務論理profile）、有限非巡回SCM、有限空間関係。無制限の高階／任意自然言語を対応済みとしない。SMTとASPは実外部backendをWindows／Ubuntuで起動し、version・binary・options・licenseをTLCX-00でpinする。その他も実行可能な有限評価器と独立小モデルoracleを要求し、enumだけで完成にしない。

ソルバー間の結果を多数決しない。SAT/UNSATとentailment、brave/cautious consequence、default否定と古典否定、支持／反駁と未観測を区別する。各adapterは入口の翻訳D/Delta/Uと出口の再構成D/Delta/Uを返す。証明を検査できなければ `SolverReported` のままで、proof verifiedへ昇格させない。

幾何は探索スケジューラー専用。実測した状態遷移・検証費用・利得履歴から、階層距離、方向付き費用、文脈依存の局所metricを作る。最低一つの非ユークリッド実装（例：Poincare ball上の階層embedding）と非幾何基線を持ち、ID／列挙順を意味座標にしない。曲率の採用理由、数値domain、NaN／境界、自己距離、再ラベル不変性を試験する。方向付き遷移コストと距離は別型。geometryの予測をTL測定の独立証拠として再利用しない。

schedulerは期待改善・重要なU解消・多様性・費用で次のqueryを選ぶ。飢餓防止枠・deadline・actual concurrency capを設け、幾何から予算や権限を増幅しない。離散→連続→候補→独立validator→離散stateの更新を閉じる。新しい関係は既存edgeへの一致だけでなく、専用検証器の観測によって追加できる。検証に使う正解を候補側が変更しない。

旧256×8 diagnostic contractは互換として保持する。新探索は256の基底探索枠、最大2,048のlive evaluation枠とbounded evictionを持つが、識別子の違いを意味的独立性と偽らない。論理的distinctnessが確認できない枝はその状態を表示し、最低探索数をhard gateにしたprofileでは不足を成功にしない。2,048を超える履歴を全文コピーで積まず、要約・再計算可能な索引・必要な証跡だけを保持する。

### 5. 内部JSONから型付きRust／バイナリへの移行

内部メモリはRustの型付きarenaと共有sourceを正本にする。各関数間で毎回バイナリへserializeする設計にはしない。永続化・IPC・GPU転送・checkpointの境界だけでbinary frameを使う。base64化したbinaryを巨大JSONへ戻してはならない。

現行の具体的な置換点：

| 現行 | 改訂先 |
| --- | --- |
| `BoundaryCell.surface`の文字ごとのString | 共有source＋UTF-8 byte span＋class |
| selected AST／alternatives／contractのdeep clone | arena内AST IDとimmutable view |
| pipeline全体→candidate_text→同じpipelineの外側出力 | 単一artifact＋bounded summary、legacy full DTOは明示要求時 |
| `finalize_with_pipeline -> serde_json::Value` | 型付きPackageFinalizationReport |
| repair/resumeの `from_value(report[...].clone())` | 型付きclaim/snapshotの参照または所有権移動 |
| 全体JSON再serializeによるdigest | versioned canonical binaryをstream hash |
| Stringのdigest/nonce／固定ラベル | Digest32／Nonce16／enum／interned SymbolId |
| JSON journalの毎回全再読 | bounded segment＋検証済head＋再構築可能index |
| stdout／frame／PCMの一括保持 | bounded streaming＋必要なpayloadのみarenaへ |

公開JSON、Cargo metadata、既存Python framed JSONは境界adapterとして残せる。旧schema fieldの削除・rename・意味変更はしない。`additionalProperties:false` のconsumerへ新fieldを押し込まず、新schema／明示version negotiationを用いる。旧commandは旧shape、新facadeは小さい新viewを使用する。

binary v1は64-byte header：magic[8]、major:u16、minor:u16、flags:u32、payload_len:u64、section_count:u32、header_len:u32、frame_digest[32]。section entryは32 bytes：tag:u16、version:u16、flags:u32、offset:u64、length:u64、count:u32、reserved:u32。little endian、reserved=0、checked arithmetic、非重複range、上限検査後にallocation。未知critical tag／majorを拒否し、未知optional fieldはskipするかopaque bytesを保持して明示する。schema limitsと内部limitsを区別する。

binary v1のmagicはASCII `EPITLCX1`、header_lenは64固定、directoryは直後に配置する。payload_lenはdirectoryを除くpayload bytes。offsetはframe先頭基準、section_count<=64、directory_len=count×32をchecked計算する。header flags=0、section flagsのbit0だけをCriticalとして許可し他bitは拒否、tag昇順・tag一意・unknown flags拒否・全reserved=0・padding/未使用gapなし・section順に隙間なく配置・全frame長との完全一致を要求する。FrameDigestはdomain separator＋digest欄をzeroにしたheader＋directory＋payload全体を束縛する。hash一致は認証ではなく、信頼するreceiptがこのFrameDigestを束縛した場合だけ真正性を別判定する。未知optional sectionは未解釈のまま保持し、署名保持roundtripを行うconsumerは原bytesを保存する。破棄したsectionを再符号化して旧署名が維持されたとは主張しない。

raw Rust struct dump、persisted pointer、usize wire、enum transmute、unchecked mmap referenceを禁止。GPU用POD転送layoutと永続wireを別定義にする。既存のMAC／replay hashは旧canonical bytesで検証し、新domainへの移行は明示的なmigration receiptを伴う。保存形式変更だけから旧Grantを復活させない。

Rustの型のlayoutは永続wire保証ではないため、明示codecを使う。[Rust Reference](https://doc.rust-lang.org/reference/type-layout.html)を参照。readerはmetadata確認後の全readではなく、読取り自体に上限を持たせる。[Read::take](https://doc.rust-lang.org/std/io/trait.Read.html#method.take)はその構成要素であり、allocation／deadline／総量管理を単独で代替しない。

### 6. キャッシュ・資源・再開の契約

新runtimeの出発時budget profileを次とする。数値は性能実証値ではなく、TLCX-00/37で実測し設定として固定する工学的な上限。意味精度の閾値ではない。

| 対象 | 初期profile |
| --- | --- |
| 原文 | 現DGCLの256KiB上限を維持。大文書はsource mapping付きchunk化 |
| Rust所有live state | 128MiB/run、512MiB/processの予約上限 |
| 同時外部solver | 2。child memoryはOS別の実enforcement状態を別報告 |
| 新binary frame | 最大16MiB、header後・allocation前検査。proof等はsegmented |
| 通常summary | 64KiB、detailsはcursor/revision付きpage。旧full viewは既存の明示上限を維持し、無上限入口はbounded read／streamingへ改訂 |
| 再計算可能disk cache | 既定off。明示enable時256MiB総量、TTL60分、entry/bytes双方管理 |
| runtime scratch | 256MiB/run、1GiB総量。Media大入力は別profile・明示予約 |
| durable evidence | 4MiB segment、256MiB store上限。満杯時はadmission停止／export要求であり自動破棄しない |
| telemetry | online統計＋上限付き代表sample。冗長なreceiptコピーの全件保持は明示diagnosticのみ |

`OwnedTemp/CacheLease/PinnedEvidence/DurableJournal/SecurityReplayState/ModelStore/BuildCache` を別所有型にする。証拠の唯一コピー、nonce消費履歴、revocation、trusted headを通常LRUで消さない。security stateの安全なcompaction条件がない場合は容量拒否。crash後も所有manifestとfile identityで予約を回復し、別sessionのstateを横取りしない。

cache keyはcontext/source/goal/probe/logic/solver/options/model/tool/validator/codec revisionを含む。cache hitは現在のauthority・期限・失効・host deliveryを復元しない。固定logical work plan・seed・solver条件ではcold/warm/corrupted/evicted cacheによる結果の同一性を要求する。同じwall budgetでは探索量や推奨が変わり得るが、権限・適用条件・証拠状態は変えない。必要な採用／replay／revocation／repair証跡はEvidenceStoreで保持し、telemetry圧縮で唯一のreceiptを捨てない。

削除機構は所有root・marker・resolved identity・active lease・reparse/hardlink・retentionを検査し、許可範囲内の再計算可能物だけを回収する。locked／失敗時は停止し `CleanupFailed/Retained` と予約を残す。Dropはbest effortであり、明示closeの成功を回収証拠にする。ユーザー既存cacheを一括削除する機能ではない。

logical artifactとphysical recordを分ける。IPC chunkおよびjournal recordは1MiB以下で、object/transfer ID、sequence、total bytes、terminal marker、全体digestを束縛する。4MiB segmentをrecord途中で分割せず、大きいcheckpoint/proofは予約済みchunk列と参照で保存し、一括128MiB serializeしない。partial receive/appendは未完成のまま公開しない。既存2MiB stdin上限はlegacy profileで維持する。より大きいsolver入力はmax_stdin_total付きの明示streaming profileで扱い、1MiB chunkを用いて総量・deadlineを監督し、暗黙に上限を上げない。

VRAMはdeviceごとにmax_allocated_bytes／max_inflight_bytes／max_buffers／retired_until_fence_bytesをTLCX-00のdevice limitsと計測から固定する。未設定ならGPU admission拒否。runtime自身のdevice全allocationを数え、frameworkの見えない予約は未観測として区別する。同じstoreを共有するprocess間ではlock/atomic reservationで総量を制限し、process-local capの足し算を総量保証としない。

共同rollbackは、外部trusted monotonic anchorがあるprofileだけで後退検出を要求する。anchorなしはRollbackResistanceUnavailableを維持しfresh検証を要求し、同じstoreのheadだけで検出可能とはしない。host-integrated profileが外部anchorを必要とする場合、その供給をTLCX-00で外部hard dependencyとして登録する。nonce compactionは、全対象receiptのexpiry＋clock条件、または旧key epochを二度と受理しない確認済retirement等が成立する場合だけ。無期限receiptや未閉鎖epochはevictしない。

build target、model weights、runtime cacheを混同しない。通常plugin起動から暗黙cargo buildをなくし、開発fallbackだけ明示選択する。テストは所有scratchと共用targetを分け、成功時回収／失敗時上限付き保存を契約化する。大きなGPU依存buildがゼロ容量になるとは主張しない。

compact summaryが参照するartifactの寿命も契約化する。Full profileは明示したstorage authorityと所有EvidenceStoreを必要とし、inspect/resumeは同じstore・context・revisionへのread capabilityを検査する。cache offは再計算cacheがoffという意味で、必要証拠の破棄ではない。store未設定のephemeral modeではinlineで保持できる結果だけ返し、process終了後に辿れないartifact refやresume可能という表示を出さない。page cursorの期限・retention・権限失効を表示し、最後の唯一証拠への参照を回収対象にしない。

### 7. Rust実装規律と接続完了の定義

- safe・idiomatic Rustを既定。semantic coreとcodecにunsafeなし。既存OS/GPU unsafeは小さいmoduleへ隔離し安全契約を維持。
- まずinvariant、invalid state、owner、error、budget、consumerを書き、それから型と実装を書く。
- 1moduleは一責務。新sourceは原則400行以内、関数60行以内をreview目安とし、機械的分割や巨大generic/macroを避ける。例外には理由を記録。既存monolithは新機能追加前に境界単位で分割。
- Optionの存在だけでVerified／Completedにしない。型付きResult／明示状態を使用。productionのunwrap/expectは静的に成立する不変条件以外禁止。
- immutable sourceはArc、arenaは単一owner＋借用view。大きい構造のCloneを既定にしない。HashMap順序・Debug表現を新canonical digestに使わない。
- hot numeric pathだけSoA、checked fixed-width integer／finite float wrapperを使う。実測なしにunsafe・SIMD・GPUを追加しない。
- 新設interfaceは、そのsliceまでに導入済みの実consumerへ接続する。未導入の最終consumerを先行sliceの出口に要求しない。将来の接続先と再検証sliceを登録し、最終G14では全辺に実positive観測とcutによる結果変化を要求する。

完成とは、宣言した必須profileの `入力→生成→実行→観測→消費→選択→出力→再開` が実データで通り、各辺のcut testが失敗すること。`型がある／doctorに出る／hashが一致／全候補が空なのでall()がtrue` は完了証拠ではない。実装完成、モデル内最適性、一般研究効果、host配信を別判定にする。最後の必須host callbackが未提供なら全体完了を偽らずhard gate未達として残す。

### 8. スライス共通の実行契約

全sliceは次を記録する：依存snapshot、対象path、負例の失敗観測、実装差分、正常系・境界検証、producer/consumer、失効条件、未観測事項、出口判定。記録はbounded summaryで、巨大pipeline dumpを添付しない。

対象検証は `cargo fmt --all --check`、対象crate test、必要なproperty/schema/codec/negative integration。節目と最終gateでは `cargo check --workspace --locked`、`cargo test --workspace --no-default-features --locked`、`cargo test --workspace --locked`、`cargo clippy --workspace --all-targets --locked -- -D warnings`、`cargo build --workspace --release --locked`。Windows/Ubuntu、実solver、実configured JA/EN worker、実GPU、fresh packageを別gateとして記録する。未対応platformを成功としない。

各sliceの全テスト名は下記IDをprefixにし、登録表でfixture→test→production entrypoint→consumerを一意に結ぶ。public schema破壊、source/span破壊、既存差分の実質衝突、検証不能なhard gateだけを停止対象にし、予定されたfail-first失敗は停止理由にしない。scope・budget・claimを緩めてgateを通さない。


TLCX-00でOS/backend matrixも固定する：Windows x86_64はportable core・実SMT/ASP・現在のconfigured JA/EN・対象host、Ubuntuはportable core・実SMT/ASPを必須とする。Ubuntu configured NLPを必須にする場合は別runtime/model pinと実positiveを追加する。GPU/Mediaはdevice/backend別の明示profileとし、別OSの成功を流用しない。host/version、precommit/postsend/sink入口、証跡発行者、外部anchor、必要なinstall/activation authorityと外部ownerを最初に記録する。35はadapter/source契約、36はpackage、42はcandidate readiness、43は許可されたactivation後の実host G13/G15であり、順序を逆転しない。

### 9. 依存順の詳細ロードマップ

以下のpathは、特記しない限り `scripts/lc631/crates/lc631-<名前>/` 基準（例：`analysis/src`＝`scripts/lc631/crates/lc631-analysis/src`）。`substrate/solvers/runtime` は新crate、`(new)` は追加予定であり現在存在するとの主張ではない。各sliceは下記の明示DAGに依存する。原則として番号順で進めるが、外部観測待ちの独立sliceは進められる。既存契約は全て保持し、変更箇所が重なるsliceは並列編集しない。

#### TLCX-00：基線・全コード台帳・必須profile固定

- 依存：固定基線と明示した設計合意。
- 対象：`Cargo.toml; .github/workflows; docs/tlcx-source-reading-ledger-2026-10-28.json`。
- 実装・配線：既存差分、全producer/consumer、公開command、canonical preimage、署名領域、solver binary/version/license、OS別profileを固定。空の正常系やdoctorだけの証拠を拒否する基準を先に作る。
- 負例→正常系：対象file欠落・snapshot変更・必須backend欠落で未達。cold基線の意味結果、出力bytes、RAM/VRAM、disk増分、latencyを同じseedで採る。
- 出口：source台帳100%、依存lock、最低2種の実solverと有限論理profile、比較条件・予算を登録。既存重大baseline失敗は修復前提sliceへ分離。

#### TLCX-01：契約分離と先行断線試験

- 依存：TLCX-00のslice-local出口。
- 対象：`core/src/context.rs (new); runtime/tests/closure_cuts.rs (new)`。
- 実装・配線：ContextRevision、AuthorityRevision、EvidenceRevision、TaskKind、DecisionStatusを別型にする。D/Delta/U/Gと操作状態、完成状態を混ぜない。
- 負例→正常系：前提再構成からGrant生成、未検証を0損失化、unsupportedをSAT化、空候補all()成功を負例にする。正例には実候補数>0と結果消費を要求。
- 出口：新crateのCargo skeleton、共通protocol型、明示NotConnectedのruntime入口と全cut pointを登録する。初期契約試験はNotConnected/Unsupportedを正しく返すgreenな試験。将来のpositiveを常設red/ignoreで残さず、該当sliceでfail-first導入→実装→greenとし、最終G14で全positiveを必須にする。

#### TLCX-02：単一source・型付きarena

- 依存：TLCX-01のslice-local出口。
- 対象：`substrate/src/{ids,source,arena}.rs (new); tldg/src/source.rs; analysis/src/dgcl_artifact.rs`。
- 実装・配線：Arc<str>のsource一個、Digest32、SymbolId、AstId、ConditionId、TaskId、EvidenceIdと世代付き参照を実装。旧reportは借用viewで作る。
- 負例→正常系：UTF-8境界、cross-context参照、世代再利用、重複ID、allocation失敗を拒否。selected ASTとalternativesが同じ正本を参照する正例。
- 出口：原文byte/spanと旧結果を維持し、AST/taskの深い複写を通常経路から除去。source→arena→旧DTOが実接続。

#### TLCX-03：上限付きcanonical binary codec

- 依存：TLCX-02のslice-local出口。
- 対象：`substrate/src/codec/{header,sections,encode,decode}.rs (new)`。
- 実装・配線：本書の64-byte headerと32-byte section entryをsafe明示codecで実装。stream hash、固定tag、長さ付き文字列、sorted map、unknown critical拒否を通す。
- 負例→正常系：切断、長さoverflow、重複section、範囲重複、不正UTF-8、未知major、trailing bytes、巨大countをnegative/fuzz化。独立golden bytesで正常decode。
- 出口：encode/decode/canonical hashがplatform-independent。bounded readerを実入口で使用し、型layoutのメモリdumpを使わない。

#### TLCX-04：旧JSON・署名domain互換

- 依存：TLCX-03のslice-local出口。
- 対象：`wire/src/legacy_codec.rs (new); receipt-kernel/src; analysis/src/*digest* callers; schemas`。
- 実装・配線：内部型と旧canonical encodingを分離し、legacy署名は旧bytesで検査。新binaryは別domain。strict DTOはversion選択し旧fieldを維持。
- 負例→正常系：旧MACを新domainで受理、再hashだけで証拠昇格、未知fieldでstrict v1破壊を拒否。既存fixture/golden MAC/hashを正例にする。
- 出口：全canonical preimageの移行表と消費先が埋まり、legacy JSON↔新内部型↔legacy JSONで従来意味と署名検証を保持。

#### TLCX-05：共通資源brokerとbounded I/O

- 依存：TLCX-02, TLCX-04のslice-local出口。
- 対象：`runtime/src/resource.rs (new); process-supervisor/src/bounded.rs; CLI io; host readers`。
- 実装・配線：allocation前のRAM/disk/output/proof/child予約、deadline、cancel、cleanup予算を一つのbrokerへ接続。無上限stdin/file/Command::output経路を除く。
- 負例→正常系：metadata後にfile増大、solver出力暴走、pipe未終了、child逃走／cleanup不明を検査。正常終了で予約が戻る実観測。
- 出口：全実行入口の資源owner一覧が埋まり、上限超過は型付き失敗。OS enforcementと観測上限を混同しない。

#### TLCX-06：cache・証拠・replay状態の分離

- 依存：TLCX-05のslice-local出口。
- 対象：`runtime/src/store/{cache,lease,recovery}.rs (new); receipt-kernel::ReplayGuard; test helpers`。
- 実装・配線：再計算可能cache、active lease、durable evidence、security replay、weights、build cacheを分離。既定disk cache off、enable時quota/TTLと所有manifest。
- 負例→正常系：eviction後nonce再利用、live GPU buffer回収、未知dir削除、crash後予約消失、locked fileを強制削除する経路を拒否。
- 出口：固定work plan・seedではcold/warm/evicted/corrupt cacheで結果同一。同じwall budgetでの追加探索は別計測。権限境界は不変、回収失敗をRetainedとして費用計上しuser既存cacheを触らない。

#### TLCX-07：binary journalと安全な移行

- 依存：TLCX-04, TLCX-06のslice-local出口。
- 対象：`analysis/src/closure_resume.rs; host/src/lib.rs; runtime/src/store/journal.rs (new)`。
- 実装・配線：上限付きsegment/index/headを実装。旧JSON chainは旧codecでread-only検証、新chainへ明示migration。全readerに同じ上限を通す。
- 負例→正常系：torn append、segment swap、古いhead、compaction中crash、唯一証拠削除を負例。共同rollbackは外部anchor有で検出、無ではRollbackResistanceUnavailableを検査。正常resumeはfresh authority/sourceを再検査。
- 出口：appendの全履歴再parseを除き、index破損は証拠を捏造せず再構築／Hold。retentionのない証拠は自動削除しない。

#### TLCX-08：文脈・前提再構成を第一級化

- 依存：TLCX-01, TLCX-02のslice-local出口。
- 対象：`core/src/context.rs; world/src/context_graph.rs (new); analysis context lowering`。
- 実装・配線：OriginalContextを不変anchorとして、ContextDelta付き再構成枝を本線と同等の探索対象にする。hard invariantと操作権限は変更不可。
- 負例→正常系：必須条件を落として高得点、元context上書き、同じ意味の枝をID違いで水増し、推測前提をObserved化を拒否。
- 出口：ContextGraphの実consumerでparent/delta/適用条件と固定比較scopeを検査する。推奨候補への消費は23、説明は29で再検証する。

#### TLCX-09：DeepGrammarからtask別論理表現へ

- 依存：TLCX-04, TLCX-08のslice-local出口。
- 対象：`analysis/src/{instruction_grammar,dgcl_pipeline,dgcl_projection,completion_plan}.rs`。
- 実装・配線：数学／推論／説明／Coding／MediaをTaskKindで分ける。source-bound ASTから型付きformula・条件predicateをlowerし、自然言語の未対応spanを保持。
- 負例→正常系：説明へCargo5証拠を要求、unknown条件をtrue、default negationを古典否定、referenceの推測束縛を拒否。
- 出口：同じartifactからContextIRとProgram IRへ接続。taskごとに独自のevidence義務が生成され、全N/Aで逃げない。

#### TLCX-10：実solver registry・protocol・監督

- 依存：TLCX-05, TLCX-09のslice-local出口。
- 対象：`solvers/src/{registry,protocol,runner}.rs (new); process-supervisor`。
- 実装・配線：semantics/version/options/domain capabilityを登録し、typed queryをargv＋stdinへ送る。model/proof/counterexampleをbounded parserで読む。
- 負例→正常系：command injection、誤binary、scope違い、timeout→UNSAT、truncated出力→SAT、失効した結果再利用を拒否。
- 出口：実processのquery/result/evidence IDを、ここで導入するtyped ResultStoreが実消費する。TLへの最終接続は17、候補選択は23で再検証。mockだけのbackendは必須profileを満たさない。

#### TLCX-11：SMTの実求解・最適化

- 依存：TLCX-10のslice-local出口。
- 対象：`solvers/src/smt.rs (new); fixtures/tlcx/smt`。
- 実装・配線：Boolean/QF_LIAを最初の宣言fragmentにする。SAT model再評価、UNSAT/proof状態、最適値・gap・timeoutを区別。typed emitterでsymbolをescape。
- 負例→正常系：誤model、未宣言symbol、unbounded objective、unsupported quantifier、単なるSATを最適解化を拒否。
- 出口：実backendの整数制約結果をmodel validatorとResultStoreが消費する。条件producerへの接続は28、候補比較は23で再検証する。

#### TLCX-12：ASPの例外・非単調推論

- 依存：TLCX-10のslice-local出口。
- 対象：`solvers/src/asp.rs (new); fixtures/tlcx/asp`。
- 実装・配線：有限ground normal rulesと明示default negation、brave/cautious consequence、最適化costをclingoへ接続。入力の有限化と根拠を記録。
- 負例→正常系：閉世界仮定の無断追加、braveをcautious化、ground爆発、例外脱落、複数modelの勝手な単一化を拒否。
- 出口：同じ問題の例外追加で結論が正しく変化し、元contextと再構成contextへそれぞれ結果を束縛。

#### TLCX-13：時間論理・順序・反例trace

- 依存：TLCX-10のslice-local出口。
- 対象：`solvers/src/temporal.rs (new); core temporal types; media event adapter`。
- 実装・配線：有限trace意味論を明示し、before/after/untilと基準時・単位を評価。無限traceを扱う場合は別capabilityと実model checkerに分離。
- 負例→正常系：有限prefixを永久保証、絶対／相対時刻混同、境界時刻・loop反例脱落を拒否。
- 出口：実評価器が成立／不成立／unknownと反例traceをResultStoreへ返す。temporal TLは19、条件付きtask consumerは28で接続・再検証。

#### TLCX-14：矛盾許容・四値推論

- 依存：TLCX-10のslice-local出口。
- 対象：`solvers/src/four_valued.rs (new); core truth-state types`。
- 実装・配線：支持と反駁を独立bitとして有限規則を評価。古典UNSATと矛盾保持を別resultにし、局所矛盾の依存範囲を記録。
- 負例→正常系：pとnot-pから無関係qを導く爆発、NeitherをFalse化、Bothを未知へ隠すことを拒否。
- 出口：独立truth table／全小モデルとの一致と、ResultStore・context枝への結果消費を観測。選択・最終説明の変化は23/29で再検証。

#### TLCX-15：有限modal・直観主義・認識・義務

- 依存：TLCX-10, TLCX-14のslice-local出口。
- 対象：`solvers/src/kripke/{model,evaluate,profiles}.rs (new)`。
- 実装・配線：有限Kripke modelとprofileごとのframe条件を実装。直観主義の持続性、認識のaccessibility、義務の規範と実行権限を分離。
- 負例→正常系：同じenumで論理を替えたふり、frame条件違反、O(action)をGrant化、古典同値の無断使用を拒否。
- 出口：各宣言profileに成立例／反例／独立小モデルoracleがあり、projection lossの対応表を返す。

#### TLCX-16：因果・空間と反事実の実評価

- 依存：TLCX-10, TLCX-13のslice-local出口。
- 対象：`solvers/src/{causal,spatial}.rs (new); core context schemas`。
- 実装・配線：有限非巡回SCMの介入と条件付け、有限座標／包含／接続関係を実評価。時間的因果と意味上の依存を型で分ける。
- 負例→正常系：相関→因果、未指定構造式の捏造、座標系違い、介入によるhard constraint消去を拒否。
- 出口：反事実query・空間関係の実結果と不足情報をtyped ResultStoreへ返す。D/Delta/Uへの消費は17/19で再検証し、未指定情報をUnknownとして保持する。

#### TLCX-17：四記録のcanonical ledger

- 依存：TLCX-04, TLCX-09, TLCX-10のslice-local出口。
- 対象：`tl/src/{transform,measurement,composition}.rs (new); projection_v3 adapter`。
- 実装・配線：TransformId・source/target/context・方向・測定scope・D/Delta/U/G・witnessを単一正本へ保存。既存defect graphはviewとして対応付け。
- 負例→正常系：同じ欠損／利得の二重加算、異なる尺度の合算、意図的変更の免罪、target-only事実の成功昇格を拒否。
- 出口：導入済みsolver入口・出口・前提変更を同一ledgerへ接続し逆追跡する。renderer接続は29、host後の同一性は35/43で再検証する。

#### TLCX-18：再構成・意味損失の測定

- 依存：TLCX-11, TLCX-17のslice-local出口。
- 対象：`tl/src/measure/{reconstruction,semantic}.rs (new); solvers query adapters`。
- 実装・配線：元sourceを隠した逆変換、固定probe、含意／反例／量化／否定／scopeの保存を検査。exact・bounds・estimatedと未測定範囲を別出力。
- 負例→正常系：原文のコピー返却でrecon=0、probe数を減らして損失低下、共通誤変換による見せかけ一致を拒否。
- 出口：既知の変形fixtureで損失の方向と範囲が変わり、ledgerの実consumerへ保存される。測定値が候補選択／修復を変える試験は23/31で再検証。

#### TLCX-19：時間・空間損失の測定

- 依存：TLCX-13, TLCX-16, TLCX-17のslice-local出口。
- 対象：`tl/src/measure/{temporal,spatial}.rs (new); temporal/spatial solver outputs`。
- 実装・配線：時系列・期間・基準時・配置・接続・座標変換についてdomain別単位とprobeを固定し、forward/reverseを独立評価。
- 負例→正常系：時刻offsetの意味保存を欠陥化、時間逆転を保存化、画面座標と世界座標の混同、証拠不足を0にすることを拒否。
- 出口：導入済み時間／空間solverのtyped relationを測定器とledgerが実消費し、scope外はIncomparable。自然言語／Mediaの全接続は30で再検証。

#### TLCX-20：多論理間の対応と不一致分析

- 依存：TLCX-11, TLCX-12, TLCX-14, TLCX-15, TLCX-18, TLCX-19のslice-local出口。
- 対象：`tl/src/cross_logic.rs (new); solvers result normalization`。
- 実装・配線：query・model quantification・前提・shared extractorの依存を比較し、不一致を説明するbridge witnessを構築。多数決を使わない。
- 負例→正常系：SATとentailmentの比較、brave/cautious混同、同じ誤parserを独立票として数える、矛盾を無言消去する経路を拒否。
- 出口：不一致原因からtyped ReprojectionRequestを構築し、query/context consumerが検査する。実schedulerへのfeedbackは25、最終説明は29で再検証。

#### TLCX-21：創造的候補生成と前提再構成

- 依存：TLCX-08, TLCX-20のslice-local出口。
- 対象：`world/src/{creative,operators,archive}.rs (new); runtime CandidateProvider`。
- 実装・配線：保存・再結合・reframe・抽象化・反例からの前提改訂をtyped operatorsとして実行。LLM提案はsource-bound候補として受け、生成providerを明示する。
- 負例→正常系：ラベルだけの新規性、無根拠な事実追加、操作許可変更、同一候補の別名登録を拒否。
- 出口：原案にない実候補が生成され、独立validatorを通って有効候補として保存される正例。deterministic operatorとLLM adapterを別能力として報告。

#### TLCX-22：S_Creativeの要素別計測

- 依存：TLCX-17, TLCX-21のslice-local出口。
- 対象：`world/src/creative/{novelty,compatibility,gain,cost}.rs (new)`。
- 実装・配線：原案v1/v2の距離・親和性・構造／再編成／実効利得を基線相対に観測。新規性corpusのrevisionと未知範囲を保持。
- 負例→正常系：高新規性で必須欠陥相殺、TL二重減点、単純化への不当罰点、候補自身がoracleを書換え、未観測Gを実績扱いする経路を拒否。
- 出口：CreativeAssessmentが非空で各componentの観測変化を保持し、比較用typed consumerが読む。frontierへの因果効果は23で再検証。既知手法を一律排除しない。

#### TLCX-23：文脈と解の共同最適化

- 依存：TLCX-18, TLCX-19, TLCX-20, TLCX-22のslice-local出口。
- 対象：`world/src/{frontier,selection,optimality}.rs (new)`。
- 実装・配線：元contextと再構成contextの候補を、固定goal/hard invariants/比較scopeで比較。Pareto集合から推奨1つ＋重要な代替解を選ぶ。
- 負例→正常系：前提削減で問題を易しくして勝つ、incomparableを低scoreへ変換、予算切れをglobal optimum、top16以外の候補消失を拒否。
- 出口：推奨・代替・不採用理由・最適性の範囲が同じDecisionArtifactへ束縛。再構成枝が正当に推奨される正常系も必須。

#### TLCX-24：非ユークリッド探索空間

- 依存：TLCX-08, TLCX-17, TLCX-23のslice-local出口。
- 対象：`world/src/geometry/{graph,poincare,directed_cost}.rs (new); tldg legacy adapter`。
- 実装・配線：実際の状態遷移graphを基礎に階層embeddingと方向付き費用を分離。Poincareのnorm-squareは専用の非負区間`square`で構築し、非幾何基線を保つ。離散noveltyは`DistanceToFrontierUnion-v1`と`NearestVerifiedCandidate-v1`を別`SchedulerNoveltyPolicyId`として保持し、current union policyを無計測で置換しない。意味距離という未実証名を付けない。
- 負例→正常系：node ID順でrank変化、自己距離非zero、区間二乗の負下限、n_geo逆端点、入力box幅の誤判定、ball境界/NaN/overflow、geometryから真理・Grant生成を拒否（measurement contract G31〜G37）。
- 出口：少なくとも一つの実非ユークリッド計算と不変性試験。順位提案をtyped SchedulingProposal consumerへ渡し、実schedulerの選択は25で再検証。TL正本は独立。

#### TLCX-25：損失・利得・不確実性による資源配分

- 依存：TLCX-05, TLCX-23, TLCX-24のslice-local出口。
- 対象：`world/src/scheduler.rs (new); runtime resource broker; process-supervisor`。
- 実装・配線：期待改善、重要U解消、多様性、費用を明示policyで比較し、次のsolver/query/branchを選ぶ。SchedulerNoveltyPolicyIdをbudget/scopeへ束縛し、union距離とnearest-candidate距離のどちらを使ったか記録する。exploration枠・fairness・cancelを実装。
- 負例→正常系：cheap branchだけの飢餓、unknownの無限再試行、priorityからquota増幅、勝手なnetwork実行を拒否。
- 出口：同じ問題でD/G/U/costの変更が次の実process選択を変え、CPU/GPU/childの予約上限を守る。

#### TLCX-26：実world管理と意味的重複

- 依存：TLCX-06, TLCX-08, TLCX-25のslice-local出口。
- 対象：`world/src/lib.rs refactor; world/src/{arena,identity,coverage}.rs (new)`。
- 実装・配線：固定enum直積のproduction依存を除き、実premise/model/query/resultを持つ枝に置換。256基底枠・2048live枠と世代を実管理。
- 負例→正常系：slotだけ違う枝のdistinct主張、論理未実行を探索済扱い、quota不足のsilent padding、歴史の無制限蓄積を拒否。
- 出口：探索数・実行数・構造distinct・意味distinct未確認を別報告。legacy256×8は明示互換diagnosticに隔離。

#### TLCX-27：新構造を獲得する相互蒸留

- 依存：TLCX-20, TLCX-24, TLCX-25, TLCX-26のslice-local出口。
- 対象：`tldg/src/distillation.rs; world proposal validators; analysis/src/dgcl_distillation.rs`。
- 実装・配線：連続候補を離散typed editへ復号し、既存edge membershipか新規関係専用validatorへ分岐。認めた変更だけ新stateへ反映。
- 負例→正常系：既存edgeと同じものしか通らない閉ループ、候補を正解に書き戻す循環、digestだけ更新、残差消去による偽進捗を拒否。
- 出口：新関係の追加で次epochのqueryと最終selectionが変わる。誤候補は残差へ戻り、NoProgress/予算で終了。

#### TLCX-28：solver結果から条件・Program IR・Codingへ

- 依存：TLCX-09, TLCX-11, TLCX-17, TLCX-23, TLCX-27のslice-local出口。
- 対象：`analysis/src/{dgcl_projection,completion_plan,dgcl_implementation_closure,validation_producer}.rs`。
- 実装・配線：exact predicate/context/semantics/resultに束縛したConditionObservation producerを追加。推奨実装だけ既存Cargo・connection・acceptance経路へ渡す。
- 負例→正常系：SATだけで条件解除、存在modelを全称化、solver scoreからTest/Edit許可、数学回答にCargoを強制する経路を拒否。
- 出口：論理taskは専用証拠で閉じ、Coding taskは既存5種証拠で閉じる。結果をcutすると条件／closureが未達へ戻る。

#### TLCX-29：推奨解・代替解・最終説明

- 依存：TLCX-23, TLCX-28のslice-local出口。
- 対象：`runtime/src/decision.rs (new); wire/src/render.rs (new); analysis/src/dgcl_final_output.rs`。
- 実装・配線：推奨1つ、代替、context変更、D/Delta/U/G、最適性scope、依存付き部分回答をtyped DecisionArtifactへまとめ、最後にbounded render。
- 負例→正常系：都合の悪い前提変更を説明から落とす、未解決前提に依存する部分回答、render後改変、JSON dumpを回答扱いすることを拒否。
- 出口：raw result→decision→説明のsource対応が通り、説明自体のTL測定とexact bytes bindingまで届く。

#### TLCX-30：Media・RPA・NLPの実観測接続

- 依存：TLCX-09, TLCX-17, TLCX-19, TLCX-28のslice-local出口。
- 対象：`media/src/lib.rs; scripts/media_backend.py; analysis/src/{pipeline,language_worker}.rs`。
- 実装・配線：source/time/frame/region/tool/modelに束縛した観測をContextIRの候補前提へ接続。worker streaming、model走査上限、registered model IDを使用。
- 負例→正常系：音響clusterを話者認識と呼ぶ、co-presenceを意味一致、provenanceをsemantic truth、方法catalogを実装済扱いを拒否。
- 出口：既存実backendの観測が時間・空間TL／仮説比較へ消費される。未導入methodはUnsupportedのまま必要profile gateを満たさない。

#### TLCX-31：候補生成から実修復まで

- 依存：TLCX-07, TLCX-28, TLCX-29のslice-local出口。
- 対象：`runtime CandidateProvider; analysis/src/{repair_loop,file_repair}.rs; CLI dgcl_repair.rs`。
- 実装・配線：最大4回の候補生成→fresh intent/authority→apply→実検証→loss/gain再評価を同一runtimeへ接続。per-attemptとrun-total deltaを別予算。
- 負例→正常系：同じreplacementの水増し、旧receipt再利用、counterfactual contextを実fileへ無断apply、scope・時計後退を拒否。
- 出口：複数候補の実修復正例、NoProgress/cycle/MayHaveAppliedの負例が同じproduction入口で成立し、backupを保持。

#### TLCX-32：checkpoint・再開・失効の閉鎖

- 依存：TLCX-07, TLCX-31のslice-local出口。
- 対象：`runtime/src/resume.rs (new); analysis/src/{closure_resume,evidence_lifecycle}.rs`。
- 実装・配線：選択・文脈・solver状態・未完了作用・四記録のcheckpointをbinary journalへ保存。再開時はfresh source/tool/validator/authorityとleaseを再検査。
- 負例→正常系：Completed復元だけの成功、UnknownDelivery後の二重apply、cacheから期限復活、変更contextの旧solver結果流用を拒否。
- 出口：実修復→checkpoint→process終了→同じ入口でresume→新観測→推奨再構成が通り、state保存量がbudget内。

#### TLCX-33：意味観測を使うGPU経路

- 依存：TLCX-05, TLCX-23, TLCX-24, TLCX-26, TLCX-27のslice-local出口。
- 対象：`accelerator/src/{lib,gpu}.rs; world numeric adapter; runtime scheduler`。
- 実装・配線：fixture由来軸をproductionから除き、比較scope付き実測D/G/costをSoAへ転送。arena lease/fence、device-side reduction、CPU final再検査を接続。
- 負例→正常系：unknown sentinelを小さいloss、ring近傍Paretoを全体Pareto、summary一致を全vector証明、device-loss後のpartial再利用を拒否。
- 出口：実GPUで全宣言laneが新データを消費し、selectionまでparity確認。fallbackは新しいCPU観測、occupancy／高速化は別実測。

#### TLCX-34：通常CLI・facadeを単一runtimeへ

- 依存：TLCX-25, TLCX-26, TLCX-27, TLCX-28, TLCX-29, TLCX-30, TLCX-32, TLCX-33のslice-local出口。
- 対象：`cli/src/{args,application,commands,render,io}; scripts/run-*.ps1; skills`。
- 実装・配線：epistesys-solve／inspect／resumeをtyped commandへ分離。conversation/coding facadeは同じsolve APIをtask policy付きで呼ぶ。doctorの自動多重実行を通常経路から外す。
- 負例→正常系：facadeが旧doctorだけを呼ぶ、--promptとstdin競合、default巨大dump、binaryなし暗黙build、同requestの二重solver実行を拒否。
- 出口：通常のユーザーseedが実solver→四記録→推奨解まで一回のcanonical経路で通る。旧commandsは明示互換viewで回帰成功。

#### TLCX-35：最終回答・host lifecycleの閉鎖

- 依存：TLCX-29, TLCX-34のslice-local出口。
- 対象：`host/src/{lib,stop_hook}.rs; runtime host adapter; hooks`。
- 実装・配線：render後のexact bytesをcandidate/precommit/postsend/sink/replayへ束縛。Stop観測と事前gateを分離し、現hostが提供するcallback能力を検査。
- 負例→正常系：自己署名Stopログを実sink配信へ昇格、別seed混合、callback順序違反、出力差替え、未配信を成功扱いを拒否。
- 出口：slice-local出口はsource adapterと登録callback contractの正負試験。対象host/version/発行主体/callback/必要権限を00で事前確認し、未提供をHostIntegrationPendingへ記録する。fresh package→明示activation→実host G13は43の必須出口であり、35の模擬callbackを実host観測としない。

#### TLCX-36：schema・source・platform・plugin配布

- 依存：TLCX-04, TLCX-32, TLCX-34, TLCX-35のslice-local出口。
- 対象：`schemas; compat; wire; scripts/new-*-package.ps1; plugin manifest; hooks/skills`。
- 実装・配線：source package、platform executable、plugin bundle、optional runtime/model bundleを別manifestで構築。fullview互換と新boundedviewをfresh packageから検査。
- 負例→正常系：source package成功をplugin完成、missing hooks/skills、wrong executable、旧schema上書き、秘密path/modelweight混入を拒否。
- 出口：repo外・cargoなしのfresh plugin packageでsolve/inspect/resumeが実行。manifestとbinary/source identityを照合。

#### TLCX-37：資源・cache・長期run回帰

- 依存：TLCX-06, TLCX-07, TLCX-25, TLCX-32, TLCX-33, TLCX-34, TLCX-36のslice-local出口。
- 対象：`runtime/tests/resource_soak.rs (new); worker/media tests; gate scripts`。
- 実装・配線：同じseed群でcold/warm、1000-turn bounded soak、cancel/crash、全予算境界を計測。test target共有と成功cleanup／失敗retentionを導入。
- 負例→正常系：runごとの巨大exe複製、無限sampler Vec、既存cache誤削除、security state退避漏れ、model走査の上限後付けを拒否。
- 出口：RAM/VRAM/disk/file/handle/childの上限と回収が観測でき、通常応答は64KiB以内。binary化前後の費用と意味結果をpaired記録。

#### TLCX-38：敵対・metamorphic・切断試験

- 依存：TLCX-11, TLCX-12, TLCX-13, TLCX-14, TLCX-15, TLCX-16, TLCX-18, TLCX-19, TLCX-20, TLCX-21, TLCX-22, TLCX-23, TLCX-24, TLCX-25, TLCX-26, TLCX-27, TLCX-28, TLCX-29, TLCX-30, TLCX-31, TLCX-32, TLCX-33, TLCX-34, TLCX-36, TLCX-37のslice-local出口。
- 対象：`fixtures/tlcx; runtime/tests; CI test registry`。
- 実装・配線：全必須辺へcut・swap・stale・corrupt・empty・timeoutを注入。再ラベル／順序／単位変換、意図的欠損、再構成勝者の正例を独立oracleで評価。v1.4のG31〜G37でn_geo端点、irrational q、区間入力、square enclosure、novelty-policy逆転、K3 universe移行、EmbeddingPolicy未接続を必須caseとして登録。
- 負例→正常系：fixtureを実装から自動再生成して正解化、空集合all()、mockだけpositive、切断しても同じ推奨結果の未消費経路を検出。
- 出口：全cutが期待した未達か別結果になり、復元後の同source正常系が通る。既存EPR/DGCL回帰を削除して通さない。

#### TLCX-39：校正・品質・過剰保留の評価

- 依存：TLCX-18, TLCX-19, TLCX-22, TLCX-23, TLCX-37, TLCX-38のslice-local出口。
- 対象：`tl calibration; analysis/src/dgcl_gold.rs; benchmarks/tlcx (new)`。
- 実装・配線：loss/gain尺度のtrain/calibration/holdout/semantic familyを分離。silent error、false hold、valid novelty rejection、risk-coverage、文脈再構成利得を測る。
- 負例→正常系：同文言のseedだけで一般化、共通oracleを独立扱い、未知を除外して高精度、未来のholdout調整を拒否。
- 出口：評価APIと小さな独立形式oracle corpusを稼働。未提供の自然言語goldはPendingNoCorpusのまま、一般効果を実装gateから推論しない。

#### TLCX-40：全経路integrationと廃経路整理

- 依存：TLCX-28, TLCX-29, TLCX-30, TLCX-31, TLCX-32, TLCX-33, TLCX-34, TLCX-36, TLCX-38のslice-local出口。
- 対象：`runtime integration; all public command registry; legacy adapters`。
- 実装・配線：全public入口をproducer/consumer/evidence/失効/切断試験に対応付け、未使用新moduleを除く。旧stage-only成功経路はhistory/compatへ隔離。
- 負例→正常系：新機能がdoctorだけ、二つの正本、旧経路で新gate回避、report fieldだけで完了する経路を拒否。
- 出口：同一source/context/decisionでinput→actual solve→D/Delta/U/G→selection→coding/media consumer→render→resumeまで通る。削除は別途許可された対象だけ。

#### TLCX-41：独立再監査・日英文書整合

- 依存：TLCX-36, TLCX-37, TLCX-38, TLCX-39, TLCX-40のslice-local出口。
- 対象：`docs; README; RepoSeiri; bounded Assurance review`。
- 実装・配線：実装者と異なる観点でarchitecture、unsafe、codec、authority、cache、solver意味論、finite-formal-v1.4数値契約を再監査。RepoSeiriは固定source digestで実行、Assuranceは完全16KiB以下単位。
- 負例→正常系：監査unknownをpassへ変換、将来構想を実装済claim、日英claim level不一致、旧version史の書換えを拒否。
- 出口：指摘ごとの修復・再試験があり、未解決hard gateは残る。README/docs/schema/verificationの同内容日英が整合。

#### TLCX-42：全hard gate・release candidate判定

- 依存：TLCX-36, TLCX-37, TLCX-38, TLCX-39, TLCX-40, TLCX-41のslice-local出口。
- 対象：`CI; release gate; Cargo/plugin identity; verification record`。
- 実装・配線：現在snapshotでsource側の機能・互換・資源・実backend・package gateを再実行。target versionを実装開始時に明示決定し、ここでは配布用candidate identityまで。製品release完了への昇格は43のG13/G15後だけ。
- 負例→正常系：古いreceipt、欠落profile、skip成功扱い、source版だけで完了、fullvector未確認を証明扱いを拒否。
- 出口：配布candidateのsource/binary/plugin/schema/doc identity一致。host待ちはCandidateNotFullyReleasedとし、G13を通過扱いにしない。43で全identityと全hard gateを再確認し、基準を緩めない。

#### TLCX-43：最終配線確認・引渡し

- 依存：TLCX-35, TLCX-36, TLCX-37, TLCX-38, TLCX-39, TLCX-40, TLCX-41, TLCX-42のslice-local出口。
- 対象：`installed package (when authorized); runtime/host end-to-end; final ledger`。
- 実装・配線：配布候補から新sessionで同一seedを実行し、ソルバー・四記録・推奨・代替・exact host・replay・resumeを逆追跡。権限がある場合だけinstall/activation。
- 負例→正常系：「完了」と書いたrecordだけで成功、別binaryの検証流用、必須hostを残した全体完成、未所有cache掃除を拒否。
- 出口：全必須辺のExecuted/Consumed/Contained/HostBoundを実観測。欠ければ部分完成と理由を明示し、全体完了を主張しない。commit/push等は別authority。


### 必須接続台帳（全20辺）

各辺に実producer・consumer・正常系観測・切断試験・失効条件を登録する。実装時にcallerだけ／calleeだけを増やして出口を通さない。

| ID | 接続 | 保持するもの | 切断・攻撃例 |
| --- | --- | --- | --- |
| E01 | host/user ingress → SourceStore | 原文・role・UTF-8 span | source swap／上限超過 |
| E02 | SourceStore → DeepGrammar/ContextIR | 解釈候補と未対応span | 抽出脱落／他revision AST |
| E03 | ContextIR → ContextGraph | 親context・Delta・hard invariants | 前提緩和で必須条件消去 |
| E04 | ContextGraph → SolverProjection | 同じ問いと変換scope | 否定・量化・domainの誤lowering |
| E05 | SolverProjection → supervised process | binary/options/query identity | 偽stdout／timeout／注入 |
| E06 | SolverResult → model/condition validator | query/result/semantics対応 | SAT→entailment昇格 |
| E07 | Validated observation → TL four facets | D/Delta/U/G・scope・証跡 | 不明=0／欠損重複加算 |
| E08 | CandidateProvider → CreativeAssessment | 原案componentと基線 | 自己oracle／未知Gの実績化 |
| E09 | TL + CreativeAssessment → frontier | 同じ目的・単位・比較可能性 | 異なる問題の混合／未消費値 |
| E10 | frontier/geometry → scheduler | 候補・遷移費用・U | ID順依存／飢餓 |
| E11 | scheduler → resource broker → backend | 予約・deadline・cancel | quota増幅／資源漏れ |
| E12 | continuous proposal → discrete validation → next epoch | typed editと新関係の検証 | digestだけ進捗／循環証明 |
| E13 | selection → Program IR / task plan | task別必要証拠 | 全taskをCargo化／全N/A |
| E14 | coding candidate → permit → validation producers | fresh authority/source/tool/binding | solverがGrant生成 |
| E15 | repair → checkpoint → resume | 未完了作用・backup・fresh検証 | 二重apply／Completed復元 |
| E16 | Media/RPA/NLP → candidate premises | fragment/time/method/provenance | 方法labelだけでtruth化 |
| E17 | observed numeric batch → GPU → CPU final check | row/epoch/scope/fence | fixture値／stale readback |
| E18 | DecisionArtifact → exact renderer | 推奨1・代替・前提変更・U | 説明で留保脱落 |
| E19 | render → precommit → postsend → sink/replay | exact bytes・同一seed・stage順序 | Stopを配信と同一視 |
| E20 | artifact store → cache/journal/security consumers | 所有・lease・retentionの分離 | security evict／未知root削除 |

### 10. Rust断片の実装契約

次の断片は命名・所有権・状態分離の契約例であり、そのまま全実装が揃ったことを意味しない。TLCX-02/03でconstructor、canonical codec、property testを実装する。Ratioの比較はchecked cross multiplicationまたは上限付き整数方式を用い、overflowをfloatへ黙って逃がさない。Boundedはlower<=upperと同一unitを検査する。ArenaKey単体のdeserializeから検証済参照を作らない。domain/arena/contextをアクセス時に検査し、generation overflowではwrapせずslotをretireする。永続LogicalIdとprocess-local ArenaKeyを別型にし、resumeではLogicalIdから新handleを再構成する。

```rust
// API sketch: private fields are constructed by checked arena/validator APIs.
// This sketch is a design contract, not compiled implementation evidence.
use std::num::NonZeroU64;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct Digest32([u8; 32]);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct ArenaDomainId(Digest32);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct ArenaKey {
    domain: ArenaDomainId,
    slot: u32,
    generation: u32,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct LogicalId(Digest32);
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct DeltaId(ArenaKey);
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct UncertaintyId(ArenaKey);
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct ContextId(ArenaKey);
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct EvidenceId(ArenaKey);
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct MeasurementId(ArenaKey);
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct TransformId(ArenaKey);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Ratio {
    numerator: i64,
    denominator: NonZeroU64,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnknownReason {
    MissingEvaluator,
    UnsupportedFragment,
    Timeout,
    EvidenceMissing,
    Uncalibrated,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Measurement<T> {
    Exact { value: T, evidence: EvidenceId },
    Bounded { lower: T, upper: T, evidence: EvidenceId },
    Estimated { value: T, model_revision: Digest32 },
    Unknown(UnknownReason),
    NotApplicable { basis: EvidenceId },
    Incomparable { scope: Digest32 },
}
#[derive(Clone, Debug)]
pub struct TransformWitness {
    id: TransformId,
    source_context: ContextId,
    target_context: ContextId,
    comparison_scope: Digest32,
    defect: MeasurementId,
    declared_delta: DeltaId,
    uncertainty: UncertaintyId,
    gain: MeasurementId,
}
// No execution permit, trust-root writer or success bool belongs in this type.
```

```rust
// Complete small decoding unit; the frame-level admission is a separate caller.
#[derive(Debug, PartialEq, Eq)]
pub enum DecodeError {
    Truncated,
    OffsetOverflow,
    LimitExceeded,
}

pub struct Decoder<'a> {
    input: &'a [u8],
    offset: usize,
}

impl<'a> Decoder<'a> {
    pub fn new(input: &'a [u8], limit: usize) -> Result<Self, DecodeError> {
        if input.len() > limit {
            return Err(DecodeError::LimitExceeded);
        }
        Ok(Self { input, offset: 0 })
    }

    pub fn take(&mut self, length: usize) -> Result<&'a [u8], DecodeError> {
        let end = self.offset.checked_add(length)
            .ok_or(DecodeError::OffsetOverflow)?;
        let bytes = self.input.get(self.offset..end)
            .ok_or(DecodeError::Truncated)?;
        self.offset = end;
        Ok(bytes)
    }

    pub fn read_u32_le(&mut self) -> Result<u32, DecodeError> {
        let bytes = self.take(4)?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }
}
// A complete codec additionally validates tags, counts, ordering, scopes,
// digests, UTF-8, graph references and trailing bytes before admission.
```

SolverBackendの境界は `solve(&mut self, query: &CheckedQuery, budget: &RunBudget, cancel: &Cancellation) -> Result<SolverRun, SolverError>`。CheckedQueryはcontext/logic/domain/formula/optionsを束縛し、SolverRunは実行状態Completed/Timeout/ResourceExhausted/Unsupported/Failedと、task別LogicalResult（Satisfiability/Entailment/ModelCheck/Inference/Optimization/Intervention/SpatialRelation）を分離する。Sat/UnsatはSatisfiabilityの結果だけに使い、存在/全称、finite/infinite、brave/cautious、objective boundをtyped payloadへ保持する。solverにReceiptIssuerを渡さない。結果検証器がmodel/counterexample/proofと変換対応を検査し、host側の観測producerが別途証跡を発行する。

通常orchestratorは、(1)source admit、(2)parse/lower、(3)context＋再構成枝、(4)budget予約、(5)実solver、(6)結果検証、(7)四記録とgain、(8)frontier/schedule、(9)task consumer、(10)exact output、(11)host/replayの順を持つ。各段階はtyped handleを渡し、巨大JSONを関数間の接着剤にしない。

### 11. 最終hard gate

| Gate | 完了に必要な観測 |
| --- | --- |
| G00 | 全source/command/consumer台帳・snapshot一致・既存baseline |
| G01 | single source/arena、旧canonical preimage、binary golden/negative/fuzz |
| G02 | context再構成が第一級で、hard constraint/authorityを増幅しない |
| G03 | 実SMT・ASP＋宣言有限論理の正常例／反例／Unknown。model結果の独立再検査 |
| G04 | D/Delta/U/Gの観測と下流の選択・再探索が因果的につながる |
| G05 | S_Creativeの非空正例、再構成枝の勝者、比較不能と最適性scope |
| G06 | 非ユークリッド幾何と基線、実scheduler、蒸留による実state/query変化 |
| G07 | 旧fixture軸を使わないCPU/GPUの入力・mask・結果・selection対応 |
| G08 | task別completion、実修復→checkpoint→fresh resume、条件producer |
| G09 | source/time/regionに束縛したMedia/RPA/NLPの適用profile接続 |
| G10 | 資源・長期run・cache・replay・cleanup・build scratchの上限と障害試験 |
| G11 | legacy JSON／新binary／strict consumer／source・platform・plugin package |
| G12 | 通常facade→唯一runtime→推奨1＋代替→exact rendererの実接続 |
| G13 | required hostのprecommit/postsend/sink/durable replay実観測 |
| G14 | 全20辺のcut/swap/stale試験、別条件で再監査、日英文書整合 |
| G15 | 同一release identityで全gate再実行、必要なinstall/host pickup後の引渡し |

G13が必要なprofileでは、host capability不足をG13成功へ読み替えない。source-only達成と全体達成を別々に記録し、本ロードマップ全体は未完了とする。独立自然言語gold未提供や一般性能未実証は研究状態として保持できるが、実solver未接続・測定未実装・新候補未消費・binary未移行を研究pendingへ移して完成にしない。

受入fixtureは最低でも、保存変換、意図的欠損、許容前提再構成、禁止条件緩和、複数論理の不一致、共通誤parser、自然言語曖昧性、新規有効候補、奇抜だが無効候補、単純化、証拠失効、反例、比較不能、codec破損、cacheなし、crash/restartを含む。元問題と別公理系を区別する例では、domain/演算/記号対応を明示し、単なる「2+2」という文字列を異なる代数へ無断で解釈しない。

### 12. 監査・参照・作業境界

RepoSeiri 1.1の通常repository走査は100,000 entry上限に達したため、不完全な結果を成功根拠に使わなかった。その後、固定commitのcache非含有exportをsubtree scopeで走査し、397 entries・67 documents・limit exceeded 0を観測。source-session digestは `sha256:d440e908799e39bbfbdb466c5133fb3f56ce9422cd468c44024d02c19c16ad65`。この観測は基線文書の構造検査で、新ロードマップ実装の証明ではない。

Assurance-Compilerはdoctor一回の後、完全な16KiB以下のRust単位を補助監査した。`effect_error.rs`、supervisorの`filesystem.rs`／`media_snapshot.rs`はblocked_with_diagnostics、`host/stop_hook.rs`はneeds_evidence、すべてmissing_or_unknownを保持。未監査単位や大きなmoduleを安全と扱わず、断片化して範囲制約を迂回しない。

設計参考：[Z3の複数目的](https://microsoft.github.io/z3guide/docs/optimization/combiningobjectives/)、[Potassco/clingo](https://potassco.org/guide/)、[Spot](https://gitlab.lre.epita.fr/spot/spot)、[MAP-Elites](https://arxiv.org/abs/1504.04909)、[Conformal Risk Control](https://arxiv.org/abs/2208.02814)。参照技術の存在はEpistesys上の効果や対応済み状態を意味しない。校正を利用する場合はその統計的前提を確認する。

この文書の固定で行ったのは調査・設計・文書追加だけである。ソース実装、version変更、commit/push/merge、既存cache削除、plugin更新、host再起動は行わない。次の実装指示ではTLCX-00から依存順に進め、機能gate・互換gate・資源gate・host gateを全て現在snapshotで閉じる。


---


## English

### 0. Frozen status, decisions and reading coverage

- Design date: 2026-10-28 JST. Baseline: Epistesys 6.3.2-alpha.3, commit `1fca5165eca21011bcefd6518d3b3c9b93e17d58`, tree `fa5fffa50a4f512380b4fda2f869e0ff3ad88706`.
- Status: **design frozen / implementation not started**. Completing this document does not complete the proposed functionality. The next product version is not selected and is not changed by this task.
- Apply the latest explicit answer: **premise reconstruction is also a primary objective**. Preserve the original context as an immutable comparison anchor, but do not always subordinate reconstruction to the original-context answer. Preserve mandatory constraints, compare loss and gain, and return one recommendation plus important alternatives by default.
- The original goal is to measure transformation loss through actual heterogeneous logic solvers, acquire useful new structures and improve context-conditioned solutions. Safety/evidence management supports this goal; it does not replace reasoning, measurement or search.
- Team full-text reading covers **205 tracked source/test/configuration/schema files, 2,337,622 bytes and 62,905 lines, including 145 Rust files**. All post-read hashes matched. The [reading ledger](tlcx-source-reading-ledger-2026-10-28.json) records relative paths, working-tree SHA-256 and readers. External dependency implementations, generated binaries, ignored caches and personal runtime state are outside this owned-source scope. Reading is not exhaustive execution, zero unknown defects or a safety proof.
- Do not copy the private concept/background documents themselves into this repository. Preserve the [migration contract](migration-contract.md), provenance, rights and disclosure boundaries.
- Every implementation slice follows negative-first tests, minimum implementation, targeted verification, diff critique and exit records. Do not reclassify unmet required work as optional to claim completion.

2026-10-29 addendum: implementation was paused at the user's request while the [TL/S_Creative measurement contract v1 / finite-formal-v1.4](tlcx-tl-screative-measurement-contract-v1-2026-10-29.md) was fixed first. Initial status labels describe design-freeze history; consult the [progress/resume record](tlcx-implementation-progress-2026-10-28.md) for current state. The measurement contract takes precedence for definitions, algorithms, applicability, independent validation and selection effects. The 2026-10-29 v1.1 mathematical review was checked against current v1.3: already-corrected F-01–03 are retained as regression goldens, while the new interval-square kernel and explicit novelty-policy separation enter v1.4. After specification closure and the Kusuri notice, stop all session work; do not resume implementation without explicit instruction.

### 1. Architecture and the single normal path

```text
host/user source + explicit context + current authority
  -> bounded ingress / single immutable SourceStore
  -> DeepGrammar / structured context / unresolved interpretations
  -> ContextArena: original anchor + reconstructed premises + hard invariants
  -> actual solver portfolio / evidence-aware model validation
  -> TransformWitness { D, Delta, U, G } + comparison scope
  -> contextual loss/gain frontier + one recommendation + key alternatives
       ^                 |
       |                 +-> geometric scheduler -> new solver/query/branch
       +---- discrete <-> continuous proposal / independent validation
  -> task-specific Program IR / verification or coding repair
  -> exact final rendering / output contract
  -> host precommit / post-send / sink / durable replay
```

The current normal entry returns an analysis pipeline as a JSON candidate; it has no production solver/S_Creative selection path. Add `epistesys-solve` and route facades through the same `lc631-runtime::solve`. Preserve old `lc631-dgcl-run` wire contracts through compatibility adapters; doctors are not production substitutes.

Add only three crates where ownership requires them:

| Crate/layer | Owns | Does not own |
| --- | --- | --- |
| New `lc631-substrate` | Digest32, typed IDs, source pools, arenas, checked binary codecs, budget value types | Solvers, authority, execution I/O |
| `lc631-core` | Shared context/claim/decision types and existing authority types | Grants generated from scores |
| `lc631-tldg` / `lc631-analysis` | Parsing, AST/context lowering and existing coding validation/repair | Claims of complete arbitrary-language understanding |
| `lc631-tl` | Four-facet records, comparison scopes, loss measurements, legacy defect adapters | Scheduling and operation authority |
| `lc631-world` | Context branches, frontiers, creative candidates/gains, geometric search allocation | Automatic factual or execution-authorization commitment |
| New `lc631-solvers` | SMT/ASP/finite-model adapters and query/result protocols | Turning prose into truth or owning trusted issuers |
| `lc631-process-supervisor` | Actual child/pipe/filesystem/temporary ownership | Logical optimality |
| New `lc631-runtime` | Single orchestrator, budget broker, storage, resume and task-specific consumers | CLI syntax or renderer-specific decisions |
| `lc631-accelerator` | Meaningfully sourced numeric batches, leases/fences and parity | Treating fixture values as semantic loss |
| `lc631-media` | Source/time/region-bound observation candidates | Unsupported identity/emotion/intent assertions |
| `lc631-host` / `lc631-receipt-kernel` | Current authority, exact output, authenticity and replay | Converting hypothetical premises into permissions |
| `lc631-wire` / `lc631-compat` / CLI | Bounded DTOs, old schemas, presentation and arguments | Giant JSON as canonical internal state |

Dependencies flow `CLI -> runtime -> analysis/tldg/world/tl/solvers/host/media`. Solvers use core/supervisor, world uses core/tl, tl uses core, and core/receipt-kernel use substrate. Wire renders shared public types. Lower layers never depend back on runtime. Put RunBudget/Cancellation/Reservation contracts in substrate or supervisor and small JournalRead/JournalAppend/EvidenceLookup ports in core; runtime owns implementations and injects borrowed interfaces. DecisionArtifact types live in core; runtime/decision constructs them. Forbid world→tldg/accelerator reverse dependencies and bridge through runtime adapters. Add cargo-metadata allowed-dependency tests in TLCX-01. Limit traits to real boundaries and prefer small implementations and solver enums.

Keep the existing [architecture](system-architecture.md) as a description of current implementation. Do not present this roadmap's future features as implemented in the README.

### 2. Context reconstruction, objectives and comparison

A `ContextRevision` references original source, parent context, premises, domain, logic semantics, hard invariants, goal policy, comparison probes and typed Delta. Premises retain `Given/Observed/Assumed/Derived/Proposed/Refuted/Conflicted`. Recasting evidence as an assumption never erases its original observation.

Fixed mandatory constraints, operation authority and evidence authenticity are not search variables. Identify mutable premises and uncertain premises requiring clarification. Changes create new ContextIds with applicability conditions; they are not silently presented as solutions to the original problem. Do not obtain apparent gains by changing context weights, goals or denominators.

Compare only within one `ComparisonScope`: question, baseline, scales, units, probes, goal policy and domain. Return `Incomparable` outside it; unmeasurable is not zero. Retain Pareto alternatives and select one recommendation with declared task preferences and a stable tie-break. If genuine incomparability remains, label the selection an operational recommendation with reasons, not a unique optimum.

Optimality states are `ModelOptimal/BestAmongEvaluated/BudgetLimited/Incomparable`. Distinguish formal-model optimality evidence, superiority among evaluated candidates and exhausted budgets. Evaluating creativity never automatically authorizes edits or network retrieval.

### 3. Four-facet records and measurement

Each transformation has `TransformWitness(D, Delta, U, G)`. These are four perspectives, not exclusive categories: intentionally violating a mandatory condition appears in both Delta and D.

- **D**: preservation defects, measured by domain-specific reconstruction/semantic/temporal/spatial evaluators. Do not collapse structural defects, verification failures and unsupported coverage.
- **Delta**: abstraction, substitution, premise addition/retraction, rebinding and representation changes. Record typed source-to-target edits and separately validate their admissibility.
- **U**: unverified, timeout, unsupported, uncalibrated, conflicting and incomparable states. Do not fabricate probabilities.
- **G**: baseline-relative gains in coverage, compression, bridging, prediction, reorganization, design, transfer and problem solving. Separate predicted gains from observations.

`Measurement<T>` uses `Exact/Bounded/Estimated/Unknown/NotApplicable/Incomparable`, with intervals, units, denominators, checked scope, evaluator revisions and evidence IDs. One known defect and two unknowns among ten equally weighted binary probes yields [1/10,3/10], not a statistical confidence interval or coverage of unextracted obligations.

Restore original S_Creative v1/v2 components in `CreativeAssessment`, separating original raw scores from revised decision policies. Preserve original v2 Complexity Parity as the symmetric exponential formula; different treatment of simplification requires another FormulaRevision. Default JointPareto separates migration costs from meaning-preserving simplification gains and avoids unintended duplicate TL penalties. Retain gain, novelty, compatibility, migration cost and counterexample risk separately. Knownness is relative to a fixed archive, not a blanket prohibition on safe established methods. Consult the measurement contract for details.

Route defects by cause: omissions to local repair, ambiguity to interpretation branches, counterexamples to reconstruction, missing evaluators to alternative checks, repeated residuals to different representations, and exhausted budgets to qualified partial answers. Compose paths by source obligation correspondence, without repeatedly counting the same loss or gain.

### 4. Actual solvers, geometry and distillation

Initial required profiles cover SMT Boolean/QF_LIA, finite ground normal-rule ASP with explicit default negation, finite temporal traces, four-valued paraconsistency, finite Kripke profiles (modal K, epistemic S5, finite intuitionistic and explicitly selected deontic semantics), finite acyclic SCMs and finite spatial relations. Do not claim unbounded higher-order or arbitrary-language coverage. Run actual SMT and ASP backends on Windows/Ubuntu and pin versions/binaries/options/licenses in TLCX-00. Other profiles also require executable finite evaluators and independent small-model oracles, not enum-only implementations.

Do not vote across solver results. Distinguish SAT/UNSAT from entailment, brave/cautious consequences, default/classical negation and support/refutation from missing observations. Each adapter reports translation D/Delta/U at ingress and reconstruction D/Delta/U at egress. Uncheckable proofs remain `SolverReported`, not proof-verified.

Geometry is exclusively for search scheduling. Build hierarchical embeddings, directional costs and context-dependent local metrics from observed transitions, validation costs and gains. Require at least one actual non-Euclidean implementation, such as a hierarchical Poincare-ball embedding, plus a non-geometric baseline. IDs/enumeration order are not semantic coordinates. Test curvature rationale, numeric domains, NaN/boundaries, self-distance and relabeling invariance. Use separate types for distance and directed transition cost. Geometry predictions are not independent TL evidence.

Scheduling balances expected improvement, important uncertainty, diversity and costs, with exploration/fairness/deadline/concurrency limits. Geometry cannot amplify budgets or authority. Close discrete→continuous→proposal→independent validation→discrete-state updates. Admit new relations via dedicated observations rather than only existing-edge membership; candidates cannot modify their evaluation oracle.

Preserve the old 256×8 diagnostic contract explicitly. New search has 256 base exploration slots, at most 2048 live evaluation slots and bounded eviction, without claiming semantic independence from distinct IDs. Report unconfirmed distinctness; profiles requiring a minimum explored count cannot pass when short. Histories beyond live capacity use summaries, rebuildable indexes and necessary evidence rather than accumulating full copies.

### 5. From internal JSON to typed Rust and binary boundaries

Typed Rust arenas and shared source are canonical in memory. Do not serialize every internal function call. Use binary frames at persistence, IPC, GPU-transfer and checkpoint boundaries. Re-embedding base64 binary inside giant JSON does not solve the problem.

Concrete replacements:

| Current form | Replacement |
| --- | --- |
| Per-character `BoundaryCell.surface` String | Shared source plus UTF-8 byte span/class |
| Deep-cloned selected AST/alternatives/contracts | Arena AST IDs and immutable views |
| Entire pipeline as candidate_text plus outer pipeline | One artifact and bounded summary; legacy full DTO only explicitly requested |
| `finalize_with_pipeline -> serde_json::Value` | Typed PackageFinalizationReport |
| Repair/resume `from_value(report[...].clone())` | Typed claims/snapshot borrowing or ownership transfer |
| Repeated whole-JSON serialization for digests | Stream-hashed versioned canonical binary |
| String digests/nonces/fixed labels | Digest32/Nonce16/enums/interned SymbolId |
| Whole JSON-journal rereads | Bounded segments, verified heads and rebuildable indexes |
| Whole stdout/frame/PCM retention | Bounded streaming with required payloads admitted into arenas |

Public JSON, Cargo metadata and existing Python framed JSON may remain boundary adapters. Never remove/rename/reinterpret old public fields. Do not add fields blindly to `additionalProperties:false` consumers; use new schemas and explicit negotiation. Old commands retain old shapes; new facades select compact views.

Binary v1 has a 64-byte header: magic[8], major:u16, minor:u16, flags:u32, payload_len:u64, section_count:u32, header_len:u32, frame_digest[32]. A 32-byte section entry contains tag:u16, version:u16, flags:u32, offset:u64, length:u64, count:u32 and reserved:u32. Use little endian, zero reserved values, checked arithmetic, nonoverlapping ranges and pre-allocation limits. Reject unknown critical tags/major versions; skip or explicitly retain unknown optional fields as opaque bytes. Separate schema and internal limits.

Binary-v1 magic is ASCII `EPITLCX1`; header_len is fixed at 64 and the directory follows immediately. payload_len excludes the directory. Offsets are from frame start; section_count<=64 and directory_len=count×32 use checked arithmetic. Require header flags=0, allow only section-flag bit0 for Critical and reject other bits, ascending unique tags, rejection of unknown flags, zero reserved fields, no padding/unclaimed gaps, contiguous sections in directory order and an exact total-frame length. FrameDigest binds a domain separator, the header with its digest field zeroed, directory and all payload bytes. Hash equality is not authentication; only a trusted receipt binding FrameDigest supports a separate authenticity decision. Retain unknown optional sections as uninterpreted bytes when signed roundtrips are required; discarded sections cannot be reencoded while claiming the original signature survives.

Forbid raw Rust-struct dumps, persisted pointers, usize wire values, enum transmute and unchecked mmap references. Define GPU POD transfer layout separately from persistent wire. Verify existing MAC/replay hashes using old canonical bytes; migrate to new domains explicitly with migration receipts. Storage migration never revives old Grants.

Rust type layout is not a persistent-wire guarantee; use explicit codecs, as motivated by the [Rust Reference](https://doc.rust-lang.org/reference/type-layout.html). Bound actual reads rather than checking metadata before unbounded reads. [Read::take](https://doc.rust-lang.org/std/io/trait.Read.html#method.take) is a component, not a substitute for allocation/deadline/aggregate budgets.

### 6. Caches, resources and resume

Use the following starting runtime budget profile. These are engineering limits to measure and pin as configuration in TLCX-00/37, not established performance results or semantic thresholds.

| Resource | Initial profile |
| --- | --- |
| Source | Preserve the current 256KiB DGCL limit; larger documents need source-mapped chunks |
| Rust-owned live state | Reservation ceilings of 128MiB/run and 512MiB/process |
| Concurrent external solvers | 2; separately report actual OS child-memory enforcement |
| New binary frame | At most 16MiB, checked after header and before allocation; segment large proofs |
| Normal summary | 64KiB; revision-bound cursor pages for details; old full views preserve explicit limits; unbounded entries gain bounded reads/streaming |
| Recomputable disk cache | Off by default; explicit enable allows 256MiB total, 60-minute TTL and entry/byte limits |
| Runtime scratch | 256MiB/run, 1GiB aggregate; large media needs a separate explicit reservation profile |
| Durable evidence | 4MiB segments, 256MiB store ceiling; full storage blocks admission or requests export, not silent deletion |
| Telemetry | Online statistics plus bounded samples; complete redundant receipt copies only for explicit diagnostics |

Use distinct ownership types for `OwnedTemp/CacheLease/PinnedEvidence/DurableJournal/SecurityReplayState/ModelStore/BuildCache`. Never LRU-evict sole evidence copies, consumed nonces, revocations or trusted heads. Without safe security-state compaction, reject admission. Recover reservations from manifests/file identities after crashes without taking another session's state.

Cache keys include context/source/goal/probe/logic/solver/options/model/tool/validator/codec revisions. Hits never restore current authority, expiry, revocation or host delivery. Under a fixed logical work plan, seed and solver configuration, require equivalent results across cold/warm/corrupted/evicted caches. Equal wall budgets may explore more and recommend differently with caches, but authority, applicability and evidence states remain unchanged. EvidenceStore retains necessary selection/replay/revocation/repair witnesses; telemetry compression cannot discard sole receipts.

Reclamation checks owned roots/markers/resolved identities/active leases/reparse points/hardlinks/retention and only reclaims authorized recomputable objects. Stop on locked files/failure, retaining `CleanupFailed/Retained` and reservations. Drop is best effort; explicit successful close is reclamation evidence. This is not permission to bulk-delete existing user caches.

Separate logical artifacts from physical records. Each IPC chunk and journal record is at most 1MiB and binds object/transfer ID, sequence, total bytes, terminal marker and whole-object digest. Do not split records across 4MiB segments. Persist large checkpoints/proofs as reserved chunks/references rather than serializing 128MiB at once. Partial receive/append never publishes a complete artifact. Preserve the old 2MiB stdin cap in legacy profiles. Larger solver input requires an explicit streaming profile with max_stdin_total, supervised 1MiB chunks, aggregate limits and deadlines; never silently raise limits.

Pin device-specific max_allocated_bytes/max_inflight_bytes/max_buffers/retired_until_fence_bytes from device limits and measurements in TLCX-00; reject GPU admission when unspecified. Account for all runtime-owned device allocations and separately report opaque framework reservations. Enforce aggregate shared-store quotas through cross-process locking/atomic reservations; per-process caps alone are not aggregate guarantees.

Require joint-rollback detection only with an external trusted monotonic anchor. Without one, retain RollbackResistanceUnavailable and require fresh validation; a head inside the same store cannot detect joint rollback. If a host-integrated profile requires an external anchor, register its provision as an external hard dependency in TLCX-00. Compact nonces only after conditions such as expiry of all affected receipts with clock requirements, or verified retirement of old key epochs that can never be accepted again. Do not evict indefinite receipts or unclosed epochs.

Keep build targets, model weights and runtime caches separate. Eliminate implicit production cargo builds; development fallback is explicit. Separate test-owned scratch from shared targets and bound success cleanup/failure retention. Do not claim large GPU-dependency builds consume zero space.

Contract artifact lifetimes behind compact summaries. The Full profile requires explicit storage authority and an owned EvidenceStore; inspect/resume checks read capabilities for the same store/context/revision. Cache-off disables recomputable caching, not required evidence retention. Without a store, ephemeral mode returns only self-contained inline results and must not advertise artifact references or resume after process exit. Report page-cursor expiry, retention and authorization invalidation; never reclaim the sole evidence supporting a live reference.

### 7. Rust discipline and connection-completion criteria

- Default to safe idiomatic Rust; no unsafe in semantic cores/codecs. Isolate existing OS/GPU unsafe in small modules with maintained contracts.
- Write invariants, invalid states, owners, errors, budgets and consumers before types/implementation.
- One responsibility per module. Use 400 source lines/module and 60 lines/function as review guidelines, not mechanical splitting rules; document exceptions and avoid excessive generic/macro machinery. Split existing monoliths at real boundaries before adding features.
- Option presence never means Verified/Completed. Prefer typed Result and explicit states. Production unwrap/expect requires a statically justified invariant.
- Share immutable source with Arc; use single-owner arenas and borrowed views. Avoid deep Clone defaults. New canonical digests must not depend on HashMap ordering or Debug text.
- Use SoA only for numeric hot paths, checked fixed-width integers and finite-float wrappers. Do not add unsafe/SIMD/GPU without relevant measurements.
- Connect new interfaces to actual consumers introduced by that slice. Do not require future final consumers as earlier slice exits. Register deferred consumers and revalidation slices; final G14 requires actual positive observations and causal cut results for every edge.

Completion means actual data traverses required-profile `input→generation→execution→observation→consumption→selection→output→resume`, and cutting each edge fails appropriately. Types, doctor fields, equal hashes and vacuous all() tests over empty candidates are not completion evidence. Separate engineering closure, model optimality, general research effects and host delivery. Missing required host callbacks remain an incomplete hard gate.

### 8. Common slice execution contract

Record dependency snapshots, target paths, observed negative failures, implementation deltas, positive/boundary tests, producers/consumers, invalidation, unobserved scope and exit decisions. Use bounded summaries, not giant pipeline dumps.

Targeted checks include `cargo fmt --all --check`, relevant crate tests and required property/schema/codec/negative integration. At milestones/final gates run `cargo check --workspace --locked`, `cargo test --workspace --no-default-features --locked`, `cargo test --workspace --locked`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, and `cargo build --workspace --release --locked`. Record Windows/Ubuntu, actual solvers, configured JA/EN workers, actual GPU and fresh packages separately. Unsupported platforms do not pass.

Prefix tests with their slice IDs and register fixture→test→production entry→consumer. Stop for public-schema breakage, source/span corruption, material conflicts with existing changes or unverifiable hard gates, not expected fail-first failures. Do not weaken scope, budgets or claims to make gates pass.


Pin the OS/backend matrix in TLCX-00: Windows x86_64 requires portable core, actual SMT/ASP, the current configured JA/EN profile and the target host; Ubuntu requires portable core and actual SMT/ASP. Requiring configured NLP on Ubuntu additionally requires runtime/model pins and actual positives. Declare GPU/Media per device/backend and do not substitute another OS's results. Record host/version, precommit/postsend/sink entrypoints, issuers, external anchors, needed install/activation authority and external owners upfront. Slice 35 implements adapter/source contracts, 36 packages them, 42 establishes candidate readiness and 43 observes actual-host G13/G15 after authorized activation; never invert that order.

### 9. Detailed dependency-ordered roadmap

Unless otherwise specified, paths are relative to `scripts/lc631/crates/lc631-<name>/` (for example, `analysis/src` expands to `scripts/lc631/crates/lc631-analysis/src`). `substrate/solvers/runtime` are new crates; `(new)` denotes proposed additions, not existing files. Each slice depends on the explicit DAG below. Proceed in numeric order by default, but independent slices may advance while external observations are pending. Preserve all earlier contracts and never edit overlapping slices concurrently.

#### TLCX-00: Baseline, full-source ledger and required profiles

- Depends on: the fixed baseline and explicit design decisions.
- Targets: `Cargo.toml; .github/workflows; docs/tlcx-source-reading-ledger-2026-10-28.json`.
- Implementation/wiring: Pin existing changes, every producer/consumer, public command, canonical preimage, signed domain, solver binary/version/license and OS profile. Reject empty positive tests and doctor-only evidence.
- Negative→positive tests: Missing files, changed snapshots or missing required backends must fail readiness. Capture cold semantic results, output bytes, RAM/VRAM, disk growth and latency on identical seeds.
- Exit: Register complete source coverage, locks, at least two actual solver families plus finite-logic profiles, comparison conditions and budgets. Separate serious baseline failures into prerequisite repairs.

#### TLCX-01: Separate contracts and establish cut tests

- Depends on: slice-local exits of TLCX-00.
- Targets: `core/src/context.rs (new); runtime/tests/closure_cuts.rs (new)`.
- Implementation/wiring: Separate ContextRevision, AuthorityRevision, EvidenceRevision, TaskKind and DecisionStatus. Keep D/Delta/U/G separate from effect and completion states.
- Negative→positive tests: Reject Grants from reconstructed premises, unknown-as-zero loss, unsupported-as-SAT and empty all() positives. Positives require nonempty candidates and consumed results.
- Exit: Register new Cargo crate skeletons, common protocol types, a NotConnected runtime entry and every cut point. Initial green contract tests require NotConnected/Unsupported. Do not leave future positives permanently red/ignored: introduce them fail-first in their implementation slices, then make them green; G14 requires all positives.

#### TLCX-02: Single source and typed arenas

- Depends on: slice-local exits of TLCX-01.
- Targets: `substrate/src/{ids,source,arena}.rs (new); tldg/src/source.rs; analysis/src/dgcl_artifact.rs`.
- Implementation/wiring: Implement one Arc<str> source, Digest32, SymbolId, AstId, ConditionId, TaskId, EvidenceId and generation-checked references. Render old reports through borrowed views.
- Negative→positive tests: Reject invalid UTF-8 boundaries, cross-context refs, reused generations, duplicate IDs and allocation failures. Selected AST and alternatives reference one canonical object.
- Exit: Preserve source bytes/spans and legacy results while removing deep AST/task duplication from the normal path; connect source, arena and old DTOs.

#### TLCX-03: Bounded canonical binary codec

- Depends on: slice-local exits of TLCX-02.
- Targets: `substrate/src/codec/{header,sections,encode,decode}.rs (new)`.
- Implementation/wiring: Implement the specified 64-byte header and 32-byte section entry with a safe explicit codec, streaming hashes, fixed tags, length-prefixed strings, sorted maps and unknown-critical rejection.
- Negative→positive tests: Exercise truncation, length overflow, duplicate/overlapping sections, invalid UTF-8, unknown major versions, trailing bytes and huge counts; validate independent golden bytes.
- Exit: Encoding, decoding and canonical hashes are platform-independent. Actual ingress uses bounded readers; no raw Rust-memory dumps.

#### TLCX-04: Legacy JSON and signed-domain compatibility

- Depends on: slice-local exits of TLCX-03.
- Targets: `wire/src/legacy_codec.rs (new); receipt-kernel/src; analysis/src/*digest* callers; schemas`.
- Implementation/wiring: Separate internal types from legacy canonical encoding; verify old signatures against old bytes. Give binary data a new domain and select strict DTO versions explicitly.
- Negative→positive tests: Reject old MACs in new domains, promotion by rehashing and strict-v1 breakage through unknown fields. Preserve existing fixtures and golden MAC/hash vectors.
- Exit: Complete the canonical-preimage migration/consumer map and preserve legacy semantics and signature verification across typed conversion.

#### TLCX-05: Shared resource broker and bounded I/O

- Depends on: slice-local exits of TLCX-02, TLCX-04.
- Targets: `runtime/src/resource.rs (new); process-supervisor/src/bounded.rs; CLI io; host readers`.
- Implementation/wiring: Connect pre-allocation RAM/disk/output/proof/child reservations, deadlines, cancellation and cleanup budgets to one broker. Remove unbounded stdin/file/Command::output paths.
- Negative→positive tests: Test file growth after metadata, runaway solver output, stuck pipes and escaped/unconfirmed child cleanup; observe reservation return after successful completion.
- Exit: Every execution entry has a resource owner; limit failures are typed. Distinguish actual OS enforcement from observational limits.

#### TLCX-06: Separate caches, evidence and replay state

- Depends on: slice-local exits of TLCX-05.
- Targets: `runtime/src/store/{cache,lease,recovery}.rs (new); receipt-kernel::ReplayGuard; test helpers`.
- Implementation/wiring: Separate recomputable cache, active leases, durable evidence, security replay, weights and build caches. Disk caching defaults off; enabled caches require quotas, TTLs and ownership manifests.
- Negative→positive tests: Reject replay after eviction, reclaiming live GPU buffers, deleting unknown roots, losing reservations after crashes and force-deleting locked files.
- Exit: For fixed work plans/seeds, cold/warm/evicted/corrupt caches preserve results. Measure extra exploration under equal wall budgets separately. Authority boundaries remain invariant; account failed reclamation as Retained and leave user caches untouched.

#### TLCX-07: Binary journals and safe migration

- Depends on: slice-local exits of TLCX-04, TLCX-06.
- Targets: `analysis/src/closure_resume.rs; host/src/lib.rs; runtime/src/store/journal.rs (new)`.
- Implementation/wiring: Implement bounded segments/indexes/heads. Validate legacy JSON chains read-only with legacy codecs and migrate explicitly; apply limits to every reader.
- Negative→positive tests: Test torn appends, segment swaps, stale heads, compaction crashes and sole-evidence deletion. Detect joint rollback with an external anchor; without one test RollbackResistanceUnavailable. Normal resume rechecks fresh authority/source.
- Exit: Remove whole-history parsing on every append. Corrupt indexes rebuild or Hold without fabricating evidence; no automatic evidence deletion without retention policy.

#### TLCX-08: First-class context and premise reconstruction

- Depends on: slice-local exits of TLCX-01, TLCX-02.
- Targets: `core/src/context.rs; world/src/context_graph.rs (new); analysis context lowering`.
- Implementation/wiring: Keep OriginalContext as an immutable anchor and make reconstructed ContextDelta branches first-class search targets. Hard invariants and operation authority remain immutable.
- Negative→positive tests: Reject higher scores by dropping mandatory constraints, overwriting originals, ID-only duplicate worlds and promoting assumptions to Observed.
- Exit: The actual ContextGraph consumer checks parent/delta/applicability and fixed comparison scopes. Revalidate recommendation consumption in 23 and explanation in 29.

#### TLCX-09: DeepGrammar to task-specific logical forms

- Depends on: slice-local exits of TLCX-04, TLCX-08.
- Targets: `analysis/src/{instruction_grammar,dgcl_pipeline,dgcl_projection,completion_plan}.rs`.
- Implementation/wiring: Separate mathematics/reasoning/explanation/coding/media TaskKind policies. Lower source-bound ASTs into typed formulas and condition predicates while retaining unsupported natural-language spans.
- Negative→positive tests: Reject Cargo-only evidence plans for explanations, unknown-as-true conditions, default/classical-negation collapse and invented reference bindings.
- Exit: Connect one artifact to ContextIR and Program IR with task-specific evidence obligations; do not bypass them by marking everything N/A.

#### TLCX-10: Actual solver registry, protocols and supervision

- Depends on: slice-local exits of TLCX-05, TLCX-09.
- Targets: `solvers/src/{registry,protocol,runner}.rs (new); process-supervisor`.
- Implementation/wiring: Register semantics/version/options/domain capabilities, send typed queries through argv/stdin and parse bounded models/proofs/counterexamples.
- Negative→positive tests: Reject command injection, wrong binaries/scopes, timeout-as-UNSAT, truncated-output-as-SAT and stale result reuse.
- Exit: A typed ResultStore introduced here actually consumes process query/result/evidence identities. Revalidate final TL consumption in 17 and selection in 23; mocked backends cannot satisfy required profiles.

#### TLCX-11: Actual SMT solving and optimization

- Depends on: slice-local exits of TLCX-10.
- Targets: `solvers/src/smt.rs (new); fixtures/tlcx/smt`.
- Implementation/wiring: Start with declared Boolean/QF_LIA fragments. Reevaluate SAT models and distinguish UNSAT/proof states, objective bounds/gaps and timeouts; escape symbols through a typed emitter.
- Negative→positive tests: Reject incorrect models, undeclared symbols, unbounded objectives, unsupported quantifiers and SAT-as-optimal claims.
- Exit: Model validators and ResultStore consume actual integer-constraint results. Revalidate condition-producer consumption in 28 and candidate comparison in 23.

#### TLCX-12: ASP exceptions and nonmonotonic reasoning

- Depends on: slice-local exits of TLCX-10.
- Targets: `solvers/src/asp.rs (new); fixtures/tlcx/asp`.
- Implementation/wiring: Connect finite ground normal rules, explicit default negation, brave/cautious consequences and costs to clingo; record grounding bounds and assumptions.
- Negative→positive tests: Reject implicit closed-world assumptions, brave-to-cautious promotion, grounding explosions, lost exceptions and arbitrary collapse of multiple models.
- Exit: Adding an exception changes conclusions correctly, with separate original/reconstructed context bindings.

#### TLCX-13: Temporal logic, ordering and counterexample traces

- Depends on: slice-local exits of TLCX-10.
- Targets: `solvers/src/temporal.rs (new); core temporal types; media event adapter`.
- Implementation/wiring: Declare finite-trace semantics for before/after/until, reference times and units; infinite-trace checking requires a separate capability and actual model checker.
- Negative→positive tests: Reject finite-prefix-as-permanent-guarantee, absolute/relative-time collapse and lost boundary/loop counterexamples.
- Exit: An executable evaluator returns satisfaction/refutation/unknown and counterexample traces to ResultStore. Connect/revalidate temporal TL in 19 and conditional tasks in 28.

#### TLCX-14: Paraconsistent and four-valued reasoning

- Depends on: slice-local exits of TLCX-10.
- Targets: `solvers/src/four_valued.rs (new); core truth-state types`.
- Implementation/wiring: Evaluate finite rules with independent support/refutation bits. Separate classical UNSAT from retained inconsistency and record its dependency scope.
- Negative→positive tests: Reject explosion from p/not-p to unrelated q, Neither-as-False and hiding Both as mere unknown.
- Exit: Match independent truth tables/exhaustive small models and observe consumption by ResultStore/context branches. Revalidate selection/final-explanation effects in 23/29.

#### TLCX-15: Finite modal, intuitionistic, epistemic and deontic profiles

- Depends on: slice-local exits of TLCX-10, TLCX-14.
- Targets: `solvers/src/kripke/{model,evaluate,profiles}.rs (new)`.
- Implementation/wiring: Implement finite Kripke models with explicit frame conditions, intuitionistic persistence, epistemic accessibility and a strict separation of norms from operation authority.
- Negative→positive tests: Reject label-only logic changes, invalid frames, O(action)-as-Grant and unlicensed classical equivalences.
- Exit: Every declared profile has positive/counterexample/independent-small-model tests and emits projection-loss correspondences.

#### TLCX-16: Causal, spatial and counterfactual evaluation

- Depends on: slice-local exits of TLCX-10, TLCX-13.
- Targets: `solvers/src/{causal,spatial}.rs (new); core context schemas`.
- Implementation/wiring: Execute interventions versus conditioning in finite acyclic SCMs and finite coordinate/containment/connectivity relations; separate temporal causation from semantic dependence.
- Negative→positive tests: Reject correlation-as-causation, invented structural equations, coordinate-frame mixing and intervention-based hard-constraint removal.
- Exit: Return actual counterfactual/spatial results and missing information to typed ResultStore. Revalidate D/Delta/U consumption in 17/19 while retaining unspecified information as Unknown.

#### TLCX-17: Canonical four-facet ledger

- Depends on: slice-local exits of TLCX-04, TLCX-09, TLCX-10.
- Targets: `tl/src/{transform,measurement,composition}.rs (new); projection_v3 adapter`.
- Implementation/wiring: Store TransformId, source/target/context, direction, measurement scope, D/Delta/U/G and witnesses in one canonical ledger; adapt the existing defect graph as a view.
- Negative→positive tests: Reject duplicate loss/gain counting, incompatible-unit sums, immunity for deliberate changes and promotion of unsupported target-only facts.
- Exit: Connect existing solver ingress/egress and premise changes to one reverse-traceable ledger. Revalidate rendering in 29 and host continuity in 35/43.

#### TLCX-18: Reconstruction and semantic-loss measurement

- Depends on: slice-local exits of TLCX-11, TLCX-17.
- Targets: `tl/src/measure/{reconstruction,semantic}.rs (new); solvers query adapters`.
- Implementation/wiring: Measure reconstruction without access to the original source and preservation of fixed probes, entailment, quantification, negation and scope; distinguish exact/bounded/estimated values and unmeasured scope.
- Negative→positive tests: Reject source-copy zero reconstruction loss, loss reduction by shrinking probe denominators and agreement caused by a shared bad translator.
- Exit: Known mutations change measured direction/bounds correctly and are consumed by the ledger. Revalidate causal effects on selection/repair in 23/31.

#### TLCX-19: Temporal and spatial-loss measurement

- Depends on: slice-local exits of TLCX-13, TLCX-16, TLCX-17.
- Targets: `tl/src/measure/{temporal,spatial}.rs (new); temporal/spatial solver outputs`.
- Implementation/wiring: Pin domain-specific units/probes for ordering, duration, reference time, placement, connectivity and coordinate transforms; evaluate forward/reverse separately.
- Negative→positive tests: Reject penalizing valid time shifts, accepting reversed order, mixing image/world coordinates and unknown-as-zero measurements.
- Exit: Measurements and the ledger consume typed relations from existing temporal/spatial solvers; out-of-scope cases remain Incomparable. Revalidate complete natural-language/media integration in 30.

#### TLCX-20: Cross-logic correspondence and disagreement analysis

- Depends on: slice-local exits of TLCX-11, TLCX-12, TLCX-14, TLCX-15, TLCX-18, TLCX-19.
- Targets: `tl/src/cross_logic.rs (new); solvers result normalization`.
- Implementation/wiring: Compare queries, model quantification, premises and shared-extractor dependencies; construct explanatory bridge witnesses instead of voting.
- Negative→positive tests: Reject SAT/entailment comparisons, brave/cautious collapse, counting shared parser errors as independent votes and silent contradiction removal.
- Exit: Build typed ReprojectionRequests from disagreement causes and validate them through query/context consumers. Revalidate actual scheduler feedback in 25 and final explanation in 29.

#### TLCX-21: Creative generation and premise reconstruction

- Depends on: slice-local exits of TLCX-08, TLCX-20.
- Targets: `world/src/{creative,operators,archive}.rs (new); runtime CandidateProvider`.
- Implementation/wiring: Execute typed preservation/rebinding/reframing/abstraction/counterexample-driven premise operators; accept LLM proposals only as source-bound candidates from explicit providers.
- Negative→positive tests: Reject label-only novelty, unsupported factual additions, authority changes and renamed duplicates.
- Exit: Generate genuinely new candidate structures with a positive path through independent validation; report deterministic operators and LLM adapters as separate capabilities.

#### TLCX-22: Component-wise S_Creative measurement

- Depends on: slice-local exits of TLCX-17, TLCX-21.
- Targets: `world/src/creative/{novelty,compatibility,gain,cost}.rs (new)`.
- Implementation/wiring: Observe original v1/v2 novelty, compatibility, structural/reorganization/utility gains against a fixed baseline; retain corpus revisions and unknown novelty scope.
- Negative→positive tests: Reject offsetting mandatory defects with novelty, double TL penalties, unjustified simplification penalties, candidate-edited oracles and estimated gains presented as observed.
- Exit: Nonempty CreativeAssessments retain component-wise observation changes and are consumed by typed comparison inputs. Revalidate causal frontier effects in 23; retain useful established methods.

#### TLCX-23: Joint optimization of contexts and solutions

- Depends on: slice-local exits of TLCX-18, TLCX-19, TLCX-20, TLCX-22.
- Targets: `world/src/{frontier,selection,optimality}.rs (new)`.
- Implementation/wiring: Compare original and reconstructed context/solution pairs under fixed goals, hard invariants and comparison scopes; select one recommendation plus key alternatives from the frontier.
- Negative→positive tests: Reject winning by weakening the problem, incomparable-as-low-score, budget exhaustion as global optimality and silent loss of non-top16 candidates.
- Exit: Bind recommendation, alternatives, rejection reasons and optimality scope to one DecisionArtifact; include a positive case recommending a reconstructed context.

#### TLCX-24: Non-Euclidean search geometry

- Depends on: slice-local exits of TLCX-08, TLCX-17, TLCX-23.
- Targets: `world/src/geometry/{graph,poincare,directed_cost}.rs (new); tldg legacy adapter`.
- Implementation/wiring: Base hierarchical embeddings on actual transition graphs and separate directed cost. Build Poincare norm-squares with the dedicated nonnegative interval `square` operation; retain a non-geometric baseline. Keep `DistanceToFrontierUnion-v1` and `NearestVerifiedCandidate-v1` as distinct `SchedulerNoveltyPolicyId` values; do not replace the current union policy without paired measurement. Avoid unsupported semantic-distance claims.
- Negative→positive tests: Reject ID-order ranking changes, nonzero self-distance, negative interval-square lower bounds, reversed `n_geo` endpoints, input-box width treated as numeric error, invalid ball boundaries/NaN/overflow and truth/Grants from geometry (measurement-contract G31–G37).
- Exit: Implement at least one actual non-Euclidean computation with invariance tests and a typed SchedulingProposal consumer. Revalidate actual scheduling choices in 25; keep canonical TL independent.

#### TLCX-25: Loss/gain/uncertainty-based resource allocation

- Depends on: slice-local exits of TLCX-05, TLCX-23, TLCX-24.
- Targets: `world/src/scheduler.rs (new); runtime resource broker; process-supervisor`.
- Implementation/wiring: Use explicit policies over expected improvement, important uncertainty, diversity and cost to choose actual solver/query/branch work. Bind SchedulerNoveltyPolicyId into the budget/scope and report whether union-distance or nearest-candidate distance was used; implement exploration, fairness and cancellation.
- Negative→positive tests: Reject starvation by cheap branches, infinite unknown retries, quota amplification by priority and unauthorized network work.
- Exit: Changing D/G/U/cost changes the next actual process selection while respecting CPU/GPU/child reservations.

#### TLCX-26: Actual world management and semantic duplication

- Depends on: slice-local exits of TLCX-06, TLCX-08, TLCX-25.
- Targets: `world/src/lib.rs refactor; world/src/{arena,identity,coverage}.rs (new)`.
- Implementation/wiring: Replace production reliance on fixed enum products with branches holding actual premises/models/queries/results; manage 256 base slots and 2048 live slots with generations.
- Negative→positive tests: Reject slot-only distinctness, unexecuted logic reported as explored, silent quota padding and unlimited historical accumulation.
- Exit: Report attempted/executed/structurally distinct/semantically unresolved counts separately; isolate legacy 256×8 diagnostics.

#### TLCX-27: Mutual distillation that acquires new structure

- Depends on: slice-local exits of TLCX-20, TLCX-24, TLCX-25, TLCX-26.
- Targets: `tldg/src/distillation.rs; world proposal validators; analysis/src/dgcl_distillation.rs`.
- Implementation/wiring: Decode continuous proposals into typed discrete edits; validate existing membership or use dedicated new-relation validators, then materialize only admitted changes.
- Negative→positive tests: Reject loops accepting only preexisting edges, candidate-as-oracle circularity, digest-only progress and erasing residuals to fake progress.
- Exit: New relations change next-epoch queries and final selection; rejected proposals backflow visibly and stop on NoProgress/budget limits.

#### TLCX-28: Solver results to conditions, Program IR and coding

- Depends on: slice-local exits of TLCX-09, TLCX-11, TLCX-17, TLCX-23, TLCX-27.
- Targets: `analysis/src/{dgcl_projection,completion_plan,dgcl_implementation_closure,validation_producer}.rs`.
- Implementation/wiring: Add ConditionObservation producers bound to exact predicates, contexts, semantics and results; send selected implementations through existing Cargo/connection/acceptance paths.
- Negative→positive tests: Reject conditions discharged by SAT alone, existential-to-universal promotion, Test/Edit grants from scores and mandatory Cargo for mathematical answers.
- Exit: Reasoning tasks close with their own evidence and coding with the existing five kinds; cutting results reopens conditions/closure.

#### TLCX-29: Recommendation, alternatives and final explanation

- Depends on: slice-local exits of TLCX-23, TLCX-28.
- Targets: `runtime/src/decision.rs (new); wire/src/render.rs (new); analysis/src/dgcl_final_output.rs`.
- Implementation/wiring: Build a typed DecisionArtifact with one recommendation, alternatives, context deltas, D/Delta/U/G, optimality scope and dependency-safe partial answers, then render bounded output.
- Negative→positive tests: Reject omitted premise changes, partial conclusions depending on unresolved premises, post-render mutation and raw JSON dumps treated as answers.
- Exit: Trace results through decisions to explanations, measure explanation TL and bind exact final bytes.

#### TLCX-30: Media, RPA and NLP observation integration

- Depends on: slice-local exits of TLCX-09, TLCX-17, TLCX-19, TLCX-28.
- Targets: `media/src/lib.rs; scripts/media_backend.py; analysis/src/{pipeline,language_worker}.rs`.
- Implementation/wiring: Connect source/time/frame/region/tool/model-bound observations as candidate ContextIR premises; use streaming workers, bounded model traversal and registered model IDs.
- Negative→positive tests: Reject acoustic clustering as speaker identity, co-presence as semantic agreement, provenance as truth and catalog entries as implemented methods.
- Exit: Actual existing backend observations feed temporal/spatial TL and hypothesis comparison; uninstalled methods remain Unsupported and cannot satisfy required-profile gates.

#### TLCX-31: Candidate generation through actual repair

- Depends on: slice-local exits of TLCX-07, TLCX-28, TLCX-29.
- Targets: `runtime CandidateProvider; analysis/src/{repair_loop,file_repair}.rs; CLI dgcl_repair.rs`.
- Implementation/wiring: Connect up to four actual candidate→fresh intent/authority→apply→validation→loss/gain reevaluation attempts in one runtime, with separate per-attempt and total deltas.
- Negative→positive tests: Reject duplicate replacement padding, old receipts, unauthorized file application of hypothetical contexts and scope/clock rollback.
- Exit: Exercise real multi-candidate repair and NoProgress/cycle/MayHaveApplied through the same production entry while retaining backups.

#### TLCX-32: Checkpoint, resume and invalidation closure

- Depends on: slice-local exits of TLCX-07, TLCX-31.
- Targets: `runtime/src/resume.rs (new); analysis/src/{closure_resume,evidence_lifecycle}.rs`.
- Implementation/wiring: Persist selection, contexts, solver states, pending effects and four-facet records in binary checkpoints; revalidate source/tool/validator/authority and leases on resume.
- Negative→positive tests: Reject completion-flag restoration, duplicate application after UnknownDelivery, revived expiry from caches and stale solver results under changed contexts.
- Exit: Run actual repair→checkpoint→process exit→same-entry resume→fresh observations→reconstructed recommendation within state budgets.

#### TLCX-33: GPU path driven by semantic observations

- Depends on: slice-local exits of TLCX-05, TLCX-23, TLCX-24, TLCX-26, TLCX-27.
- Targets: `accelerator/src/{lib,gpu}.rs; world numeric adapter; runtime scheduler`.
- Implementation/wiring: Remove fixture-derived production axes and transfer scope-bound observed D/G/cost as SoA; connect arena leases/fences, device reductions and CPU final rechecks.
- Negative→positive tests: Reject unknown sentinels as small loss, ring-neighbor Pareto as global Pareto, summary equality as full-vector proof and partial reuse after device loss.
- Exit: Actual GPU lanes consume new data and preserve selection parity; fallback produces fresh CPU observations, while occupancy/speed require separate measurements.

#### TLCX-34: Normal CLI/facades through one runtime

- Depends on: slice-local exits of TLCX-25, TLCX-26, TLCX-27, TLCX-28, TLCX-29, TLCX-30, TLCX-32, TLCX-33.
- Targets: `cli/src/{args,application,commands,render,io}; scripts/run-*.ps1; skills`.
- Implementation/wiring: Split epistesys-solve/inspect/resume into typed commands; conversation/coding facades call one solve API with task policies. Remove automatic repeated doctor runs from normal operation.
- Negative→positive tests: Reject doctor-only facades, prompt/stdin conflicts, giant default dumps, implicit production builds and duplicate solver runs for one request.
- Exit: A normal user seed reaches actual solvers, four-facet records and recommendations through one canonical path; explicit legacy views preserve old commands.

#### TLCX-35: Final-answer and host-lifecycle closure

- Depends on: slice-local exits of TLCX-29, TLCX-34.
- Targets: `host/src/{lib,stop_hook}.rs; runtime host adapter; hooks`.
- Implementation/wiring: Bind exact rendered bytes through candidate/precommit/postsend/sink/replay; distinguish Stop observations from precommit gates and inspect actual host callback capabilities.
- Negative→positive tests: Reject self-signed Stop logs as sink delivery, mixed seeds, wrong callback order, output replacement and unobserved delivery as success.
- Exit: The slice-local exit is positive/negative source-adapter and registered callback-contract tests. Preflight host/version/issuer/callback/required authority in 00 and retain missing capabilities as HostIntegrationPending. Fresh package→explicit activation→actual-host G13 is required in 43; simulated callbacks in 35 are not real-host evidence.

#### TLCX-36: Schema, source, platform and plugin distribution

- Depends on: slice-local exits of TLCX-04, TLCX-32, TLCX-34, TLCX-35.
- Targets: `schemas; compat; wire; scripts/new-*-package.ps1; plugin manifest; hooks/skills`.
- Implementation/wiring: Build source, executable-platform, plugin and optional runtime/model bundles under separate manifests; validate old full views and new bounded views from fresh packages.
- Negative→positive tests: Reject source-package-as-plugin completion, missing hooks/skills, wrong executables, overwritten old schemas and private paths/weights in distributables.
- Exit: Run solve/inspect/resume from fresh out-of-repo plugin packages without Cargo and verify source/binary/manifest identity.

#### TLCX-37: Resource, cache and long-run regression

- Depends on: slice-local exits of TLCX-06, TLCX-07, TLCX-25, TLCX-32, TLCX-33, TLCX-34, TLCX-36.
- Targets: `runtime/tests/resource_soak.rs (new); worker/media tests; gate scripts`.
- Implementation/wiring: Measure cold/warm operation, 1000-turn bounded soaks, cancellation/crashes and all budget boundaries; share build targets and implement success cleanup/bounded failure retention.
- Negative→positive tests: Reject per-run giant executable copies, unbounded sample vectors, deletion of user caches, lost security state and post-hoc model-traversal limits.
- Exit: Observe RAM/VRAM/disk/file/handle/child bounds and reclamation, keep normal responses within 64KiB and record paired semantic/resource results before/after binary migration.

#### TLCX-38: Adversarial, metamorphic and disconnect tests

- Depends on: slice-local exits of TLCX-11, TLCX-12, TLCX-13, TLCX-14, TLCX-15, TLCX-16, TLCX-18, TLCX-19, TLCX-20, TLCX-21, TLCX-22, TLCX-23, TLCX-24, TLCX-25, TLCX-26, TLCX-27, TLCX-28, TLCX-29, TLCX-30, TLCX-31, TLCX-32, TLCX-33, TLCX-34, TLCX-36, TLCX-37.
- Targets: `fixtures/tlcx; runtime/tests; CI test registry`.
- Implementation/wiring: Inject cut/swap/stale/corrupt/empty/timeout faults into every required edge; test relabeling/order/unit transforms, deliberate defects and reconstructed-context winners with independent oracles. Register v1.4 G31–G37 for `n_geo` endpoint order, irrational q, interval input, square enclosures, novelty-policy ranking reversals, K3 universe transitions and unconnected EmbeddingPolicy.
- Negative→positive tests: Detect implementation-generated gold, vacuous all() assertions, mocked-only positives and unconsumed edges whose removal never affects results.
- Exit: Every cut produces the specified failure or changed result and restoration passes on the same source; retain existing EPR/DGCL regressions.

#### TLCX-39: Calibration, quality and over-hold evaluation

- Depends on: slice-local exits of TLCX-18, TLCX-19, TLCX-22, TLCX-23, TLCX-37, TLCX-38.
- Targets: `tl calibration; analysis/src/dgcl_gold.rs; benchmarks/tlcx (new)`.
- Implementation/wiring: Separate training/calibration/holdout and semantic families; measure silent errors, false holds, rejected valid novelty, risk–coverage and reconstruction gains.
- Negative→positive tests: Reject generalization from paraphrase-only seeds, shared oracles claimed independent, inflated scores by dropping unknowns and tuning to future holdouts.
- Exit: Run evaluation APIs and a small independent formal-oracle corpus. Unprovided natural-language gold stays PendingNoCorpus; engineering gates do not establish general research effects.

#### TLCX-40: Whole-path integration and retired-route isolation

- Depends on: slice-local exits of TLCX-28, TLCX-29, TLCX-30, TLCX-31, TLCX-32, TLCX-33, TLCX-34, TLCX-36, TLCX-38.
- Targets: `runtime integration; all public command registry; legacy adapters`.
- Implementation/wiring: Map every public entry to producers, consumers, evidence, invalidation and cut tests; eliminate unused new modules and isolate stage-only legacy success as history/compatibility.
- Negative→positive tests: Reject doctor-only features, duplicate canonical state, legacy bypasses and report-field-only completion.
- Exit: One source/context/decision traverses actual solving, four-facet evaluation, selection, coding/media consumers, rendering and resume; delete only separately authorized targets.

#### TLCX-41: Independent reaudit and bilingual consistency

- Depends on: slice-local exits of TLCX-36, TLCX-37, TLCX-38, TLCX-39, TLCX-40.
- Targets: `docs; README; RepoSeiri; bounded Assurance review`.
- Implementation/wiring: Reaudit architecture, unsafe boundaries, codecs, authority, caches, solver semantics and finite-formal-v1.4 numeric contracts from a separate perspective. Run RepoSeiri on a fixed digest and Assurance on complete units no larger than 16KiB.
- Negative→positive tests: Reject audit-unknown-as-pass, future designs as implemented claims, JA/EN claim mismatches and rewriting version history.
- Exit: Repair and retest each finding while retaining unresolved hard gates; align equivalent JA/EN README/docs/schema/verification.

#### TLCX-42: All hard gates and release-candidate assessment

- Depends on: slice-local exits of TLCX-36, TLCX-37, TLCX-38, TLCX-39, TLCX-40, TLCX-41.
- Targets: `CI; release gate; Cargo/plugin identity; verification record`.
- Implementation/wiring: Rerun source-side functional, compatibility, resource, actual-backend and package gates on the current snapshot. Select the target version explicitly at implementation start; this slice creates a distribution-candidate identity only. Product release completion requires G13/G15 in 43.
- Negative→positive tests: Reject stale receipts, missing profiles, skipped-as-passed checks, source-version-only completion and unobserved full-vector proofs.
- Exit: Align distribution-candidate source/binary/plugin/schema/docs identity. Pending host evidence means CandidateNotFullyReleased, not a passed G13. Recheck all identities/hard gates in 43 without weakening criteria.

#### TLCX-43: Final wiring verification and handoff

- Depends on: slice-local exits of TLCX-35, TLCX-36, TLCX-37, TLCX-38, TLCX-39, TLCX-40, TLCX-41, TLCX-42.
- Targets: `installed package (when authorized); runtime/host end-to-end; final ledger`.
- Implementation/wiring: Run identical seeds in a fresh session from the distribution candidate and trace solvers, four-facet records, recommendation/alternatives, exact host, replay and resume; install/activate only when authorized.
- Negative→positive tests: Reject self-declared completion records, evidence from other binaries, overall completion with required host gaps and cleanup of unowned caches.
- Exit: Observe Executed/Consumed/Contained/HostBound for every required edge; otherwise report partial completion and reasons. Commit/push remain separate authority.


### Required connection ledger (20 edges)

Register actual producers, consumers, positive observations, cut tests and invalidation for every edge. Adding only a caller or callee cannot satisfy an exit.

| ID | Connection | Preserved contract | Cuts/adversarial cases |
| --- | --- | --- | --- |
| E01 | host/user ingress → SourceStore | source/role/UTF-8 spans | source swap/oversize |
| E02 | SourceStore → DeepGrammar/ContextIR | interpretations and unsupported spans | omission/other-revision AST |
| E03 | ContextIR → ContextGraph | parent/delta/hard invariants | mandatory-constraint removal |
| E04 | ContextGraph → SolverProjection | same question and projection scope | wrong negation/quantification/domain |
| E05 | SolverProjection → supervised process | binary/options/query identity | forged stdout/timeout/injection |
| E06 | SolverResult → model/condition validator | query/result/semantics bindings | SAT-to-entailment promotion |
| E07 | Validated observation → TL four facets | D/Delta/U/G/scope/witnesses | unknown-as-zero/duplicate loss |
| E08 | CandidateProvider → CreativeAssessment | original components and baseline | self-oracle/estimated-as-observed gain |
| E09 | TL + CreativeAssessment → frontier | same goals/units/comparability | mixed problems/unconsumed values |
| E10 | frontier/geometry → scheduler | candidates/transition costs/U | ID-order dependence/starvation |
| E11 | scheduler → resource broker → backend | reservations/deadlines/cancellation | quota amplification/leaks |
| E12 | continuous proposal → discrete validation → next epoch | typed edits and new-relation checks | digest-only progress/circular proof |
| E13 | selection → Program IR / task plan | task-specific evidence | Cargo for all tasks/all-N/A |
| E14 | coding candidate → permit → validation producers | fresh authority/source/tool/binding | solver-created Grants |
| E15 | repair → checkpoint → resume | pending effects/backups/fresh checks | double apply/restored completion |
| E16 | Media/RPA/NLP → candidate premises | fragment/time/method/provenance | method-label-as-truth |
| E17 | observed numeric batch → GPU → CPU final check | row/epoch/scope/fence | fixture values/stale readback |
| E18 | DecisionArtifact → exact renderer | one recommendation/alternatives/deltas/U | lost qualifications in output |
| E19 | render → precommit → postsend → sink/replay | exact bytes/same seed/stage order | Stop-as-delivery |
| E20 | artifact store → cache/journal/security consumers | ownership/lease/retention separation | security eviction/unknown-root deletion |

### 10. Rust sketch contracts

The following sketches specify naming, ownership and state separation, not completed implementation. TLCX-02/03 must implement constructors, canonical codecs and property tests. Compare Ratio values with checked cross multiplication or bounded integer arithmetic; never silently switch overflow to floats. Validate lower<=upper and matching units for Bounded values. Deserializing ArenaKey alone must not construct verified references. Check domain/arena/context on access, retire slots rather than wrapping exhausted generations, and separate persistent LogicalId from process-local ArenaKey. Resume reconstructs fresh handles from LogicalIds.

```rust
// API sketch: private fields are constructed by checked arena/validator APIs.
// This sketch is a design contract, not compiled implementation evidence.
use std::num::NonZeroU64;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct Digest32([u8; 32]);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct ArenaDomainId(Digest32);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct ArenaKey {
    domain: ArenaDomainId,
    slot: u32,
    generation: u32,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct LogicalId(Digest32);
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct DeltaId(ArenaKey);
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct UncertaintyId(ArenaKey);
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct ContextId(ArenaKey);
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct EvidenceId(ArenaKey);
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct MeasurementId(ArenaKey);
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct TransformId(ArenaKey);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Ratio {
    numerator: i64,
    denominator: NonZeroU64,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnknownReason {
    MissingEvaluator,
    UnsupportedFragment,
    Timeout,
    EvidenceMissing,
    Uncalibrated,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Measurement<T> {
    Exact { value: T, evidence: EvidenceId },
    Bounded { lower: T, upper: T, evidence: EvidenceId },
    Estimated { value: T, model_revision: Digest32 },
    Unknown(UnknownReason),
    NotApplicable { basis: EvidenceId },
    Incomparable { scope: Digest32 },
}
#[derive(Clone, Debug)]
pub struct TransformWitness {
    id: TransformId,
    source_context: ContextId,
    target_context: ContextId,
    comparison_scope: Digest32,
    defect: MeasurementId,
    declared_delta: DeltaId,
    uncertainty: UncertaintyId,
    gain: MeasurementId,
}
// No execution permit, trust-root writer or success bool belongs in this type.
```

```rust
// Complete small decoding unit; the frame-level admission is a separate caller.
#[derive(Debug, PartialEq, Eq)]
pub enum DecodeError {
    Truncated,
    OffsetOverflow,
    LimitExceeded,
}

pub struct Decoder<'a> {
    input: &'a [u8],
    offset: usize,
}

impl<'a> Decoder<'a> {
    pub fn new(input: &'a [u8], limit: usize) -> Result<Self, DecodeError> {
        if input.len() > limit {
            return Err(DecodeError::LimitExceeded);
        }
        Ok(Self { input, offset: 0 })
    }

    pub fn take(&mut self, length: usize) -> Result<&'a [u8], DecodeError> {
        let end = self.offset.checked_add(length)
            .ok_or(DecodeError::OffsetOverflow)?;
        let bytes = self.input.get(self.offset..end)
            .ok_or(DecodeError::Truncated)?;
        self.offset = end;
        Ok(bytes)
    }

    pub fn read_u32_le(&mut self) -> Result<u32, DecodeError> {
        let bytes = self.take(4)?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }
}
// A complete codec additionally validates tags, counts, ordering, scopes,
// digests, UTF-8, graph references and trailing bytes before admission.
```

The SolverBackend boundary is `solve(&mut self, query: &CheckedQuery, budget: &RunBudget, cancel: &Cancellation) -> Result<SolverRun, SolverError>`. CheckedQuery binds context/logic/domain/formulas/options. SolverRun separates execution status Completed/Timeout/ResourceExhausted/Unsupported/Failed from task-specific LogicalResult: Satisfiability/Entailment/ModelCheck/Inference/Optimization/Intervention/SpatialRelation. Sat/Unsat belong only to satisfiability; typed payloads retain existential/universal, finite/infinite, brave/cautious and objective bounds. Solvers receive no ReceiptIssuer. Result validators check models/counterexamples/proofs and projection correspondence; a separate host-side observation producer issues receipts.

The normal orchestrator performs (1) source admission, (2) parsing/lowering, (3) original/reconstructed contexts, (4) budget reservation, (5) actual solving, (6) result validation, (7) four-facet/gain records, (8) frontier/scheduling, (9) task consumers, (10) exact output and (11) host/replay. Pass typed handles, not giant JSON as internal glue.

### 11. Final hard gates

| Gate | Required completion observation |
| --- | --- |
| G00 | Complete source/command/consumer ledger, matching snapshots and existing baseline |
| G01 | Single source/arenas, old canonical preimages, binary golden/negative/fuzz tests |
| G02 | First-class reconstruction without hard-constraint/authority amplification |
| G03 | Actual SMT/ASP and declared finite-logic positives/counterexamples/Unknown; independent model-result checks |
| G04 | D/Delta/U/G observations causally affect downstream selection and further search |
| G05 | Nonempty S_Creative positives, reconstructed-context winners, incomparability and optimality scope |
| G06 | Non-Euclidean geometry/baseline, actual scheduling and state/query changes from distillation |
| G07 | CPU/GPU input/mask/result/selection correspondence without old fixture axes |
| G08 | Task-specific completion, actual repair→checkpoint→fresh resume and condition producers |
| G09 | Applicable source/time/region-bound Media/RPA/NLP connections |
| G10 | Resource/long-run/cache/replay/cleanup/build-scratch limits and fault tests |
| G11 | Legacy JSON/new binary/strict consumers/source/platform/plugin packages |
| G12 | Normal facade→one runtime→recommendation plus alternatives→exact rendering |
| G13 | Actual required-host precommit/postsend/sink/durable-replay observations |
| G14 | Cut/swap/stale tests for all 20 edges, reaudit under different conditions and JA/EN consistency |
| G15 | All gates rerun under one release identity and handoff after required install/host pickup |

Where G13 is required, missing host capabilities cannot count as success. Record source-only and overall completion separately; the overall roadmap remains incomplete. Unprovided independent natural-language gold and unproven general performance may remain research states, but missing solver connections, measurements, candidate consumption or binary migration cannot be moved into research-pending to claim completion.

Acceptance fixtures must include preservation, deliberate defects, admissible premise reconstruction, prohibited constraint weakening, cross-logic disagreement, shared bad parsers, natural-language ambiguity, valid novel candidates, invalid unusual candidates, simplification, stale evidence, counterexamples, incomparability, codec corruption, empty caches and crash/restart. When contrasting original problems and alternative axiom systems, explicitly declare domains/operations/symbol mappings rather than silently interpreting a string such as “2+2” in another algebra.

### 12. Audit, references and work boundary

The initial RepoSeiri 1.1 repository scan hit its 100,000-entry limit; its incomplete result was not used as successful coverage. A subsequent cache-free fixed-commit export was inspected with subtree scope, observing 397 entries, 67 documents and zero limit-exceeded scopes. Its source-session digest is `sha256:d440e908799e39bbfbdb466c5133fb3f56ce9422cd468c44024d02c19c16ad65`. This describes baseline document structure, not proof that the new roadmap is implemented.

After one Assurance-Compiler doctor call, complete Rust units no larger than 16KiB received supplementary review. `effect_error.rs`, supervisor `filesystem.rs` and `media_snapshot.rs` returned blocked_with_diagnostics; `host/stop_hook.rs` returned needs_evidence; all retained missing_or_unknown. Do not treat unreviewed/oversized modules as safe or split them to evade scope limits.

Design references: [Z3 multiple objectives](https://microsoft.github.io/z3guide/docs/optimization/combiningobjectives/), [Potassco/clingo](https://potassco.org/guide/), [Spot](https://gitlab.lre.epita.fr/spot/spot), [MAP-Elites](https://arxiv.org/abs/1504.04909), [Conformal Risk Control](https://arxiv.org/abs/2208.02814). Their existence does not establish Epistesys effects or implemented support; check statistical assumptions before calibration.

Freezing this document performs research, design and document additions only. It does not implement source changes, bump versions, commit/push/merge, delete existing caches, update plugins or restart hosts. A later implementation instruction should proceed from TLCX-00 in dependency order and close functional, compatibility, resource and host gates on the current snapshot.
