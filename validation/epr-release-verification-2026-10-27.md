# EPR alpha.3 source-release verification

## 日本語

### 状態と対象

**宣言standalone profileのfunctional gateは完了。最終source release readinessはRepoSeiriの固定export監査を確認中。** source候補は `20084b43fd66d5b619047afee4da2fdca07397b2`、identityは `6.3.2-alpha.3`。基線は `75500ed7a33a009f5e1ce813862dc0a7986372c9`、作業branchは `codex/epr-alpha3-20261027`。

[機械可読な最終記録](epr-alpha3-release-readiness-2026-10-27.json)、[231 source入力のinventory](epr-alpha3-source-input-inventory-2026-10-27.json)、[個別欠陥台帳](epr-remediation-ledger.v1.json)を分離する。source／設定／fixture入力は検証後にも231/231のbyte一致を確認した。これは選択したWindows working-tree入力のcurrentnessであり、Git blob／他OSの改行形式、完全な外部依存・build worldの同一性ではない。

### alpha.3 identityでの最終gate

| Gate | 観測 |
| --- | --- |
| Windows local | format、check、no-default、default、strict clippy `-D warnings`、locked releaseを含む13 commandがすべてexit0 |
| Windows test summary | no-default 409 passed／0 failed／1 ignored、default 409／0／2。各85 summary group |
| Windows／Ubuntu CI | [run 40461969877](https://github.com/ViszCham/Epistesys/actions/runs/40461969877)、両job・全required step成功、skipなし。Windows14分08秒、Ubuntu6分57秒 |
| Ubuntu test summary | no-default 416 passed／0 failed／1 ignored、default 416／0／2。各85 summary group |
| release parser／numeric／hash | parser3、CPU numeric5、SHA-256標準・mutation vectorのtest1成功 |
| configured日英backend | pinしたStanzaの実positive成功。command64.426秒、test26.46秒。一般意味精度の実証ではない |
| 実GPU | RTX 3060／Vulkan。2,048／2／2／8／65,536 rows、native completionとsummary／mask／finalist／decision parity成功。command17.133秒、test2.08秒 |
| 実Media | 所有する合成speech／videoからdecode、OpenCV、非空ASR候補を観測。CPU、faster-whisper1.2.1。command11.561秒、test10.10秒 |
| schema／consumer | unit6成功、新規alpha.3 capture18 JSONから27 checks成功。6種類を両feature系で実行した12 mutationを拒否 |
| fresh package | 固定 `20084b4` の正規full helperがexit0／`built_and_tested=true`。release buildと実CLI修復・再開6/6成功 |
| package inventory | 216 filesをbuild後に全SHA-256再照合。members digest `sha256:984a03af1e5c070c47e9dc1c2fd7788b89f31db540286d1f74c6abeee08168b1` |
| identity／source launcher | workspace13 crate・plugin source manifest・source skill3件を整合。checkout／fresh packageの実CLIと、別cwdからの通常source launcherがalpha.3を返すことを確認 |
| 補助監査 | Assurance選択6単位の現在byte一致。5件 `blocked_with_diagnostics`、1件 `needs_evidence`、全件 `missing_or_unknown` |
| RepoSeiri／文書 | 日英・リンク検査済み。最終fixed exportの同session query／linter／governanceは確認中 |

summary groupを足した数であり、独立課題数や意味正答率ではない。ignoredなdevice／model testをportable CI成功へ混ぜず、GPU／Stanza／Mediaを明示的に別実行した。GPUは全vector一致・occupancy・saturation・speedupを、ASRは精度・license監査・全vision familyのpositiveを実証しない。

packageは**CLI-source検証用のsource-only package**で、全skill／hookを含むplugin install archiveではない。binary、weight、trust rootを同梱しない。通常source launcherのTL `Clarify`／shadow-onlyは診断状態の保持であり、使用許可や意味正確性の認証ではない。最初のlauncher観測はstdoutとstderrの混合をJSONとして解析してprobe側が失敗したため、streamを分離して再確認した。MSVC linker診断を消したりsourceを変更したりしていない。

### 昇格順序と失敗履歴

`91009e6` の昇格前13 command、両OS CI、正規full package成功後だけalpha.3 identityへ更新し、上表を再実行した。[昇格前観測](epr-prepromotion-observations-2026-10-27.json)に初回gate・途中失敗・後続成功を保持する。

- Rust-only負例は[PR #6](https://github.com/ViszCham/Epistesys/pull/6)でWindows／Linuxとも予定通り失敗し、unmergedでclosed。負例を成功へ読み替えない。
- Windows CIの500ms cleanup正例はowned tree停止後のpipe cleanupを確認できなかった。製品のUnknown分岐を変えず、test-only競合分離と所有native helperの1,500ms正例／3秒wall上限へ修正。全assert、500ms descendant負例、期限切れ／admission負例を維持し、16 harness threads×10回で70/70成功した。
- 中間PowerShell fixtureは.NET初期化でsleep前に失敗し採用しなかった。製品の環境allowlistを拡張しない。
- 初回固定packageの `ProcessFailed`／`Hold` は根因未特定。同じpackageの再実行成功や、後続 `91009e6`／alpha.3の正規full-helper成功と分け、MAX_PATH等の原因を推測で確定しない。policy fixtureのGit `Filename too long` は別の直接観測で、process-local `core.longpaths=true` による再実行を記録した。global設定は不変。

### 監査と主張の境界

[監査A](epr-adversarial-audit-a-2026-10-27.md)／[B](epr-adversarial-audit-b-2026-10-27.md)は別担当のチーム内レビュー、[最終差分点検](epr-final-delta-review-2026-10-27.md)は統合担当の自己検証を含む。独立第三者監査ではない。[Assurance記録](epr-assurance-observations-2026-10-27.json)はdoctor一回と16KiB以下の完全単位に限定し、大きいprocess／coordinatorを分割して監査済みとしない。

既知priority findingの修正と宣言profileの接続・回復検証は、未知欠陥ゼロ、形式証明、一般Coding／意味性能、hallucination削減、長期保持、OS-wide sandbox、完全build world、実host送信の実証ではない。[残存条件](epr-residual-risk-ledger.v1.json)を保持する。研究評価は `PendingNoCorpus`、実hostは `PendingHostObservation`。本作業はplugin再インストール・host activationを行わず、既存の必須installed facadeはalpha.2のまま。source alpha.3と混同しない。

## English

### Status and scope

**Functional gates for the declared standalone profile are complete; final source-release readiness awaits the fixed-export RepoSeiri audit.** Source candidate: `20084b43fd66d5b619047afee4da2fdca07397b2`, identity `6.3.2-alpha.3`. Baseline: `75500ed7a33a009f5e1ce813862dc0a7986372c9`; branch: `codex/epr-alpha3-20261027`.

Separate the [machine-readable final record](epr-alpha3-release-readiness-2026-10-27.json), [231-input source inventory](epr-alpha3-source-input-inventory-2026-10-27.json), and [individual-finding ledger](epr-remediation-ledger.v1.json). All 231 source/configuration/fixture byte identities were rechecked after validation. This establishes currentness for selected Windows working-tree inputs, not Git-blob/cross-OS newline equality or complete external-dependency/build-world identity.

### Final alpha.3-identity gates

| Gate | Observation |
| --- | --- |
| Local Windows | All 13 commands exit0, including format/check/no-default/default, strict clippy `-D warnings`, and locked release |
| Windows test summaries | No-default 409 passed/0 failed/1 ignored; default 409/0/2; 85 summary groups each |
| Windows/Ubuntu CI | [Run 40461969877](https://github.com/ViszCham/Epistesys/actions/runs/40461969877): both jobs/all required steps success, none skipped; Windows14m08s, Ubuntu6m57s |
| Ubuntu test summaries | No-default 416 passed/0 failed/1 ignored; default 416/0/2; 85 summary groups each |
| Release parser/numeric/hash | Three parser tests, five CPU numeric tests, one standard/mutated SHA-256-vector test passed |
| Configured Japanese/English workers | Pinned-Stanza actual positive passed; command64.426s, test26.46s; not general semantic-accuracy evidence |
| Actual GPU | RTX 3060/Vulkan, 2,048/2/2/8/65,536 rows; native completion and summary/mask/finalist/decision parity passed; command17.133s, test2.08s |
| Actual Media | Owned synthetic speech/video produced decoder/OpenCV/nonempty ASR candidates; CPU/faster-whisper1.2.1; command11.561s, test10.10s |
| Schema/consumers | Six unit tests; 27 checks over 18 fresh alpha.3 JSON captures; twelve executions of six mutation classes across both feature profiles rejected |
| Fresh package | Normal full helper for fixed `20084b4` exit0/`built_and_tested=true`; release build plus six real CLI repair/resume cases passed |
| Package inventory | All 216 file SHA-256 values rechecked after build; members digest `sha256:984a03af1e5c070c47e9dc1c2fd7788b89f31db540286d1f74c6abeee08168b1` |
| Identity/source launcher | Thirteen workspace crates, source plugin manifest and three source skills aligned; checkout/package CLIs and the normal launcher from another cwd returned alpha.3 |
| Advisory review | Six current Assurance input hashes match; five `blocked_with_diagnostics`, one `needs_evidence`, all `missing_or_unknown` |
| RepoSeiri/documents | Bilingual/link checks passed; final same-session fixed-export query/linter/governance pending |

Counts aggregate test-summary groups, not independent tasks or semantic accuracy. Ignored device/model tests were not folded into portable-CI success; GPU/Stanza/Media were executed separately. GPU observations do not establish full-vector equality, occupancy, saturation, or speedup; ASR establishes neither accuracy/license review nor positive coverage of every vision family.

The package is a **source-only CLI verification package**, not a complete plugin-install archive containing all skills/hooks. It excludes binaries, weights and trust roots. The normal launcher's shadow-only TL `Clarify` retains diagnostic state, not permission or semantic certification. An initial probe mixed stdout/stderr and failed JSON parsing in the probe harness; separate streams were rechecked without suppressing MSVC linker diagnostics or changing source.

### Promotion order and failure history

Promote to alpha.3 identity only after `91009e6` passed all 13 prepromotion commands, both OS CI jobs, and the normal full package helper; then rerun the table above. Preserve initial gates, intermediate failures and later passes in [prepromotion observations](epr-prepromotion-observations-2026-10-27.json).

- The deliberate Rust-only negative in [PR #6](https://github.com/ViszCham/Epistesys/pull/6) failed as intended on Windows/Linux and was closed unmerged; never relabel it success.
- Windows CI's 500ms cleanup positive could not confirm pipe cleanup after owned-tree termination. Preserve the product Unknown branch; isolate test contention and use an owned native helper with a 1,500ms positive budget/3s wall ceiling. Retain every assertion, the 500ms descendant negative, expiry/admission negatives; 70/70 cases passed across ten rounds at16 harness threads.
- An intermediate PowerShell fixture failed during .NET initialization before sleep and was not adopted; no widening of product environment allowlists.
- Initial fixed-package `ProcessFailed`/`Hold` causes remain unresolved. Separate them from same-package retries and later passing normal `91009e6`/alpha.3 full helpers; do not infer MAX_PATH or another proven cause. The policy fixture's explicit Git `Filename too long` was a different observation; record process-local `core.longpaths=true` retries, leaving global configuration unchanged.

### Reviews and claim boundaries

[Audits A](epr-adversarial-audit-a-2026-10-27.md)/[B](epr-adversarial-audit-b-2026-10-27.md) are different-contributor team reviews; the [final delta review](epr-final-delta-review-2026-10-27.md) includes integrator self-verification. They are not independent third-party assurance. The [Assurance record](epr-assurance-observations-2026-10-27.json) retains one doctor observation and complete units no larger than16KiB; larger process/coordinator units are not fragmented into claimed coverage.

Known-priority remediation and declared-profile connection/recovery validation do not establish zero unknown defects, formal proof, general Coding/semantic performance, hallucination containment, long-session retention, OS-wide sandboxing, complete build worlds, or real-host delivery. Preserve [residual conditions](epr-residual-risk-ledger.v1.json). Research remains `PendingNoCorpus`, real hosts `PendingHostObservation`. This work does not reinstall/activate the plugin; mandatory installed routing remains alpha.2, separate from source alpha.3.
