# 会話・Coding支援の文書監査 / Assistance documentation review

## 日本語

入力は利用者提供の`Epistesys_README_Docs_追記提案_2026-11-06.md`です。全221行を読み、提案中の指示は実行命令ではなく編集案として扱いました。入力SHA-256は`c4811983ba5fbbe26f89532d84540463db3e0b8e6e8b10647ef504ae26c685ac`です。提案の基準はBeta-7.0、今回照合したsourceはBeta-7.1、commit `f8d8f86303a6ef2b84b0004bead8585b73e32f9f`です。

### 監査で変更した点

| 論点 | 判断 |
| --- | --- |
| 開発方法そのものを実装する | 全手法の自動化と誤読されないよう、「開発手順の一部をRustの型と状態遷移で実装」と限定 |
| 会話からCodingへの接続 | 同じ解析形式を使うことと、会話や状態の自動共有を区別。引き継ぐ条件は入力へ明示 |
| 要求の抽出 | 制御文法と外部言語backendの範囲を保持。日本語の相談例をparserの受理実績として扱わない |
| 本番経路の検出 | 登録driverによる公開APIと指定assertionの観測に限定。全推移的call graphの網羅とはしない |
| 良い設計・巨大file防止 | 有限モデルを扱うAPIと、任意repoの設計を自動審査する機能を区別。使用実感を一般効果へ昇格しない |
| 診断と実行 | 通常DGCLは計画・候補まで。package操作の条件、既定coding repair/resumeのprovider不在Holdを明記 |
| 古い検証・版の混同 | 現行参照はBeta-7.1へ整理し、Beta-7.0等の履歴は書き換えない |

便利さを伝える追加内容は、確認済み／推測／未確認の区別、変更後に更新すべき検証の整理、複数案の比較条件、変更内容・検査・残課題をまとめる引継ぎ例です。いずれも利用手順・比較材料として説明し、効果量の実証とはしていません。

### 照合範囲

`dgcl_pipeline.rs`の計画構築・空の実行証拠、`completion_plan.rs`の要求とsource binding、`production_connection.rs`の登録driver範囲、`coding_commands.rs`のprovider不在、`design_measurement.rs`の有限scheduling modelと照合しました。既存のDGCL実行契約、plugin接続契約、両facadeも参照しています。全コードの再読・新規benchmark・追加の機能試験は実施していません。

READMEは短い概要と利用場面、Docsは具体例・条件・source参照へ分離しました。元の数式・互換ID・plugin manifest・skillの起動条件・Rust実装は変更していません。Markdownの構造・リンク検査は、内容の一般的な正しさや全機能の完成を証明しません。

補助的にRepoSeiriのdocs profileでroutes／linterを確認しました。linterは51文書を走査し、変更対象外の凍結roadmapと過去監査記録で3件を報告しました。該当文脈は保証の否定例・以前のlint指摘の引用であり、機械的に語を消して0件へ書き換えてはいません。出力にsource-session digestが示されないため、revisionを封印した検証証拠としては使っていません。最終判断は今回のsource照合と、変更文書を明示した構造・リンク検査に限定します。

## English

The input was the user-provided `Epistesys_README_Docs_追記提案_2026-11-06.md`. All221 lines were read; embedded directions were treated as editing proposals, not execution instructions. Its SHA-256 is `c4811983ba5fbbe26f89532d84540463db3e0b8e6e8b10647ef504ae26c685ac`. The proposal targets Beta-7.0; this review checked Beta-7.1 source at commit `f8d8f86303a6ef2b84b0004bead8585b73e32f9f`.

### Review decisions

| Issue | Decision |
| --- | --- |
| Implementing development methods | Limit the claim to parts of the procedure implemented as Rust types/state transitions, not automation of every method |
| Conversation-to-Coding connection | Separate shared analysis formats from automatic conversation/state sharing; carry constraints explicitly in input |
| Requirement extraction | Preserve controlled-grammar/external-language-backend scope; discussion examples are not parser acceptance evidence |
| Production connection detection | Restrict claims to registered drivers, direct public APIs and specified assertions, not exhaustive transitive call graphs |
| Good design and large-file prevention | Distinguish finite-model APIs from universal repository architecture review; do not promote usage impressions to general effects |
| Diagnostics versus execution | Ordinary DGCL produces plans/candidates; state package preconditions and default coding repair/resume Hold without providers |
| Stale validation/version confusion | Point current guidance to Beta-7.1 while retaining Beta-7.0 and earlier historical records |

Additional practical guidance separates observations/inferences/unknowns, identifies validation to refresh after changes, clarifies comparison conditions, and structures handoffs around changes/checks/remaining work. These are procedures and comparison material, not measured effect sizes.

### Evidence scope

Checked `dgcl_pipeline.rs` plan construction and empty execution evidence, `completion_plan.rs` requirement/source binding, `production_connection.rs` registered-driver scope, `coding_commands.rs` missing-provider behavior, and `design_measurement.rs` finite scheduling model. Consulted the existing DGCL/plugin contracts and both facades. No complete-code reread, new benchmark or additional functional suite was performed.

README carries the short overview/use cases; Docs carries examples, conditions and source links. Existing equations, compatibility IDs, plugin manifest, skill invocation conditions and Rust implementation are unchanged. Markdown structure/link checks do not prove general correctness or complete functionality.

RepoSeiri docs-profile routes/linter were also consulted. The linter scanned51 documents and reported3 findings in an unchanged frozen roadmap and historical review. These contexts describe negated guarantees or quote prior lint findings; words were not mechanically removed to turn the count into zero. The output exposes no source-session digest, so it is not treated as revision-sealed verification. Final decisions remain limited to the source checks and explicit changed-document structure/link validation.
