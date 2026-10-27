# EPR 敵対的監査B / EPR adversarial audit B

## 日本語

### 対象・独立性・状態

日付：2026-10-27 JST。基点：`75500ed7a33a009f5e1ce813862dc0a7986372c9`に対するEPR作業ツリー。以下は`6.3.2-alpha.2`と表示される昇格前sourceの観測であり、alpha.3 identityの最終gateではない。

このパスは、runtime/packaging担当が、それらとは別担当の文法、Program IR接続、connection producer、CLI作用報告、TLDG対応、数値入場契約を点検した**チーム内の別担当レビュー**である。共有された設計・文脈を持ち、独立第三者の監査・独立評価・repository-wide safety・merge approvalではない。B02追補の予算修正は、発見・親への報告後に本担当が実装したため、その後の確認は自己検証も含む。最終の別担当統合gateを省略する根拠にはしない。

最初はread-onlyで調べ、具体的な反例とsource hashを報告した。その後B01はobserver担当、B02は親担当と本担当が分けて修正した。下記の既知反例は対応する限定profileで再確認したが、未知欠陥ゼロとはしない。

### B01：標準assertionの名前解決をCargoが差し替えられた

**修正前の実反例：** root packageに `std = { package = "audit-std", path = "shadow_std" }` を宣言し、その依存が標準ライブラリを再exportしながらno-opの`assert_eq!`をexportする。driverは許可された2 item・literal argument・direct API call・`::std::assert_eq!(observed, Some(10))`という同じ形に保つ。実APIが`Some(11)`を返しても、plain Cargoはexact named testを1件成功として報告した。

別の対照fixtureでは同じAPIとdriverから`std` aliasだけを除き、実際のassertionが`Some(11) != Some(10)`として失敗することを確認した。したがって、単なる表示上の名前や入力値の誤認ではない。実行対象は所有するfixtureのみで、実権限の偽造や外部システム攻撃は行っていない。

修正前hash：`production_connection.rs`=`aed2571fc31245543b56ae5612544abef6d40973309fdfd767d01db38b780dff`。

**修正：** metadataを`--no-deps`ではなくofflineのfull resolve graphとして取得する。package target名、依存name/rename、resolved dependency名の`std` shadowを保守的に拒否し、null/欠落/未解決graphも受理しない。lib/testの実target path、登録済みdriver/observer/oracle metadata、exact test結果の照合は維持する。

**再確認：** `epr_17_std_shadow_can_pass_plain_cargo_but_never_mints_connection_evidence`が成功した。ここでの成功は「plain Cargoでは成功する悪いfixtureを、connection observerは証拠化しない」というnegative gateの成功である。正規driverの既存positiveを常時拒否へ置き換えたわけではない。

この修正はbounded driver profileにおけるassertion解決条件を強化する。登録されたoracleの意味的妥当性、任意build script/proc macro、ambient Cargo config、完全なhermetic build worldの正しさを証明しない。異なるreceipt/run/keyを独立したoracleの証明にしない。

### B02：文法の出力予算を適用する前に後処理を実行していた

**修正前の実反例：** `max_tokens=2`で`編し、`を7,000回繰り返し`確認してください`を付けた63,024-byte入力は、source byte上限内でも2秒の外部期限まで返らず、所有processを停止・回収した。2,000節でも約782 msかかり、過剰なactionの分割・構築の後に初めて`PostprocessBudgetExceeded`になっていた。

修正前hash：`instruction_grammar.rs`=`379a4ba6ce3976221169ac57f37eef5808be7d9b4e47354c9e6d49de1445f36f`。

親担当による第一修正は、splitterの早期action上限、full-body quote cache、重複quote区間のmergeと二分探索である。追補点検では、scope/reference/condition/graphが別々の後処理を先に構築する点も残っていた。日本語のopaque body内の`file x`反復やboolean termは、English routeのtoken制限だけでは十分に制約されない。

