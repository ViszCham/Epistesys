# Epistesys RepoSeiri audit / RepoSeiri監査

## 日本語

### alpha.3固定snapshotの文書監査

2026-10-27 JSTに、RepoSeiri `1.1.0+codex.20260823031336`のRust coreを正規bundle launcherから実行した。対象はGit commit `323c0b1b266c13156dd2cd1a4ba2a56c6b6b9142`のtracked sourceだけを新規展開した固定export、profileは`common`、scopeは`subtree`である。`git -c core.autocrlf=false -c core.eol=lf archive`で新規展開し、regular file 320件すべてのraw Git blob object ID一致と、余分・欠落fileがないことを確認した。cache、weight、秘密鍵、receipt root、個人host状態を持ち込んでいない。Git archiveにはGit履歴metadataを含めないため、時間的活動の未観測を保守状態の判定にしない。

`summary`、`linter`、`governance`、`pr-body`はすべてexit 0。前後の`summary`が返したsource-session digestは、ともに`sha256:821f4049acd66186f1afc363cb5b8051bd1828ba22d11a470644ad610c8f9cca`で、entry数・文書数・選択byte数も一致した。この版の`linter`／`governance`／`pr-body` wireはnativeなsession-digest fieldを返さないため、「各wire内のdigest fieldが一致した」とは主張しない。本記録が、固定Git exportと前後のcanonical summaryを使った入力bindingを保持する。これは、存在しないfieldの補完やnative attestationへの昇格ではない。

| 観測 | 固定alpha.3 snapshotの結果 |
| --- | --- |
| Entries / document events / document diagnostics | 397 / 8,579 / 0 |
| Documents | 67 selected / 67 candidates、962,100 bytes |
| Markdown / conflict coverage | `Complete` / `Complete` |
| Coverage scopes | 79 complete、0 partial、1 not requested、0 limit exceeded |
| Repository observations | 52 present、25 absent、1 unknown、0 conflict、0 limit exceeded |
| README grammar / program capability nodes | 139 / 2,727。program unknown reasonは`unsupported_syntax` |
| Summary claims / findings | 11 / 4 |
| Underclaim / overclaim / baseline unknown | 1 / 8 / 8。claim draftsは0 |
| Patch operations / holds | 2 / 7、`writes_files=false` |
| Wording linter | 41 files、3 generated surfaces、4 findings、15 suppressed boundary exceptions |
| Governance obligations / relations | 8 / 884 |
| Governance conflicts / proposition-conflict candidates | 0 / 458。後者はすべて`kind=version`の候補 |
| README translation alignment | 0 alignments、`unsupported_syntax`を保持 |
| Target reachability | 20 repository-local present、0 local missing、1 non-local or unknown |
| Lifecycle / temporal activity | `unsafe_to_invent` / `NotRequested` |
| PR body | draftを1件生成。GitHubへの新規PR作成・投稿は行っていない |

#### 入力・出力bindingの再現条件

export manifest SHA-256は`271542e925fbde31bd7c574b8baaffa7bbb28df60ee7a6865935641c9052612a`、Git archive ZIP SHA-256は`6e0a63e92a5519ef19db79fb74563c23b17fa70b65466764c3ba03ada84f2962`。manifestはGit tree順の`[{path,bytes,sha256}]`をcompact JSON・UTF-8・BOMなしで符号化した。Git blob IDの照合はGit objectの同一性確認であり、安全性や意味的正確性の証明ではない。

各queryは同じ正規bundle launcherに`-Path <fixed-export> -Profile common -Scope subtree -Query <query> -Format json`を渡した。以下はcaptured native JSONの各lineをLFで結合したUTF-8 text（BOM・末尾LFなし）のSHA-256であり、元stdioの改行を含むbyte列やsource-session digestとは区別する。所要時間はこの呼出しの局所観測であり、一般latencyのbenchmarkではない。

| Query | Exit | Elapsed ms | Output text SHA-256 |
| --- | --- | ---: | --- |
| summary（前） | 0 | 833 | `cf61b0e1a8a75b02590cc9a8ca1504640e7aaee7502d2166924f2266a787ac11` |
| linter | 0 | 752 | `69643ce8d878c8f74da76aa68055b59c7530b19e44e425f71edca5b58c213cf0` |
| governance | 0 | 1349 | `85c7c1ba268ce0b0511e5655df38949bfb9dfc1d2d1a5a5d895675d4a059b1a2` |
| pr-body | 0 | 748 | `e8bb2e2c40aaa1ef2f419e13d926d3fa82bd8b26cd4e1da3ed4ef7fbbb215e6e` |
| summary（後） | 0 | 718 | `cf61b0e1a8a75b02590cc9a8ca1504640e7aaee7502d2166924f2266a787ac11` |

