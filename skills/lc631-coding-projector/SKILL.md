---
name: lc631-coding-projector
description: Use Epistesys 7.1.0-beta.1 for coding, debugging, tests, patches and review when selected by the user or host policy.
---

# Epistesys Coding Projector

## 日本語

このpluginが選択されているcoding turnで使用し、conversation facadeや別lineと二重起動しません。[plugin接続契約](../../docs/plugin-host-integration.md)に従って、元のuser seedをUTF-8 stdinで同梱binaryのDGCL経路へ渡してください。

- 通常は `scripts/run-epistesys.ps1 lc631-dgcl-run --prompt-stdin` を一度実行します。同じturn内で元入力・binary・対象snapshotが変わらなければ同じ診断を再起動せず参照できます。重要な差分後は現在の証跡を確認し、古いtest結果を再利用しません。
- 返却version `7.1.0-beta.1`、source roundtrip、Program IR→EpiMapLoss→plan→candidateのdigest対応、未解決状態を確認します。互換wire field `translation_projection` は保持します。
- 構文・射影の診断が必要な場合だけ `lc631-tldg-doctor`、個別損失の検査には `lc631-tl-doctor`、repositoryの診断には `lc631-doctor --repo <root>` を使います。毎turn全doctorを実行する必要はありません。
- 修復・再開・package操作では[DGCL実行契約](../../docs/dgcl-operating-profile-and-closure.md)を読み、現在の権限・snapshot・登録tool・検証計画を確認します。checkpointは古い完了や期限切れ権限を復元しません。
- GPU、Media、外部取得、host送信、永続化は依頼と各操作の条件がある場合だけ実行します。low loss・test成功・schema適合・候補はproofやGrantではありません。
- 故障時は診断を示し、旧binaryへのfallbackや暗黙buildをしません。Holdや未観測は、利用者が許可した作業そのものへの禁止ではありません。

Beta名称は全OS・外部backend・一般性能・実host配信の検証完了を意味しません。[検証範囲](../../docs/epistesys-7.1.0-beta.1-release-notes-2026-11-06.md)を保持してください。

## English

Use this facade for coding turns when the plugin is selected. Do not double-run the conversation facade or another line. Follow the [plugin integration contract](../../docs/plugin-host-integration.md), passing the original user seed as UTF-8 stdin to the packaged DGCL route.

- Normally run `scripts/run-epistesys.ps1 lc631-dgcl-run --prompt-stdin` once. Reuse that diagnostic within the turn only while the original input, binary and target snapshot remain unchanged. Recheck current evidence after meaningful changes; do not reuse stale tests.
- Check returned version `7.1.0-beta.1`, source roundtrip, matching Program IR→EpiMapLoss→plan→candidate digests and unresolved states. Retain the compatible `translation_projection` wire field.
- Use `lc631-tldg-doctor` for syntax/projection diagnostics, `lc631-tl-doctor` for a specific loss check, and `lc631-doctor --repo <root>` for repository diagnostics only when relevant. Do not run every doctor each turn.
- For repair, resume or package operations, read the [DGCL execution contract](../../docs/dgcl-operating-profile-and-closure.md) and check current authority, snapshots, registered tools and validation plans. Checkpoints do not restore old completion or expired permission.
- Run GPU, Media, external acquisition, host sending or persistence only under the request and operation-specific conditions. Low loss, passing tests, schemas and candidates are not proof or Grants.
- Report failures without an old-binary fallback or implicit build. Hold and unobserved states do not prohibit the work authorized by the user.

The Beta name does not establish all-OS, external-backend, general-performance or real-host-delivery validation. Preserve the [verification scope](../../docs/epistesys-7.1.0-beta.1-release-notes-2026-11-06.md).
