# Beta-7.0検証記録 / Beta-7.0 verification record

## 日本語

この記録は、既存試験の一巡と実失敗の修復、および利用者が指定したBeta-7.0の配布・plugin反映を区別します。履歴の成功を新identityへ付け替えません。

| 対象 | 観測 |
| --- | --- |
| 修復snapshotの通常integration runner | Windows・no-default-featuresで430／430を一巡。最終照合で未修復の観測FAILなし |
| 固定inventory | 438 target。OS・feature除外8は未実行。実行coverage98.17%、PASS率ではありません |
| 通常library unit群 | 別枠で16／16を一巡 |
| 修復snapshotのworkspace・test型検査 | 実Cargo終了コード0、2026-11-06 01:38 UTC |
| 新identityの配布build | `7.0.0-beta.1`、Windows release・既定features、実Cargo終了コード0。2026-11-06 02:13 UTC、19分39秒 |
| binary起動・identity | 2026-11-06 02:16 UTCに終了コード0、version一致。19,275,776 bytes |
| 既定GPU機能 | compile済み。GPU実行の観測とは区別します |
| Git・plugin適用 | sourceは[PR #10](https://github.com/ViszCham/Epistesys/pull/10)で追跡します。登録・有効化・読み込みはhostごとの別観測です |

各中央batchは1,299個のRust／Cargo入力の前後hashを照合し、実行中の変化はありませんでした。rawの元失敗を保持し、後続の該当target結果で修復を確認しました。群別snapshotと影響再試験であり、全環境を固定した一つのhermetic実行ではありません。

配布binaryのSHA-256は`71031bb59cb785ecee8f4a1bb45f179665ce047c864bc5326c3dfd0df767dbbf`です。利用者の指示に従い、本体の機能試験、全OS／GPU matrix、全文再読、追加監査、CIの再一巡は省略しました。未実行のCIを合格とは記録していません。branch保護・必須checkの設定は変更していません。buildにはunused／dead-code等の警告が残っています。

別OS・外部backend・ignored case・実GPU・独立gold・一般効果・実host配信は、この表から成功と推論しません。詳細は[変更と確認範囲](../docs/epistesys-7.0.0-beta.1-release-notes-2026-11-06.md)をご覧ください。

## English

This record distinguishes the existing-test pass and actual-failure repairs from the requested Beta-7.0 distribution/plugin rollout. Historical successes are not relabeled under a new identity.

| Target | Observation |
| --- | --- |
| Repaired-snapshot normal integration runners | Windows no-default-features: 430/430 completed; no unrepaired observed FAIL in final reconciliation |
| Fixed inventory | 438 targets; 8 OS/feature exclusions unexecuted; 98.17% execution coverage, not a PASS rate |
| Normal library-unit groups | Separately, 16/16 completed |
| Repaired-snapshot workspace/test typecheck | Actual Cargo exit 0 at 2026-11-06 01:38 UTC |
| New-identity distribution build | `7.0.0-beta.1`, Windows release/default features, actual Cargo exit 0 at 2026-11-06 02:13 UTC; 19m39s |
| Binary launch/identity | Exit 0 and matching version at 2026-11-06 02:16 UTC; 19,275,776 bytes |
| Default GPU features | Compiled; distinct from observed GPU execution |
| Git/plugin rollout | Source is tracked in [PR #10](https://github.com/ViszCham/Epistesys/pull/10). Registration, enablement and pickup are separate per-host observations |

Each central batch matched before/after hashes for 1,299 Rust/Cargo inputs with no drift during execution. Original failed raw outputs remain; later matching target outcomes establish repairs. These are per-group snapshots and affected reruns, not one hermetic run over the whole environment.

The distribution binary SHA-256 is `71031bb59cb785ecee8f4a1bb45f179665ce047c864bc5326c3dfd0df767dbbf`. Per user instruction, functional suites, the full OS/GPU matrix, full rereads, additional audits and another CI pass were omitted. Unexecuted CI is not recorded as passing. Branch protection and required-check settings were not changed. Unused/dead-code and related build warnings remain.

Do not infer success for another OS, external backends, ignored cases, real GPU execution, independent gold, general effectiveness or real-host delivery from this table. See [changes and verification scope](../docs/epistesys-7.0.0-beta.1-release-notes-2026-11-06.md).
