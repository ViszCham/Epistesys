# Epistesys project instructions

## 日本語

Epistesysは、選択したv6.3.1 source snapshotから当初private cloneとして独立初期化された実験的プロジェクトです。現在のsource identityはEpistesys 6.3.2-alpha.2です。`docs/migration-contract.md`の由来・除外契約を保持します。

除外した継承元固有情報、個人状態、credentials、cache、receipt root、replay ledger、host絶対pathを再導入しません。Unavailable/Hold/Clarify/fallback/未観測は証拠境界であり成功ではありません。日本語の説明を前半、同内容の英語を後半に置きます。Rust source/schema/fixture/command IDの互換名は保持できます。Epistesys-7はこのalphaの範囲外です。

## English

Epistesys is an experimental project independently initialized from the selected v6.3.1 source snapshot, initially as a private clone.
Use the Epistesys identity and preserve the clone contract in `docs/migration-contract.md`.

Do not reintroduce excluded upstream-specific material, personal state, credentials, caches,
receipt roots, replay ledgers, or absolute host paths. Treat inherited Unavailable, Hold,
Clarify, fallback, and unobserved states as evidence boundaries rather than successful claims.

Japanese explanatory prose precedes equivalent English prose. Rust source, schemas, fixtures,
and command identifiers may retain their compatibility names while the product identity is
Epistesys 6.3.2-alpha.2. New Epistesys-7 behavior is outside this alpha.
