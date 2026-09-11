# Epistesys 6.3.2-alpha.1 clone verification

## 日本語

EPI-05の検証記録です。作業ディレクトリはEpistesys clone、対象workspaceは`scripts/lc631/Cargo.toml`です。

### 通過した検証

- `cargo fmt --all -- --check`
- `cargo check --workspace --no-default-features`
- `cargo check --workspace`
- `cargo test --workspace --no-default-features`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets --no-default-features -- -D warnings`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo build --release --locked`
- release binaryで`lc631-world-doctor`：version `6.3.2-alpha.1`、256 worlds、2,048 evaluations、materialized 2,048、exact ceiling true
- release binaryで`lc631-tl-doctor`：version `6.3.2-alpha.1`、gate `clarify`、source roundtrip exact true、parse defect 1
- release binaryで`lc631-promotion-gate`：promotion false、automatic promotion false
- clone rootから離れたcwdで`run-epistesys.ps1 lc631-world-doctor`：version `6.3.2-alpha.1`、2,048 evaluations

### 配布binaryの境界

生成release binaryは`target/release`に置かれています。検査ではcloneのabsolute pathがdebug metadataへ残ることを確認したため、`scripts/lc631/bin`へコピーせず、Git登録もしません。EPI-10相当の再現可能path remapまたはstrip方針が別途成立するまで、Epistesys alphaはsource-only distributionとして扱います。依存crateのregistry・toolchain pathも同じ検査対象です。

### 残存状態

Inherited `Unavailable`、`Hold`、`Clarify`、fallback、external receipt不足は保持します。`promotion_allowed=false`は失敗ではなく、必要なexternal evidenceがないcloneのrelease境界です。local testは実行結果の観測であり、正しさ、host activation、一般性能、形式証明ではありません。

## English

This is the EPI-05 verification record for the Epistesys clone. The workspace is `scripts/lc631/Cargo.toml`.

### Passed checks

- `cargo fmt --all -- --check`
- `cargo check --workspace --no-default-features`
- `cargo check --workspace`
- `cargo test --workspace --no-default-features`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets --no-default-features -- -D warnings`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo build --release --locked`
- Release `lc631-world-doctor`: version `6.3.2-alpha.1`, 256 worlds, 2,048 evaluations, 2,048 materialized, exact ceiling true
- Release `lc631-tl-doctor`: version `6.3.2-alpha.1`, gate `clarify`, source roundtrip exact true, parse defect 1
- Release `lc631-promotion-gate`: promotion false, automatic promotion false
- `run-epistesys.ps1 lc631-world-doctor` from outside the clone root: version `6.3.2-alpha.1`, 2,048 evaluations

### Distribution binary boundary

The generated release binary resides under `target/release`. Inspection found the clone absolute path in debug metadata, so it is not copied to `scripts/lc631/bin` and is not registered in Git. Until a reproducible path-remap or stripping policy is established in a later gate, this alpha is treated as source-only distribution. Dependency registry and toolchain paths receive the same audit.

### Remaining states

Retain inherited `Unavailable`, `Hold`, `Clarify`, fallback, and missing external receipts. `promotion_allowed=false` is the release boundary for a clone without required external evidence, not an implementation failure. Local tests are observations of execution, not correctness, host activation, general performance, or formal proof.
