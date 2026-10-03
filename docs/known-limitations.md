# Epistesys 6.3.2-alpha.2 known limitations

## 日本語

[研究目標・設計目標・研究仮説](research-hypotheses.md)を区別する。hallucination containment、constraint preservation、long-horizon instruction integrity、failure observability、reliable complexity frontierの研究効果はbenchmark pending / independent evaluation pendingである。

- 前身の[GB-CC75探索的評価](research/gb-cc75-study.md)は歴史的観測であり、現alphaの独立実証ではない。65主要QID／167採点ペアへの選択、Judgeの機構依存、未確定生成設定・予算、未同梱raw回答を保持する。公開CSVの再計算は独立再採点や再実験ではない。[敵対監査](research/gb-cc75-adversarial-audit.md)の未解決実験課題を文書変更だけで閉じない。
- 初回版はLabyrinth-Codex v6.3.1のクローンであり、Epistesys独自の推論改善を含まない。
- packaged binaryはclone・依存crate・toolchainのabsolute build path監査が未完了のため、source-only扱いである。
- `lc631-tl-doctor`は入力によって`Clarify`、parse defect、Program IRまたはhost output未bindingを返し得る。source roundtrip exactはsemantic exactnessではない。
- DGCL/EPIA2は、同sourceのcontrolled grammar→Program IR→TL→要求別completion、実Cargo検証、署名evidence、実ファイル修復、journal/head付き再開、構造蒸留下流接続、consumer negotiationを追加した。完成の判定は宣言profileの最新functional gateに限定する。model weightsは配布せず、独立gold、任意自然言語の完全文法、host最終callback、一般意味精度は未実証である。[対応範囲](dgcl-operating-profile-and-closure.md)と歴史的な[DGCL実装状況](dgcl-implementation-status-2026-09-28.md)を区別する。
- `lc631-world-doctor`の2,048 evaluationはlocal materializationであり、2,048回のLLM呼出しや一般性能証拠ではない。
- GPUのlane parity、readback、device timing、shader occupancy、overlap、device-loss callbackの登録・発生は別観測である。
- RPA、Media、external parser、license、model、remote CI、Host v2 pickupは、個別receiptがない限りrelease completeへ進まない。
- inherited historical docs、local tests、hash一致、Assurance-Compiler statusは、host activation、runtime safety、形式証明、一般性能、権限を証明しない。
- commit、push、tag、plugin install、host reloadは初回cloneの内容から自動実行されない。

次のalpha検証では、sourceからの再現可能binary、別cwd起動、fixture出力、schema互換性、receipt/replay分離を再確認する。問題があればclone契約を更新し、Epistesys-7へ機能変更を混ぜない。

## English

Distinguish [research goals, design objectives, and research hypotheses](research-hypotheses.md). Research effects for hallucination containment, constraint preservation, long-horizon instruction integrity, failure observability, and the reliable complexity frontier remain benchmark pending / independent evaluation pending.

- The predecessor [exploratory GB-CC75 study](research/gb-cc75-study.md) is historical evidence, not independent current-alpha validation. Retain selection to 65 primary QIDs/167 scored pairs, Judge mechanism dependence, unresolved generator settings/budgets, and excluded raw answers. Public CSV recomputation is not independent regrading or experimental replication. Documentation changes do not close experimental gaps in the [adversarial audit](research/gb-cc75-adversarial-audit.md).
- The initial version is a clone of Labyrinth-Codex v6.3.1 and contains no Epistesys-specific reasoning improvement.
- It remains source-only because absolute build paths in the clone, dependency crates, and toolchain have not completed the distribution audit.
- `lc631-tl-doctor` may return `Clarify`, parse defects, or unbound Program IR/host output for an input. Exact source roundtrip is not semantic exactness.
- DGCL/EPIA2 adds same-source controlled grammar→Program IR→TL→per-requirement completion, actual Cargo validation, signed evidence, real file repair, journal/head-backed resume, downstream structural distillation, and consumer negotiation. Completion decisions are limited to current functional gates for the declared profile. Model weights are not distributed; independent gold, complete arbitrary natural-language grammar, final host callbacks, and general semantic accuracy remain unproven. Distinguish the [operating scope](dgcl-operating-profile-and-closure.md) from historical [DGCL implementation status](dgcl-implementation-status-2026-09-28.md).
- The 2,048 evaluations from `lc631-world-doctor` are local materialization, not 2,048 LLM calls or general-performance evidence.
- GPU lane parity, readback, device timing, shader occupancy, overlap, and device-loss callback registration/occurrence are separate observations.
- RPA, Media, external parsers, licenses, models, remote CI, and Host v2 pickup do not reach release complete without their own receipts.
- Inherited historical docs, local tests, matching hashes, and Assurance-Compiler status do not prove host activation, runtime safety, formal proof, general performance, or authority.
- Commit, push, tag, plugin installation, and host reload do not run automatically from the initial clone content.

The next alpha verification should recheck reproducible source builds, startup from another cwd, fixture output, schema compatibility, and receipt/replay separation. Update the clone contract when needed and keep Epistesys-7 feature changes separate.