**追補のfail-first：** 小さいwork ceiling32のscope反復、20 boolean term、協調action graphの3試験は旧実装ではParsedとなり、新しい共有work契約のnegativeとして失敗した。通常の3節入力のpositiveは最初から成功した。

**修正：** privateな`PostprocessBudget`をparse全体で共有し、action/coordination、scope word scanと出力、reference candidate index、condition node/boolean split、constraint node/edge/conflict処理へ渡す。push・clone・明示loop stepの前にchargeし、枯渇は型付きerrorとして返す。action上限はtoken/chart/残余予算の小さい値に制約する。

- scope/referenceのword走査をborrowed iteratorにし、全wordのString vectorを先に作らない。
- action targetの正規化を一度だけ行い、同じsurfaceの反対polarityをgroup化する。source index順にconflict edgeを戻し、既存の順序意味を保つ。
- conditionのparenthesis depthをcacheし、markerごとに長いprefixを再走査しない。
- 条件のtruthはUnknownのまま。depth16によるUnresolved、引用、否定、作用域、source spanの意味を変更しない。

このcounterは実装上のrecord/明示stepの予算であり、TranslationLoss、意味の厳密さ、実CPU命令数、wall-clock秒数の測定値ではない。lexical byte走査は別のsource byte上限で制約される。物理的なhard deadlineや形式的停止証明へ読み替えない。

**再確認：** 同じ7,000節の独立probeは約11 msで`PostprocessBudgetExceeded`を返した。8,000 scope/chart32は約47 ms、500 action/chart32は約1 msで明示errorとなった。これらは1回のlocal debug観測であり、benchmark、速度改善率、一般latency保証ではない。

`epr_postprocess_budget`は5/5成功。source→Program IR→completionの追加確認では`BudgetRejected`、2 blocker、closure=`Clarify`、authority_created=falseを観測し、budget不足を成功や無害なNoActionへ潰していない。既存grammar/constraints/matrix/progress/pipelineの31試験も成功し、通常のmodal/forbidden polarity、条件truth、曖昧性、source spanを維持した。

### 他の重点確認

| 対象 | 確認と結論 | 境界 |
| --- | --- | --- |
| Program IR / TL / closure | projection6件、implementation closure8件の既存test成功。source/条件/scope脱落、公開flag改変、証拠弱化、失効を含む | 抽出済み要求と登録profileの検査。原文意味の完全抽出証明ではない |
| Typed CLI error | `Unknown`の既定、作用後のtyped context、future/missing stateの扱いを静的確認。実write後のhead保存障害test1件、現sourceの作用報告unit test3件成功 | errorはrollbackではない。stdout失敗を「変更なし」と表示しない |
| TLDG token binding | quote・Rust fence・日本語を混ぜた別fixtureで正規mapping/kernel成功。token bindingをroot IDへ差し替えると拒否 | source構造とnode対応の確認であり、semantic truthや実行権限ではない |
| Numeric admission | 非単調CSRを拒否。単一変数QUBOの独立energy flip oracleはself termを含み`14/-14`を確認 | CPUの小型oracle。全数値空間の形式証明ではない |
| Numeric work shape | 8,192 variables/262,144 zero-active termsの有効inputをCPUで観測し、現在のlinear accumulationを確認 | debug一例。GPU speedupや全vector parityの証明ではない |
| CPU/GPU source delta | CSR admission、checked CPU算術、同じGPU i32安全域、empty logical buffer、self term handlingを点検 | このBパスは実GPUを追加実行していない。device観測は担当の別記録に依存 |
| Packaging helper接続 | `test-epr-real-media.ps1`が明示source-package allowlistに含まれることを確認 | fresh package/alpha.3/remote CIの最終確認は別gate |

独立probeの最初のQUBO assertionはsummaryのschema/count wordをminimumと取り違えて失敗した。layoutを確認してminimum/maximum wordへ訂正すると成功した。これは検査コードの誤りであり、製品数値バグとして採用していない。

### source pin

以下は本Bパスの再確認時の作業ツリーhashである。version昇格や後続source差分へそのまま保証を移さない。