先行するmaterialized exportはhostの`core.autocrlf`に従うCRLF変換を含み、summary digestは`sha256:85eb89465dbf903cf0982cde2a666325aa9d197352e473c19769602450da993f`、selected source bytesは971,166だった。これは同commit由来の別byte profileの履歴であり、raw Git blob exportの最終結果へ流用しない。旧exportを削除・上書きせず、上記320-file照合済みの新規LF exportを最終の入力とした。

#### 手動解釈と未解決状態

linterの4件は、固定snapshotの`docs/system-architecture.md`の137行・139行にある否定境界の2件と、`docs/reposeiri-audit.md`の75行がその過去所見を説明した2件である。手動の文脈確認では肯定的な保証claimとは解釈しないが、machine resultは4 findingsのまま保持する。過去の2 findingsを今回の結果へ流用せず、今回の4件を0件にも書き換えない。

repository observationの`conflict=0`と、governanceの458 proposition-conflict candidatesは別の軸である。後者には、履歴版と現行版、短縮版名、別toolの版、平均適合度の数値などを同じ`project_version`へ抽出した候補が含まれる。例えば`6.3.2-alpha.3`対`6.3.1`／`alpha.2-specific`／`74.38`である。458件を458実装欠陥と扱わず、全件を自動的に誤検出として消すこともしない。candidate-onlyのscope確認を要するreview状態として保持する。

READMEの自動翻訳alignmentは`unsupported_syntax`であり、日英同内容を機械的に認証した結果ではない。日英の意味・claim levelの手動照合と、別途の17 bilingual documents／320 linksの構文・metric-token検査を区別する。program unknown、7 holds、8 baseline unknownを、研究性能・公開準備・policyの成功へ変換しない。policy、license、support promiseを発明してholdを消していない。

#### 記録の因果的な境界

この監査sectionの追記自体は、上記commitの監査が終わった後のdocumentation-only変更であり、監査snapshotには含まれない。数値とdigestは`323c0b1b266c13156dd2cd1a4ba2a56c6b6b9142`に固定し、この後続記録を含むtree全体の再監査を装わない。README、source、実行契約などに実質的な後続変更があれば新しいexportとqueryが必要である。source release readiness、commit/push/merge、plugin install、host activation、研究効果は、それぞれ別の判断・権限・観測であり、RepoSeiriの出力から自動的に成立しない。以下のcfe候補とEPI-05の数値は履歴として保持する。

### EPR候補の観測：最終sourceではない

2026-10-27 JSTに、RepoSeiri `1.1.0+codex.20260823031336`のRust coreをbundle launcherから実行し、`common` profile・**subtree scope**で固定source-only exportを検査した。export元commitは`cfe29f89403f44ed1cd55552cf652c7c11cb3425`、summaryのsource-session digestは`sha256:4cd84e782284363ae1ec1b98b34f7950c9bc3cd827f5d5ae913a28611524a9d3`である。exportは親repositoryを継承する作業用subtreeのため、履歴のrepository scopeとは同一scopeとして比較しない。

`summary`と`linter`を同じ固定exportへ実行し、書込みは行っていない。これは`6.3.2-alpha.2`から次の候補を点検する中間snapshotであり、alpha.3の最終source・release・公開準備を認証するものではない。後続修正やこの文書の更新へ、以下の数値をそのまま引き継がない。

| 観測 | 固定候補の結果 |
| --- | --- |
| Entries / document events / document diagnostics | 389 / 8,064 / 0 |
| Documents | 64 selected / 64 candidates、918,038 bytes |
| Markdown / conflict coverage | `Complete` / `Complete` |
| Limit-exceeded observations / scopes | 0 / 0 |
| Repository observations | 52 present、25 absent、1 unknown、0 conflict |
| Root README grammar nodes | 136 |
| Profile fit | 70/100、top profile `cli`。品質・性能の点数ではない |
| Summary | underclaim opportunities 1、overclaim risks 8、findings 4 |
| Claim drafts / baseline unknown | drafts 0、baseline unknown 8 |
| Patch operations / holds | 2 / 7、`writes_files=false` |
| Wording linter | 40 files、3 generated surfaces、2 findings、10 suppressed boundary exceptions |

linterの2件は`docs/system-architecture.md`の固定export上の137行・139行で、対応表の「Not equivalent to」列にある`guaranteed meaning preservation`と`correctness guarantee`を検出したものだった。文脈上はいずれも保証を否定する境界説明であり、手動確認では肯定的な保証claimではないと判断した。ただし、machine resultは2 findingsのまま保持し、0件へ書き換えたり、他のriskやholdを一括解除したりしない。

