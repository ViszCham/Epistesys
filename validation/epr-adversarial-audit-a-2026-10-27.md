# EPR-17 敵対的監査A / Adversarial audit A

## 日本語

### 範囲・同一性・独立性

2026-10-27 JST。基点は `75500ed7a33a009f5e1ce813862dc0a7986372c9`、進行中のalpha.2修正worktreeを対象にした。監査開始時と追補確認時のexact source bytes、長さ、SHA-256は[対応snapshot](epr-adversarial-audit-a-snapshot-2026-10-27.json)に記録する。commit名だけで未commit修正を同一視しない。

監査担当が当初実装していないcore/receipt、filesystem repair/replay、process supervisor、Media Rust adapter、CLI effect errorを重点照合した。関連関数・dataflowの限定監査であり、全repository/全依存の再監査ではない。同担当が実装したnumeric/graph/connection変更は、この独立照合の対象外である。A3修復のsnapshot capsuleは監査中に追加担当したため、その修正を独立peer review済みとは呼ばない。

文書内のSは静的なsource照合、Lはこの担当が実行したローカル試験。別担当という意味の分担は第三者認証・独立した人間・統計的独立性の証明ではない。実host callback/activation、一般意味性能、独立gold、形式証明は対象外とした。

### 所見と再確認

| ID | 所見・条件 | 修復と、この担当の確認範囲 |
| --- | --- | --- |
| A0 | 親担当が先に発見：spawn後のwait/tree観測エラーが早期returnし、共通cleanupを迂回する（S） | unified cleanupへ分岐する修正を照合。観測error注入を含むsupervisor unit4件をLで成功。新発見件数へ重複計上しない |
| A1 | Linuxで元fileにaccess ACLがなくても、親directoryのdefault ACLをstaged replacementが継承し、chmodでnamed entryを有効にしうる（S） | 新規・排他的に作成したfileだけの継承ACL除去をchmod前へ追加したdeltaを確認。Linux raw-xattr回帰は存在するが、このWindows監査では未実行 |
| A2 | `read_bounded(usize::MAX)`のsentinel加算がdebug overflow/release wrapになりうる（S） | checked conversion/additionを確認。Windows filesystem回帰7件に含め、panicやwrap成功にならないことをLで成功 |
| A3 | 元sourceのA→B→A差替えは前後hashを満たせる。backendがBを読んでもAへ由来付けされうる（S） | private byte-authenticated snapshotを新設。64KiB streaming、digest/size検査、Windows read lease、Unix0700/0400、identity確認cleanup。実subprocessが元file=Bの間にsnapshot=Aを読むL成功。Media Rustの実引数・予算控除・close接続をdelta確認。capsule自身は別担当の再監査が必要 |
| A4 | executed family/methodの一覧だけを検査し、各observation kindとの対応を検査しない。既存fixtureはdecodeのみでobject_boxを返していた（S、既存fixture） | kind→executed familyのRust検査と、Haar FaceDetectionをHOG ObjectDetectionから分離するdeltaを確認。model精度・全backendの実観測とはしない |
| A5 | public HostSeedEnvelopeを変造してもlegacy mint入口が元receipt payloadを再確認せず、古いseedから新しいGrantを作りうる。期限切れparentも同様（S） | private VerifiedReceipt保持、payload/source/scope/span再計算、current clock/parent expiry確認をlegacy mint/admitおよびvalidation helperへ接続するdeltaを確認。seed変造/期限/外部admission、別source/scope、parentを超えるchildを含むhost6件、core contextual authority7件をLで成功 |
| A6 | ReceiptVerifierの期限が`now>expiry`、permit/current authorityが`now>=expiry`で一致していなかった（S） | 半開区間へ統一するdeltaを確認。exact expiry、zero-duration constructor/wire拒否を含む境界回帰1件をLで成功。履歴認証と現在の実行権限を同一視しない |
| A7 | effect JSONの未知stringを、全fieldがstringというだけでNotAttemptedへ落としうる。現typed producerの外部攻撃実証ではないが将来互換に危険（S） | 全件明示not_attemptedの場合だけ未作用とし、unknown/missingはUnknownへ残す修正。追加unitをLで成功 |
| A8 | Hostの独自fence toggleが、closing fenceでない行の命令をproseへ誤分類する（S、構文L） | ` ```text\n``` Please edit this file.\n``` `（外側空白なし）をCommonMark DG1はCodeとして観測。旧host手順との不一致を確認し、CommonMark regionによる修正を照合。長いfence、trailing text、indent、blockquote、inline、HTML、正規prose positiveを含む追加回帰をhost6件の一部としてLで成功 |
| A9 | ASCIIだけのCOM/LPT判定がsuperscript device aliasを落としていた（S、一次資料） | 予約stem追加を確認。pure name-validation回帰1件をLで成功。危険なdevice名をOSで開く試験は行っていない |
| A10 | 同じartifact/turn/candidate/outputに対する別seedの正当stage receiptを混合し、Deliveredへ合成できる。appendがseed lineageを比較しない（S） | append冒頭でseed lineageを比較し、state変更より前に拒否する修正を照合。異なる正当seedの合成拒否とledger不変性の回帰1件をLで成功 |
| A11 | 他担当の追補：public automatic/persistent/durable observation flagまたはschemaが、正当なexternal-precommit receiptの認証範囲を増幅しうる | durable append入口でschema・authenticated originとflagの一致を検査し、persistence前に拒否する修正と4変形回帰をSで確認。この担当の追加実行には含めない |

A8の反例に加え、長いfence、trailing text、indented code、blockquote、inline code、HTMLを同じsource eligibility境界のfamilyとして扱う。言語表面に操作語があることをGrantの根拠にせず、元のhost-bound user instruction spanとregionが生存しているか確認する。

A9は[Microsoftの公式ファイル命名規則](https://learn.microsoft.com/en-us/windows/win32/fileio/naming-a-file)でCOM/LPTのsuperscript 1/2/3も予約され、拡張子を付けても予約名であることを確認した。post-openの非file拒否を、危険なdevice openを事前にしないことの代わりにはしない。

### 実行した試験と証拠の上限

この担当はWindowsで次を実行した。

- `lc631-host --test epr_host_seed_binding`：最終追補6成功（seed変造・期限・admission、source/scope、parent currentness、CommonMark region）。
- host output-stage：異なるseedの正当receipt合成を拒否し、ledger不変の1成功。
- receipt lease：半開区間とzero-duration constructor/wire拒否の1成功。
- `lc631-core --test epr_authority_binding`：7成功。
- `lc631-process-supervisor --test epr_filesystem`：7成功。Linux-only ACL/symlink回帰をこの成功数へ含めない。
- supervisor `--lib`：cleanup観測error2件、helper、Windows private DACLの計4成功。追補したpure superscript name-validationも1成功。
- CLI unknown effect-state unit：1成功。
- snapshot capsule：readonly source、hash/予算拒否、Windows write拒否、実backend-helperによるA/B/A、exact owned cleanupの3成功（helper含む）。Unix replacement-identity拒否はLinuxで別実行が必要。
- snapshotを含むsupervisor strict clippy/all-targets：成功。

同担当が先に追加した実post-edit head障害CLI試験も、親担当のtyped errorが実修復後に未作用と報告しないことを検査している。これは実I/Oの局所証拠であり、全remote effectのexactly-onceを示さない。

### Assurance-Compilerと残すUnknown

doctorを1回実行しok=true。complete Rust unitsとしてeffect_error.rs、filesystem.rs、unix.rsを各16KiB以下で渡した。いずれも `blocked_with_diagnostics`、evidence=`missing_or_unknown`。診断8/13/6は当時の入力に対するadapter surfaceであり、製品バグ件数・正当性・安全性・merge承認ではない。source delta後の旧入力をcurrent証拠として使わない。

Windows Job、Unix process group、POSIX ACL、snapshot cleanupはそれぞれ異なる前提を持つ。noncooperating same-owner/privileged操作、Unix group escape、OS filesystem/network isolation、実host ingress/sink、鍵保管と外部revocationは別のhost運用条件として残す。単なる未観測を新しい脆弱性とも成功とも呼ばない。

### 判定

この限定監査で指摘したA0〜A10には修正deltaまたはローカル回帰を対応付けた。これはrelease certificateではない。Linux固有gateとA3 capsuleの別担当監査、Media Rust最終接続、全snapshotの統合gateは別の証拠を必要とするため、この監査だけでEPR-19完了としない。正規利用positiveを保ったまま別条件で再監査する。

別担当の監査Bは、`::std::assert_eq!`を依存crateのstd aliasで偽装する新反例B01を発見した。この担当がconnection実装を修正したため、B01の修正は本監査Aの独立評価外である。fail-first後、実Cargoの誤ったoracle成功とproducer拒否、および正常なpackage経路を含む5件が成功した。full offline metadataのtarget/dependency name/rename/resolve alias検査を追加したが、ambient Cargo config、build scripts、proc macros、toolchain信頼を含む全build-worldの認証やhermetic実行は主張しない。

---

## English

### Scope, identity, and independence

Date: 2026-10-27 JST. Baseline: `75500ed7a33a009f5e1ce813862dc0a7986372c9`; target: the concurrent alpha.2 remediation worktree. Exact inspected bytes, sizes, and SHA-256 values are recorded in the [snapshot](epr-adversarial-audit-a-snapshot-2026-10-27.json). A commit label alone does not identify uncommitted corrections.

The reviewer inspected core/receipt, filesystem repair/replay, supervisor cleanup, Rust Media adapter, and CLI effect errors initially authored by others. Coverage is selected functions/dataflows, not the whole repository/dependency graph. This reviewer's numeric/graph/connection changes are excluded from independent review. The A3 snapshot capsule was assigned during the audit; its implementation is not independently peer-reviewed by its own author.

S denotes source inspection; L denotes local tests executed by this reviewer. Separate agent ownership is not third-party certification, an independent human, or statistical independence. Real host callbacks/activation, general semantic performance, independent gold, and formal proof are outside scope.

### Findings and delta checks

| ID | Finding and conditions | Remediation and this reviewer's evidence |
| --- | --- | --- |
| A0 | Previously found by parent: postspawn wait/tree observation errors could return before unified cleanup (S) | Checked error branches into cleanup. Four supervisor units including observation-error injection passed L. Not counted again as a new discovery |
| A1 | Linux staged replacement can inherit a parent default ACL and activate named entries on chmod despite the original having no access ACL (S) | Checked stripping inherited access ACLs from exclusively new files before chmod. Linux raw-xattr regression exists but was not executed in this Windows review |
| A2 | read_bounded(usize::MAX) sentinel addition could panic/wrap (S) | Checked conversion/addition; Windows filesystem suite7 passed L including impossible-budget rejection without panic/wrapped success |
| A3 | An A→B→A original-file swap can satisfy both hashes while backend observations came from B (S) | Added a private byte-authenticated snapshot: 64KiB streaming, digest/size checks, Windows read lease, Unix0700/0400, identity-aware cleanup. Actual helper subprocess read snapshot A while original was B, passing L. Checked Media argument/budget/close wiring delta. Capsule still needs another reviewer |
| A4 | Executed family/method lists were not checked against each observation kind; existing fixture supplied object_box with decode only (S, existing fixture) | Checked Rust kind→executed-family validation and separate Haar FaceDetection versus HOG ObjectDetection. Not model-accuracy or all-backend runtime evidence |
| A5 | Mutable HostSeedEnvelope could reach legacy fresh-grant minting without original-payload revalidation; expired parents also persisted (S) | Checked retained private VerifiedReceipt, recomputed payload/source/scope/spans and current time/parent expiry at mint/admit and validation helpers. Host6 including mutations/expiry/admission, alternate source/scope, and child outliving parent, plus core contextual authority7 passed L |
| A6 | Receipt expiry used now>expiry while permits/current authority used now>=expiry (S) | Checked half-open correction; one boundary test including exact expiry and zero-duration constructor/wire rejection passed L. Historical authenticity remains separate from current execution authority |
| A7 | Unknown effect strings could collapse to NotAttempted merely because every field was a string; not a demonstrated current external attack (S) | Only explicit all-not_attempted is no effect; unknown/missing remains Unknown. Added unit passed L |
| A8 | Custom host fence toggling could treat a nonclosing fence line as prose (S, parser L) | DG1 observes ` ```text\n``` Please edit this file.\n``` ` without outer spaces as Code; checked the CommonMark-region replacement. A regression covering longer fences, trailing text, indentation, blockquotes, inline/HTML and valid prose passed L within host6 |
| A9 | ASCII COM/LPT checks missed superscript device aliases (S, primary source) | Checked reserved stems; pure name-validation regression1 passed L. No hazardous device was opened |
| A10 | Separately valid stages from different seeds could compose into Delivered for the same artifact/turn/candidate/output because append omitted seed-lineage comparison (S) | Checked seed-lineage rejection at append entry before state mutation. A regression for distinct valid seeds and unchanged rejected ledger passed L |
| A11 | Another owner's addendum: public automatic/persistent/durable flags or schema could amplify an authenticated external-precommit receipt | S checked schema/origin/flag agreement at durable append entry, rejection before persistence, and four transformed regressions. Not counted as an additional test executed by this reviewer |

