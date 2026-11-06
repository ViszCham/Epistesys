# Epistesys project instructions

## 日本語

Epistesys Beta-7.1（`7.1.0-beta.1`）は、要求追跡・検証・修復を支援する開発ツールです。[移行契約](docs/migration-contract.md)と[現在の検証範囲](docs/epistesys-7.1.0-beta.1-release-notes-2026-11-06.md)を保持してください。

- coding・test・reviewにはcoding facade、説明・設計にはconversation facadeを一つだけ使います。利用者の選択とhost設定を優先し、別lineとの二重起動はしません。
- 元のsource、Program IR、EpiMapLoss、candidate、evidence、authorityを区別し、Unavailable・Hold・Clarify・未観測を成功に変えません。診断は権限を作らず、許可された作業を禁止するものでもありません。
- 通常起動は同梱binaryだけを使い、暗黙buildやturnごとのbuild cacheを作りません。具体的な起動・再利用条件は[plugin接続契約](docs/plugin-host-integration.md)にまとめています。
- 個人設定、鍵、model、receipt root、replay、cache、host絶対pathを公開sourceへ持ち込みません。ユーザー差分と唯一の証跡を保持します。
- 日本語の説明を前半、同内容の英語を後半に置きます。既存のRust・schema・command互換IDは保ちます。
- source version、Git merge、plugin install、有効化、実hostの読み込みは別の状態です。観測したOS・feature・入力の範囲だけを報告してください。

## English

Epistesys Beta-7.1 (`7.1.0-beta.1`) is a development tool for requirement traceability, validation and repair. Preserve the [migration contract](docs/migration-contract.md) and [current verification scope](docs/epistesys-7.1.0-beta.1-release-notes-2026-11-06.md).

- Use exactly one facade: coding for code/tests/review, conversation for explanation/design. Respect user selection and host configuration; do not double-run another line.
- Separate original source, Program IR, EpiMapLoss, candidates, evidence and authority. Keep Unavailable, Hold, Clarify and unobserved states. Diagnostics neither grant permission nor prohibit authorized work.
- Normal launches use the packaged binary only, without implicit builds or per-turn build caches. See the [plugin integration contract](docs/plugin-host-integration.md) for invocation and reuse conditions.
- Exclude personal settings, keys, models, receipt roots, replay state, caches and absolute host paths from public source. Preserve user differences and unique evidence.
- Put Japanese explanatory prose before equivalent English. Retain existing Rust, schema and command compatibility IDs.
- Source versions, Git merges, plugin installation, enablement and actual host pickup are separate states. Report only the observed OS, feature and input scope.