`underclaim`、`overclaim`、`baseline unknown`、finding、holdは、それぞれ異なるreview状態である。8件のriskを8件の実装欠陥と読み替えず、1件のunderclaimを性能実証と読み替えない。patch提案を実行せず、未定義のpolicyやlicense、support promiseを補ってholdを消さない。

### 中間snapshotに対する追跡境界（履歴）

このcfe候補の観測時点では、最終source export・同じscope/profileのqueryとlinter・対応するsource-session digestの固定は後続作業として必要だった。現行の固定alpha.3観測は上段に分離して記録している。sourceやscope、digestが変われば新しい観測が必要であり、この古い候補のcoverage、finding数、claim draft状態を最終版へ流用しない。commit/push/merge、release、package、host activationはRepoSeiriの出力だけから許可・完了にならない。

### EPI-05の履歴：6.3.2-alpha.1

以下は初回clone時の履歴であり、現在のsourceへの再監査結果ではない。RepoSeiri `1.1.0+codex.20260823031336`の`common` profileで、当時のEpistesys cloneをrepository scopeとして監査した。queryとlinterはそれぞれ同一内容のsource snapshotへ実行し、writes filesはfalseだった。

#### 当時の観測

- Entries: 183、document events: 3,107、document diagnostics: 0。
- Documents: 29 selected / 29 candidates、selected bytes: 288,128。
- Markdown coverage: Complete。conflict coverage: Complete。
- Repository observations: 30 present、43 absent、1 unknown、0 conflict。
- Root README grammar nodes: 22。profile fit score: 40/100（top profile `ml`）。これは文書品質や機能品質ではなく、RepoSeiriのprofile適合観測である。
- Wording linter: 21 files、3 generated surfaces、0 findings。
- Patches: writes files false。1 edit-existing preview、6 skeleton review items、5 manual decisions、11 held items。patch suggestionは適用していない。

#### 履歴の解釈境界

`absent`、`unknown`、hold、profile fit score、claim draft stateは、機能の欠落、正しさ、公開準備、security、license、support promiseを自動的に意味しない。当時は新規private repositoryの入口、support、intake、contributing、security、release、lifecycle、governance、license、automation、ownership targetが未定義だったため、RepoSeiriのpatch holdを保持した。これは当時の状態であり、現在の各targetが未定義であると再判定した記録ではない。方針や文書を発明してholdを消さない。

日英の入口はREADME、docs/README、migration contract、adoption manifest、clone verification、known limitations、Assurance boundary、implementation statusへ接続されている。継承元の長文文書は歴史記録として扱い、Epistesysの新機能や外部保証へ昇格させない。

## English

### Documentation audit of the fixed alpha.3 snapshot

On 2026-10-27 JST, RepoSeiri `1.1.0+codex.20260823031336` ran its Rust core through the normal bundle launcher. The target was a newly expanded, fixed tracked-source export of Git commit `323c0b1b266c13156dd2cd1a4ba2a56c6b6b9142`, using profile `common` and scope `subtree`. A new export from `git -c core.autocrlf=false -c core.eol=lf archive` matched raw Git blob object IDs for all 320 regular files, with no extra/missing files. No caches, weights, private keys, receipt roots, or personal host state were imported. Git archives omit Git history metadata; unobserved temporal activity is not a maintenance judgment.

`summary`, `linter`, `governance`, and `pr-body` all exited 0. Before/after summaries returned the same source-session digest, `sha256:821f4049acd66186f1afc363cb5b8051bd1828ba22d11a470644ad610c8f9cca`, with identical entry/document/selected-byte counts. This version's `linter`/`governance`/`pr-body` wire does not return a native session-digest field; matching digest fields inside all wire outputs are therefore not claimed. This record retains input binding through the fixed Git export and surrounding canonical summaries, without inventing a field or promoting it to native attestation.

