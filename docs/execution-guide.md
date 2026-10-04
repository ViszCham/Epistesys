# Epistesys execution guide

## 日本語

### 実行前の条件

このガイドはsource checkoutからの診断と、既存の実行契約への入口を示します。配布済みbinary、モデルweight、receiptや秘密鍵の自動provisionを前提にしません。source-only配布の条件は[最終検証](../validation/epia2-alpha2-release-verification-2026-10-02.md)、CLI/SDKの適用範囲は[実行契約](dgcl-operating-profile-and-closure.md)を参照してください。

### 診断

repository rootで実行する例です。JSONの解析状態・source roundtrip・未解決事項を確認し、成功flagだけで実行や内容の正しさを判断しません。

```powershell
./scripts/run-epistesys.ps1 lc631-doctor --repo .
./scripts/run-epistesys.ps1 lc631-tl-doctor --prompt "Please test the package. Do not publish."
./scripts/run-epistesys.ps1 lc631-world-doctor --prompt "Please test the package. Do not publish."
```

| 出力 | 読み方 |
| --- | --- |
| version／command | 使用したidentityと呼出し経路を確認する |
| source roundtrip／revision | 原文byteとrevisionの保存。意味的正確性とは別 |
| parse defect／unresolved | 未対応、曖昧性、未抽出事項を保持する |
| Clarify／Hold | 確認または保留の状態。権限や真理値ではない |
| promotion_allowed=false | そのgateの条件が不足する。全実装が存在しないことは意味しない |
| world budget | localな仮説・射影のmaterialization。LLM呼出し数とは別 |

launcherは自身の配置からrunnerを解決するため、別cwdでもcheckout内のlauncherを指定できます。root外の個人host pathをrepository文書へ固定しません。

### Codexからの入力

Codex facadeの既定はalpha.2です。codingとconversationのfacadeを重複実行せず、元入力をUTF-8 stdinで`lc631-dgcl-run --prompt-stdin`へ渡します。source／Program IR／TL／candidate／未解決状態を同artifactで確認します。具体的なroutingは[Codex既定ルーティング](codex-default-routing.md)に記載しています。

configured-Stanzaは明示的なworker/model/runtime pinを必要とします。未設定構成の出力を実Stanza観測へ読み替えません。

### 副作用を伴う操作

検証・修復・再開のcommandは[DGCL実行契約](dgcl-operating-profile-and-closure.md)に沿って設定します。`--execute`だけでAuthority receipt、tool registration、intent bindingを代替できません。

- finalization：登録tool、外部権限、対象snapshot、独立intent、所定の検証計画を確認する。
- repair：repository/fileに限定した権限と変更後snapshotへのbinding、backup、journal/headを確認する。
- resume：旧Completedを信用して作用を再実行せず、現在のsource・証拠・権限を再検証する。
- host output：candidate確定、send、sink受領、replayを別stageとして確認する。

### 異常時の扱い

schema不適合、source/config drift、stale/expired evidence、未対応backend、未確認作用は診断を保持します。checkpoint、構造的近さ、hash一致、local testだけで完了・配信・権限を復元しません。caller／hostに依存する未観測範囲は[既知の制約](known-limitations.md)に分離しています。

## English

### Preconditions

This guide covers source-checkout diagnostics and entry to existing execution contracts. It assumes no automatic provisioning of distribution binaries, model weights, receipts, or private keys. See [final verification](../validation/epia2-alpha2-release-verification-2026-10-02.md) for source-only conditions and the [execution contract](dgcl-operating-profile-and-closure.md) for CLI/SDK scope.

### Diagnostics

Examples run from repository root. Inspect JSON parse states/source roundtrip/residuals rather than inferring execution or correctness from a success flag alone.

```powershell
./scripts/run-epistesys.ps1 lc631-doctor --repo .
./scripts/run-epistesys.ps1 lc631-tl-doctor --prompt "Please test the package. Do not publish."
./scripts/run-epistesys.ps1 lc631-world-doctor --prompt "Please test the package. Do not publish."
```

| Output | Interpretation |
| --- | --- |
| Version/command | Identify the selected implementation and route |
| Source roundtrip/revision | Byte/revision preservation, not semantic accuracy |
| Parse defects/residuals | Retained unsupported, ambiguous, or unextracted information |
| Clarify/Hold | Clarification/hold states, not authority or truth |
| promotion_allowed=false | Missing conditions for that gate, not absence of every implementation |
| World budget | Local hypothesis/projection materialization, not LLM call count |

Launchers resolve runners relative to their installation, so reference the checkout's launcher from another working directory. Do not embed personal absolute host paths in repository documentation.

### Codex input

Alpha.2 is the default facade. Do not double-run coding/conversation facades; pass original input as UTF-8 stdin to `lc631-dgcl-run --prompt-stdin`. Check source/Program IR/TL/candidate/residuals within the same artifact. The [default-routing contract](codex-default-routing.md) specifies the route.

Configured Stanza requires explicit worker/model/runtime pins. Never reinterpret unconfigured output as observed Stanza execution.

### Effectful operations

Configure verification/repair/resume commands according to the [DGCL execution contract](dgcl-operating-profile-and-closure.md). `--execute` alone does not replace Authority receipts, tool registration, or intent binding.

- Finalization: validate registered tools, external authority, target snapshot, independent intent, and required validation plan.
- Repair: validate repository/file-scoped permissions, post-repair snapshot binding, backups, and journals/heads.
- Resume: revalidate current source/evidence/authority without trusting old Completed or repeating effects.
- Host output: separately observe candidate finalization, send, sink receipt, and replay stages.

### Failure handling

Retain diagnostics for schema errors, source/config drift, stale/expired evidence, unsupported backends, and unconfirmed effects. Checkpoints, structural proximity, matching hashes, or local tests alone restore neither completion/delivery nor authority. [Known limitations](known-limitations.md) separates unobserved caller/host-dependent scope.
