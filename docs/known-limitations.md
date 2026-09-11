# Epistesys 6.3.2-alpha.1 known limitations

## 日本語

- 初回版はLabyrinth-Codex v6.3.1のクローンであり、Epistesys独自の推論改善を含まない。
- packaged binaryはclone・依存crate・toolchainのabsolute build path監査が未完了のため、source-only扱いである。
- `lc631-tl-doctor`は入力によって`Clarify`、parse defect、Program IRまたはhost output未bindingを返し得る。source roundtrip exactはsemantic exactnessではない。
- `lc631-world-doctor`の2,048 evaluationはlocal materializationであり、2,048回のLLM呼出しや一般性能証拠ではない。
- GPUのlane parity、readback、device timing、shader occupancy、overlap、device-loss callbackの登録・発生は別観測である。
- RPA、Media、external parser、license、model、remote CI、Host v2 pickupは、個別receiptがない限りrelease completeへ進まない。
- inherited historical docs、local tests、hash一致、Assurance-Compiler statusは、host activation、runtime safety、形式証明、一般性能、権限を証明しない。
- commit、push、tag、plugin install、host reloadは初回cloneの内容から自動実行されない。

次のalpha検証では、sourceからの再現可能binary、別cwd起動、fixture出力、schema互換性、receipt/replay分離を再確認する。問題があればclone契約を更新し、Epistesys-7へ機能変更を混ぜない。

## English

- The initial version is a clone of Labyrinth-Codex v6.3.1 and contains no Epistesys-specific reasoning improvement.
- It remains source-only because absolute build paths in the clone, dependency crates, and toolchain have not completed the distribution audit.
- `lc631-tl-doctor` may return `Clarify`, parse defects, or unbound Program IR/host output for an input. Exact source roundtrip is not semantic exactness.
- The 2,048 evaluations from `lc631-world-doctor` are local materialization, not 2,048 LLM calls or general-performance evidence.
- GPU lane parity, readback, device timing, shader occupancy, overlap, and device-loss callback registration/occurrence are separate observations.
- RPA, Media, external parsers, licenses, models, remote CI, and Host v2 pickup do not reach release complete without their own receipts.
- Inherited historical docs, local tests, matching hashes, and Assurance-Compiler status do not prove host activation, runtime safety, formal proof, general performance, or authority.
- Commit, push, tag, plugin installation, and host reload do not run automatically from the initial clone content.

The next alpha verification should recheck reproducible source builds, startup from another cwd, fixture output, schema compatibility, and receipt/replay separation. Update the clone contract when needed and keep Epistesys-7 feature changes separate.