| Observation | Fixed alpha.3 snapshot result |
| --- | --- |
| Entries / document events / document diagnostics | 397 / 8,579 / 0 |
| Documents | 67 selected / 67 candidates; 962,100 bytes |
| Markdown / conflict coverage | `Complete` / `Complete` |
| Coverage scopes | 79 complete, 0 partial, 1 not requested, 0 limit exceeded |
| Repository observations | 52 present, 25 absent, 1 unknown, 0 conflict, 0 limit exceeded |
| README grammar / program capability nodes | 139 / 2,727; program unknown reason remains `unsupported_syntax` |
| Summary claims / findings | 11 / 4 |
| Underclaim / overclaim / baseline unknown | 1 / 8 / 8; 0 claim drafts |
| Patch operations / holds | 2 / 7; `writes_files=false` |
| Wording linter | 41 files, 3 generated surfaces, 4 findings, 15 suppressed boundary exceptions |
| Governance obligations / relations | 8 / 884 |
| Governance conflicts / proposition-conflict candidates | 0 / 458; every candidate has `kind=version` |
| README translation alignment | 0 alignments; retain `unsupported_syntax` |
| Target reachability | 20 repository-local present, 0 local missing, 1 non-local or unknown |
| Lifecycle / temporal activity | `unsafe_to_invent` / `NotRequested` |
| PR body | 1 draft generated; no new GitHub PR created or posted |

#### Reproducing input/output binding

The export-manifest SHA-256 is `271542e925fbde31bd7c574b8baaffa7bbb28df60ee7a6865935641c9052612a`; the Git-archive ZIP SHA-256 is `6e0a63e92a5519ef19db79fb74563c23b17fa70b65466764c3ba03ada84f2962`. The manifest encodes `[{path,bytes,sha256}]` in Git-tree order as compact UTF-8 JSON without a BOM. Git blob-ID comparison identifies Git objects, not safety or semantic correctness.

Each query used the same normal bundle launcher with `-Path <fixed-export> -Profile common -Scope subtree -Query <query> -Format json`. Hashes below cover captured native-JSON lines joined by LF as UTF-8 text, without BOM/trailing LF; they are neither original-stdio byte hashes including its line endings nor source-session digests. Durations are local invocation observations, not general latency benchmarks.

| Query | Exit | Elapsed ms | Output text SHA-256 |
| --- | --- | ---: | --- |
| summary (before) | 0 | 833 | `cf61b0e1a8a75b02590cc9a8ca1504640e7aaee7502d2166924f2266a787ac11` |
| linter | 0 | 752 | `69643ce8d878c8f74da76aa68055b59c7530b19e44e425f71edca5b58c213cf0` |
| governance | 0 | 1349 | `85c7c1ba268ce0b0511e5655df38949bfb9dfc1d2d1a5a5d895675d4a059b1a2` |
| pr-body | 0 | 748 | `e8bb2e2c40aaa1ef2f419e13d926d3fa82bd8b26cd4e1da3ed4ef7fbbb215e6e` |
| summary (after) | 0 | 718 | `cf61b0e1a8a75b02590cc9a8ca1504640e7aaee7502d2166924f2266a787ac11` |

An earlier materialized export included CRLF conversion from host `core.autocrlf`, with summary digest `sha256:85eb89465dbf903cf0982cde2a666325aa9d197352e473c19769602450da993f` and 971,166 selected source bytes. Preserve it as history of a different byte profile from the same commit, not as the final raw-Git-blob export result. The old export was neither deleted nor overwritten; the new LF export with all 320 files checked became the final input.

#### Manual interpretation and unresolved states

The four linter findings are two negated-boundary descriptions at lines 137/139 of `docs/system-architecture.md` and two historical descriptions of those findings at line 75 of `docs/reposeiri-audit.md` in the fixed snapshot. Manual contextual review does not interpret them as affirmative guarantees. The machine result remains four findings: the earlier two are not reused as current results, nor are the current four rewritten as zero.

Repository-observation `conflict=0` differs from the 458 governance proposition-conflict candidates. The latter include historical/current versions, abbreviated names, other tools' versions, and adherence figures extracted into the same `project_version` key; examples compare `6.3.2-alpha.3` with `6.3.1`, `alpha.2-specific`, or `74.38`. These are neither automatically 458 implementation defects nor wholesale-dismissed false positives. Retain candidate-only review states requiring scope inspection.

Automatic README translation alignment remains `unsupported_syntax`, not machine certification of bilingual equivalence. Separate manual semantic/claim-level comparison from the independent syntactic/metric-token check over 17 bilingual documents and 320 links. Program unknowns, 7 holds, and 8 baseline unknowns do not become research-performance, publication-readiness, or policy success. No policy, licensing, or support promises were invented to remove holds.

#### Causal boundary of this record

This audit section is itself a documentation-only addition after the named commit was audited; it is absent from that snapshot. Counts/digests remain pinned to `323c0b1b266c13156dd2cd1a4ba2a56c6b6b9142`, not presented as a re-audit of the later tree including this record. Substantive later changes to README, source, or operating contracts require a new export/query. Source readiness, commit/push/merge, plugin installation, host activation, and research effects each require separate judgments/authority/observations; RepoSeiri output does not automatically establish them. Preserve the cfe candidate and EPI-05 numbers below as history.

