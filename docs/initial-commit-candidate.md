# Epistesys 6.3.2-alpha.1 initial commit candidate

## 日本語

EPI-07で作成する初回コミット候補です。`Epistesys 6.3.2-alpha.1`として、選別したv6.3.1 sourceとEpistesys identity・境界文書を一つの新規履歴へ登録します。source revisionは`4aa84b8ddba80add1d5807413684a27d1356c6fa`です。

候補に含めるのは、`.codex-plugin`、`scripts/lc631`のCargo workspaceとsource/test、fixtures、schemas、hooks、skills、launcher、実装・検証・制約文書です。初回版はsource-only配布とし、absolute build pathを含む移植元binary、未監査binary、target、cache、log、credentials、receipt root、replay ledger、親Git履歴を含めません。

EPI-05のformat、check、test、strict clippy、locked release build、主要CLI、別cwd launcherは成功しています。release binaryは検証artifactとしてtargetに残りますが、配布対象にはしません。Assurance-Compilerはdoctor正常、4 bounded Rust unitが`blocked_with_diagnostics`／`missing_or_unknown`です。RepoSeiri 1.1は文書診断0、Markdown/conflict coverage complete、wording linter 0 findings。これらは各範囲の観測であり、clone全体の証明ではありません。

予定commit message：

```text
chore: initialize Epistesys 6.3.2-alpha.1 from Labyrinth-Codex v6.3.1
```

このファイルを含む候補は、commit/push/tag/install/host reloadの前に人が確認できるようstageします。remoteのprivate設定、root branch、既存remote commits、外部CIの起動状態はEPI-08開始時に再確認します。EPI-08はcommit/pushの明示許可があるまで`pending`です。

## English

This is the EPI-07 initial-commit candidate. It will register selected v6.3.1 source plus Epistesys identity and boundary documents in one new history under `Epistesys 6.3.2-alpha.1`. Source revision: `4aa84b8ddba80add1d5807413684a27d1356c6fa`.

The candidate includes `.codex-plugin`, the `scripts/lc631` Cargo workspace and source/tests, fixtures, schemas, hooks, skills, launcher, and implementation/verification/limitation documents. The initial version is source-only: migrated binaries containing absolute build paths, unaudited binaries, target, caches, logs, credentials, receipt roots, replay ledgers, and parent Git history are excluded.

EPI-05 format, check, test, strict clippy, locked release build, main CLI routes, and startup from another cwd passed. The release binary remains a validation artifact under target but is not a distribution artifact. Assurance-Compiler doctor was healthy; four bounded Rust units remain `blocked_with_diagnostics` / `missing_or_unknown`. RepoSeiri 1.1 reported zero document diagnostics, complete Markdown/conflict coverage, and zero wording findings. These are observations within their scopes, not proof of the entire clone.

Candidate commit message:

```text
chore: initialize Epistesys 6.3.2-alpha.1 from Labyrinth-Codex v6.3.1
```

Stage this file so a human can inspect the candidate before commit, push, tag, installation, or host reload. Recheck private remote visibility, root branch, intervening remote commits, and external-CI startup at EPI-08 entry. EPI-08 remains `pending` until explicit commit/push authorization is supplied.