| repository-relative path | SHA-256 |
| --- | --- |
| scripts/lc631/crates/lc631-analysis/src/instruction_grammar.rs | ab8fe779953f8994ae0258c6e41eeab4db6dcfc7d48fcf5db2f5c191d03a2935 |
| scripts/lc631/crates/lc631-analysis/src/production_connection.rs | ba70e00e4e008ca1525e4eabda7ad5eb01b2369b20577d48145d3ec2573c78f7 |
| scripts/lc631/crates/lc631-analysis/src/validation_producer.rs | 2a0bb3c879c2a4bc6d72ffc01fd672eeca884bbf42978bc00850d4672d990c9b |
| scripts/lc631/crates/lc631-cli/src/effect_error.rs | 8383e1738208aeda351fc6508f1eab776cc108694da9f710156826d8db92d2e6 |
| scripts/lc631/crates/lc631-cli/src/dgcl_repair.rs | 5c7e672dd11df9d54076e232239726754461ba1953c144e47c2ae44789d9e78c |
| scripts/lc631/crates/lc631-tldg/src/grammar.rs | 55268aa56bd2befe33ef960aff6b40e251b004d96408a69db6a0a370bf524a5b |
| scripts/lc631/crates/lc631-tldg/src/parser.rs | 2202e647c8becce5a7f64bb7af24d800c5ed5b0eb8f7f2beb03a2bd7fcf5fc2a |
| scripts/lc631/crates/lc631-accelerator/src/lib.rs | 3aa821df7af45f6d9613783786e82c1a5e23d96e1285d2324f4a84787633368c |
| scripts/lc631/crates/lc631-accelerator/src/gpu.rs | 6ed0ff649ac8d0478dd605a6ac09d16d73435f9b365315d21134ca0717193363 |

### 残る非主張

この記録は、一般自然言語精度、独立semantic gold、任意コードの安全性、OS-wide sandbox、完全build-input閉包、host callback/delivery、即時revocation、研究効果、merge許可を証明しない。ローカルprobeの依存解決は隔離したlockによるものも含むため、製品workspaceの全release gateとは区別する。controllerのprocess deadlineを、任意filesystem操作の割込み可能なhard wall-time保証に拡張しない。

Assuranceの選択単位・`blocked_with_diagnostics`等は[補助監査記録](epr-assurance-observations-2026-10-27.json)に分離する。静的surfaceを本パスのruntime観測や第三者の保証へ昇格させない。B01/B02の報告反例は修正・再確認したが、最終source freeze後の全必須gateとalpha.3 identity再検証は親の統合手順で必要である。

---

## English

### Scope, independence, and status

Date: 2026-10-27 JST. Baseline: the EPR worktree over `75500ed7a33a009f5e1ce813862dc0a7986372c9`. These observations used pre-promotion source reporting `6.3.2-alpha.2`; they are not final alpha.3-identity gates.

The runtime/packaging contributor inspected grammar, Program IR connections, connection producers, CLI effects, TLDG correspondence, and numeric admission implemented by other contributors. This is a **different-contributor team review**, sharing design/context—not an independent third-party audit, independent evaluation, repository-wide safety, or merge approval. After discovery/reporting, this contributor implemented B02's additional budget correction, so its follow-up includes self-verification and cannot replace separate final integration gates.

Initial inspection was read-only, with concrete counterexamples and source hashes reported before edits. B01 was repaired by the observer owner; B02 by the parent and this contributor. The reported counterexamples were rechecked in their bounded profiles; no zero-unknown-defects claim is made.

### B01: Cargo could change the standard assertion's resolution

**Observed pre-fix counterexample:** the root declared `std = { package = "audit-std", path = "shadow_std" }`. That dependency reexported the standard library and a no-op `assert_eq!`. The allowed two-item driver retained literal arguments, a direct API call, and `::std::assert_eq!(observed, Some(10))`. Plain Cargo reported one exact named test passed even when the API returned `Some(11)`.