### EPR candidate observation: not the final source

On 2026-10-27 JST, RepoSeiri `1.1.0+codex.20260823031336` ran its Rust core through the bundle launcher over a fixed source-only export using the `common` profile and **subtree scope**. The export source commit was `cfe29f89403f44ed1cd55552cf652c7c11cb3425`; the summary source-session digest was `sha256:4cd84e782284363ae1ec1b98b34f7950c9bc3cd827f5d5ae913a28611524a9d3`. This working export inherits a parent repository, so its subtree scope is not silently compared as the same scope as the historical repository-scope audit.

The `summary` and `linter` queries ran over the same fixed export without writes. This is an intermediate snapshot examining the next candidate from `6.3.2-alpha.2`, not certification of final alpha.3 source, release, or publication readiness. Do not transfer the following numbers unchanged to later fixes or updates to this document.

| Observation | Fixed-candidate result |
| --- | --- |
| Entries / document events / document diagnostics | 389 / 8,064 / 0 |
| Documents | 64 selected / 64 candidates; 918,038 bytes |
| Markdown / conflict coverage | `Complete` / `Complete` |
| Limit-exceeded observations / scopes | 0 / 0 |
| Repository observations | 52 present, 25 absent, 1 unknown, 0 conflict |
| Root README grammar nodes | 136 |
| Profile fit | 70/100, top profile `cli`; not a quality or performance score |
| Summary | 1 underclaim opportunity, 8 overclaim risks, 4 findings |
| Claim drafts / baseline unknown | 0 drafts, 8 baseline unknown |
| Patch operations / holds | 2 / 7; `writes_files=false` |
| Wording linter | 40 files, 3 generated surfaces, 2 findings, 10 suppressed boundary exceptions |

The two linter findings matched `guaranteed meaning preservation` and `correctness guarantee` in the “Not equivalent to” table column at lines 137 and 139 of `docs/system-architecture.md` in the fixed export. Both are negated claim-boundary descriptions; manual contextual review did not interpret them as affirmative guarantees. The machine result remains two findings, however: it was not rewritten as zero, nor used to dismiss other risks or holds wholesale.

`underclaim`, `overclaim`, `baseline unknown`, findings, and holds are distinct review states. Eight risks do not become eight implementation defects, and one underclaim opportunity does not establish performance. No patch suggestion was executed, and undefined policy, licensing, or support promises were not invented to remove holds.

### Historical follow-up boundary for the intermediate snapshot

At the time of this cfe-candidate observation, a final source export, queries/linter with the same scope/profile, and the corresponding pinned source-session digest were required follow-up work. The current fixed-alpha.3 observations are recorded separately above. Changed source, scope, or digest requires new observations; do not transfer this older candidate's coverage, finding counts, or claim-draft states to final-version results. RepoSeiri output alone does not authorize or complete commit/push/merge, release, packaging, or host activation.

### EPI-05 history: 6.3.2-alpha.1

The following is initial-clone history, not a re-audit of current source. RepoSeiri `1.1.0+codex.20260823031336` audited the Epistesys clone at that time at repository scope with the `common` profile. The query and linter ran over the same content snapshot; `writes files` was false.

#### Historical observations

- Entries: 183; document events: 3,107; document diagnostics: 0.
- Documents: 29 selected / 29 candidates; selected bytes: 288,128.
- Markdown coverage: Complete. Conflict coverage: Complete.
- Repository observations: 30 present, 43 absent, 1 unknown, 0 conflict.
- Root README grammar nodes: 22. Profile fit score: 40/100 (top profile `ml`). This is a RepoSeiri profile-fit observation, not document or feature quality.
- Wording linter: 21 files, 3 generated surfaces, 0 findings.
- Patches: writes files false. One edit-existing preview, six skeleton review items, five manual decisions, and eleven held items; no patch suggestion was applied.

#### Historical interpretation boundary

`absent`, `unknown`, holds, profile fit, and claim-draft state do not automatically mean missing functionality, correctness, publication readiness, security, licensing, or support promises. At that time, RepoSeiri holds were retained because new-private-repository targets for entry, support, intake, contributing, security, release, lifecycle, governance, license, automation, and ownership were undefined. This describes the historical state; it is not a renewed judgment that each current target is undefined. Do not invent policy or documents to remove the holds.

The bilingual entry points connect README, docs/README, migration contract, adoption manifest, clone verification, known limitations, Assurance boundary, and implementation status. Long inherited documents remain historical records and do not promote into new Epistesys features or external guarantees.
