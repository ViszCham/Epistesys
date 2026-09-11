# Epistesys 6.3.2-alpha.1 adoption manifest

## 日本語

EPI-02で固定した採用方針です。Labyrinth-Codex v6.3.1の選択subtreeから、必要なsource、test、fixture、schema、script、hook、skill、実装文書、Codex manifestを追跡ファイルのallowlistで採用します。親リポジトリの`.git`、remote設定、個人状態は採用しません。

採用しないものは次です。

- `.claude-plugin/plugin.json`：対象host外の入口であり、Epistesys Codex cloneの必須経路ではない。
- source `AGENTS.md`：v6.3.1専用の運用指示をそのまま継承せず、Epistesys用に再作成する。
- `scripts/lc631/bin/windows-x86_64/lc631.exe`：ビルド元ユーザーの絶対pathを含むため、sourceから再ビルドする。
- `scripts/lc631/bin/macos-arm64/lc631`：同じ配布物監査を通していないため、sourceから再ビルドする。
- parent Git履歴、cache、log、credentials、receipt root、replay ledger、個人設定、未収録の外部資料。

文字列、リンク、図、JSON、コメント、fixture、metadataは、製品固有の非開示情報、個人path、秘密値、不要なhost接続がないか検査する。名前の置換だけで採用可としない。必要な権利・license表示は保持する。

採用ファイル数はEPI-03で実際のallowlistから再計数する。packaged binaryのhashは移植元identityの観測値としてEPI-01に残すが、Epistesysの配布物として採用しない。Epistesys binaryはEPI-05でclone sourceから生成し、埋込みpath・identity・hashを別に検証する。

## English

This is the EPI-02 adoption policy. From the tracked v6.3.1 plugin subtree, adopt required source, tests, fixtures, schemas, scripts, hooks, skills, implementation notes, and the Codex manifest through an explicit allowlist. Do not adopt the parent repository `.git`, remote settings, or personal state.

The following are excluded:

- `.claude-plugin/plugin.json`: an entry point for another host and not required by the Epistesys Codex clone.
- Source `AGENTS.md`: do not inherit v6.3.1-specific operational instructions unchanged; recreate an Epistesys file.
- `scripts/lc631/bin/windows-x86_64/lc631.exe`: it contains an absolute build-user path, so rebuild from source.
- `scripts/lc631/bin/macos-arm64/lc631`: it has not passed the same distribution audit, so rebuild from source.
- Parent Git history, caches, logs, credentials, receipt roots, replay ledgers, personal settings, and unlisted external materials.

Inspect strings, links, diagrams, JSON, comments, fixtures, and metadata for non-disclosable project-specific information, personal paths, secret values, and unnecessary host connections. Renaming alone is not an adoption decision. Retain required rights and license notices.

EPI-03 will recount adopted files from the actual allowlist. Packaged-binary hashes remain source-identity observations in EPI-01 but are not adopted as Epistesys distribution artifacts. EPI-05 will build an Epistesys binary from clone source and inspect embedded paths, identity, and hash separately.