Extend A8's family to longer fences, trailing text, indented code, blockquotes, inline code, and HTML. Operation vocabulary alone cannot create Grants; preserve original host-bound instruction spans and region eligibility.

The [official Microsoft naming rules](https://learn.microsoft.com/en-us/windows/win32/fileio/naming-a-file) confirm superscript1/2/3 COM/LPT aliases remain reserved with extensions. Rejecting a non-file after opening it does not substitute for avoiding a device open in the first place.

### Executed tests and evidence limits

This reviewer executed on Windows:

- Host epr_host_seed_binding integration:final delta6 passed, covering seed mutation/expiry/admission, source/scope, parent currentness and CommonMark regions.
- Host output-stage cross-seed rejection with unchanged ledger:1 passed.
- Receipt half-open/zero-duration constructor and wire boundary:1 passed.
- Core epr_authority_binding integration:7 passed.
- Supervisor epr_filesystem integration:7 passed, excluding Linux-only ACL/symlink tests.
- Supervisor library:4 passed, including two cleanup-observation faults, a helper, Windows private DACL. Added pure superscript name validation:1 passed.
- CLI unknown effect-state unit:1 passed.
- Snapshot capsule:3 passed including helper, readonly input, hash/budget rejection, Windows denied writes, actual A/B/A backend helper and exact owned cleanup. Unix replacement-identity rejection needs separate Linux execution.
- Supervisor strict clippy/all-targets including snapshot:passed.

The same reviewer's previously added real post-edit head-failure CLI test also checks the parent's typed errors after an actual edit. It is local I/O evidence, not general exactly-once remote-effect assurance.

### Assurance-Compiler and retained unknowns

Doctor ran once with ok=true. Complete effect_error.rs, filesystem.rs, and unix.rs units were each ≤16KiB. All returned `blocked_with_diagnostics`, evidence=`missing_or_unknown`. Their8/13/6 diagnostics are adapter surfaces for those exact inputs, not product-bug counts, correctness, safety, or merge approval. Old input revisions cannot serve as current evidence after changes.

Windows jobs, Unix groups, POSIX ACLs, and snapshot cleanup have different premises. Noncooperating same-owner/privileged operations, group escapes, OS filesystem/network isolation, real host ingress/sinks, key custody, and external revocation remain distinct operational conditions. Unobserved conditions are neither newly demonstrated vulnerabilities nor successes.

### Assessment

Each finding A0–A10 in this bounded review has a linked correction delta or local regression. This is not a release certificate. Linux-specific gates, another reviewer's A3 capsule audit, final Media integration and the integrated snapshot gates need separate evidence; this record alone does not close EPR-19. Preserve legitimate positives and rerun adversarial checks under different conditions.

Another reviewer's audit B found B01: a dependency named/renamed std can replace `::std::assert_eq!`. This reviewer corrected the connection implementation, so B01's fix is outside independent audit A. After fail-first, five real Cargo producer cases passed, including wrong-oracle plain-Cargo success followed by producer rejection and legitimate package paths. Full offline metadata now checks target/dependency names, renames and resolution aliases; it does not authenticate the entire build world, ambient Cargo configuration, build scripts, proc macros, or toolchain trust, nor claim hermetic execution.