A separate control retained the same API/driver without the alias and failed on `Some(11) != Some(10)`. This distinguishes a real resolution effect from a display/name/input misunderstanding. Only owned fixtures were executed; no real authority forgery or external attack was performed.

Pre-fix `production_connection.rs` hash: `aed2571fc31245543b56ae5612544abef6d40973309fdfd767d01db38b780dff`.

**Correction:** collect the full offline resolve graph instead of `--no-deps`. Conservatively reject `std` target/dependency/rename/resolved-dependency names and missing/null/unresolved graphs. Retain actual lib/test target paths, registered driver/observer/oracle bindings, and exact test-result checks.

**Recheck:** `epr_17_std_shadow_can_pass_plain_cargo_but_never_mints_connection_evidence` passed. Passing means the observer refuses to issue connection evidence for a bad fixture that plain Cargo accepts. Legitimate-driver positives were not replaced with blanket refusal.

This strengthens assertion resolution for the bounded driver profile. It does not prove oracle semantics, arbitrary build scripts/proc macros, ambient Cargo configuration, or complete hermetic builds. Different receipts/runs/keys do not establish independent oracles.

### B02: Postprocessing ran before output-budget enforcement

**Observed pre-fix counterexample:** with `max_tokens=2`, 7,000 repetitions of `編し、` followed by `確認してください` produced 63,024 source bytes within the input limit but did not return within the external two-second deadline; the owned process was terminated/reaped. Even 2,000 clauses took about 782 ms before `PostprocessBudgetExceeded`, after excessive splitting/materialization.

Pre-fix `instruction_grammar.rs` hash: `379a4ba6ce3976221169ac57f37eef5808be7d9b4e47354c9e6d49de1445f36f`.

The parent's first correction added early splitter limits, full-body quote caching, merged intervals, and binary overlap lookup. Follow-up found separately materialized scopes/references/conditions/graphs. Japanese opaque bodies containing repeated `file x` or boolean terms are not sufficiently limited by English-route token admission.

**Additional fail-first observations:** three negative tests for scope repetition, 20 boolean terms, and coordinated graph work under a ceiling of 32 were Parsed by the old implementation and failed the shared-work contract. The ordinary three-clause positive passed initially.

**Correction:** one private `PostprocessBudget` spans actions/coordination, scope-word scans/materialization, reference indices, condition nodes/boolean splitting, and constraint nodes/edges/conflicts. Charge before pushes/clones/explicit steps; exhaustion returns a typed error. Action admission uses the minimum token/chart/remaining ceiling.

- Borrowed word iterators avoid eagerly creating all word Strings for scopes/references.
- Normalize action targets once, group opposite polarities by surface, and restore conflict edges in source-index order.
- Cache parenthesis depth instead of rescanning long prefixes for each condition marker.
- Preserve Unknown condition truth, depth-16 Unresolved behavior, quoting, negation, scopes, and exact spans.

The counter measures implementation records/explicit steps, not TranslationLoss, semantic rigor, literal CPU instruction count, or wall-clock seconds. Lexical byte scans have a separate source-byte ceiling. It is not a physical hard deadline or formal termination proof.

**Recheck:** the same independent 7,000-clause probe returned typed exhaustion in about 11 ms. The 8,000-scope/chart32 and 500-action/chart32 probes returned explicit errors in about 47 ms and 1 ms. These are single local debug observations, not benchmarks, speedup percentages, or general latency guarantees.

`epr_postprocess_budget` passed 5/5. The source→Program IR→completion follow-up observed `BudgetRejected`, two blockers, closure=`Clarify`, authority_created=false—not success or harmless NoAction. The 31 existing grammar/constraints/matrix/progress/pipeline tests passed, preserving normal modalities, forbidden polarity, condition truth, ambiguity, and spans.

### Other focused checks

