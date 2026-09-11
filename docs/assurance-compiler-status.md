# Epistesys 6.3.2-alpha.1 Assurance-Compiler boundary

## 日本語

EPI-05でAssurance-Compiler doctorを一度実行し、adapter healthは`ok=true`、deferred capabilitiesは`none`でした。次の16KiB以下の完全なRust単位をsource textとして監査しました。

| unit | status | diagnostics | evidence |
| --- | --- | ---: | --- |
| `lc631-receipt-kernel/src/lib.rs` | `blocked_with_diagnostics` | 27 | `missing_or_unknown` |
| `lc631-core/src/lib.rs` | `blocked_with_diagnostics` | 11 | `missing_or_unknown` |
| `lc631-world/src/lib.rs` | `blocked_with_diagnostics` | 9 | `missing_or_unknown` |
| `lc631-wire/src/lib.rs` | `blocked_with_diagnostics` | 5 | `missing_or_unknown` |

これらは固定されたsource textに対するbounded static reviewです。status、diagnostics、evidence gapをそのまま保持し、証明、compiler truth、runtime safety、repository-wide coverage、host activation、commit/push許可、release approvalへ昇格させません。`lc631-cli`、`lc631-analysis`、`lc631-media`、`lc631-host`、`lc631-accelerator`の大きな単位、Cargo全体、外部backend、配布binaryはこの監査範囲外です。Cargo check/test/clippy/buildの成功は別の実行証拠です。

## English

EPI-05 ran Assurance-Compiler doctor once; adapter health was `ok=true` with `deferred_capabilities=none`. The following complete Rust units no larger than 16 KiB were reviewed as source text.

| unit | status | diagnostics | evidence |
| --- | --- | ---: | --- |
| `lc631-receipt-kernel/src/lib.rs` | `blocked_with_diagnostics` | 27 | `missing_or_unknown` |
| `lc631-core/src/lib.rs` | `blocked_with_diagnostics` | 11 | `missing_or_unknown` |
| `lc631-world/src/lib.rs` | `blocked_with_diagnostics` | 9 | `missing_or_unknown` |
| `lc631-wire/src/lib.rs` | `blocked_with_diagnostics` | 5 | `missing_or_unknown` |

These are bounded static reviews of the pinned source text. Preserve status, diagnostics, and evidence gaps exactly; do not promote them into proof, compiler truth, runtime safety, repository-wide coverage, host activation, commit/push permission, or release approval. Larger `lc631-cli`, `lc631-analysis`, `lc631-media`, `lc631-host`, and `lc631-accelerator` units, the full Cargo workspace, external backends, and distribution binaries are outside this review. Cargo check/test/clippy/build success is separate execution evidence.
