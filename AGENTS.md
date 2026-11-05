# Epistesys project instructions

## 日本語

Epistesysは、ソフトウェアの要求追跡・検証・修復を支援する開発ツールです。選択したv6.3.1のソースから、当初は非公開の独立したGit履歴として開始しました。現在のsource identityはBeta 6.9（`6.9.0-beta.1`）で、未完成のTLCX実装を保存した中間スナップショットです。`docs/migration-contract.md`の由来・除外契約を保持します。sourceのversion変更を、既存hostのplugin更新や完成判定とは扱わないでください。

除外した継承元固有情報、個人状態、credentials、cache、receipt root、replay ledger、host絶対pathを再導入しません。Unavailable/Hold/Clarify/fallback/未観測は証拠境界であり成功ではありません。日本語の説明を前半、同内容の英語を後半に置きます。Rust source/schema/fixture/command IDの互換名は保持できます。この保存版から、後続のBeta-7.0の完成を宣言しないでください。

## English

Epistesys is a development tool for tracing software requirements, validating changes, and supporting repair. It began with an independent Git history, initially private, based on a selected v6.3.1 source snapshot.
Use the Epistesys identity and preserve the clone contract in `docs/migration-contract.md`.

Do not reintroduce excluded upstream-specific material, personal state, credentials, caches,
receipt roots, replay ledgers, or absolute host paths. Treat inherited Unavailable, Hold,
Clarify, fallback, and unobserved states as evidence boundaries rather than successful claims.

Japanese explanatory prose precedes equivalent English prose. Rust source, schemas, fixtures,
and command identifiers may retain their compatibility names while the product identity is
Beta 6.9 (`6.9.0-beta.1`), an intermediate checkpoint of unfinished TLCX work.
Do not treat the source-version change as an existing-host plugin update or a completion decision,
and do not claim that the later Beta-7.0 work is complete from this checkpoint.