| Area | Observation/conclusion | Boundary |
| --- | --- | --- |
| Program IR/TL/closure | Existing projection6 and implementation-closure8 tests passed, including missing conditions/scopes/source, flag tampering, weakening, expiry | Extracted requirements/registered profiles, not exhaustive semantic extraction |
| Typed CLI errors | Inspected Unknown defaults, post-effect contexts, future/missing states; one real post-write head-persistence failure test passed; three current-source effect-error unit tests passed | Errors do not roll back effects or justify reporting no mutation |
| TLDG token bindings | A distinct mixed quote/Rust-fence/Japanese fixture passed mapping/kernel checks; substituting the root ID for a token binding failed | Structural correspondence, not semantic truth or authority |
| Numeric admission | Rejected nonmonotone CSR; independent one-variable energy-flip oracle confirmed self-term deltas `14/-14` | Small CPU oracle, not proof over all numeric inputs |
| Numeric work shape | Observed valid 8,192-variable/262,144 zero-active-term CPU input and current linear accumulation | One debug case, not GPU speedup or full-vector parity |
| CPU/GPU source changes | Inspected CSR admission, checked CPU arithmetic, shared GPU i32 safety bounds, empty buffers, self terms | No new GPU execution in this B pass; device observations belong to separate owner records |
| Packaging connection | Confirmed `test-epr-real-media.ps1` is explicitly allowlisted | Fresh-package/alpha.3/remote-CI final checks remain separate |

The first independent QUBO assertion mistakenly read schema/count words as minima. After checking the layout and correcting min/max indices, it passed. This was a probe bug, not a product numeric finding.

### Source pins

These worktree hashes were rechecked in pass B; do not transfer their evidence unchanged to promotion or later source changes.

| Repository-relative path | SHA-256 |
| --- | --- |
| scripts/lc631/crates/lc631-analysis/src/instruction_grammar.rs | ab8fe779953f8994ae0258c6e41eeab4db6dcfc7d48fcf5db2f5c191d03a2935 |
| scripts/lc631/crates/lc631-analysis/src/production_connection.rs | ba70e00e4e008ca1525e4eabda7ad5eb01b2369b20577d48145d3ec2573c78f7 |
| scripts/lc631/crates/lc631-analysis/src/validation_producer.rs | 2a0bb3c879c2a4bc6d72ffc01fd672eeca884bbf42978bc00850d4672d990c9b |
| scripts/lc631/crates/lc631-cli/src/effect_error.rs | 8383e1738208aeda351fc6508f1eab776cc108694da9f710156826d8db92d2e6 |
| scripts/lc631/crates/lc631-cli/src/dgcl_repair.rs | 5c7e672dd11df9d54076e232239726754461ba1953c144e47c2ae44789d9e78c |
| scripts/lc631/crates/lc631-tldg/src/grammar.rs | 55268aa56bd2befe33ef960aff6b40e251b004d96408a69db6a0a370bf524a5b |
| scripts/lc631/crates/lc631-tldg/src/parser.rs | 2202e647c8becce5a7f64bb7af24d800c5ed5b0eb8f7f2beb03a2bd7fcf5fc2a |
| scripts/lc631/crates/lc631-accelerator/src/lib.rs | 3aa821df7af45f6d9613783786e82c1a5e23d96e1285d2324f4a84787633368c |
| scripts/lc631/crates/lc631-accelerator/src/gpu.rs | 6ed0ff649ac8d0478dd605a6ac09d16d73435f9b365315d21134ca0717193363 |

### Remaining non-claims

This record proves neither general natural-language accuracy, independent semantic gold, arbitrary-code safety, OS-wide sandboxing, full build-input closure, host callback/delivery, immediate revocation, research effects, nor merge permission. Some isolated probes resolved dependencies with their own lock; these are not the product's complete locked release gates. Do not extend controller process deadlines into interruptible hard wall-time guarantees for arbitrary filesystem operations.

Assurance selections and `blocked_with_diagnostics` outcomes remain in the [advisory audit record](epr-assurance-observations-2026-10-27.json). Static surfaces do not become this pass's runtime evidence or third-party assurance. Reported B01/B02 counterexamples were repaired/rechecked; all required gates after final source freeze and alpha.3-identity revalidation remain the parent's integration responsibility.
