# Beta-7.0の変更と確認範囲 / Beta-7.0 changes and verification scope

## 日本語

Epistesys Beta-7.0のsource identityは`7.0.0-beta.1`です。要求追跡・検証・修復を支援する開発ツールとして、TLCXの型付き文脈、複数の有限論理、EpiMapLoss、EpiRecompScore、資源管理、Coding・Media接続をまとめています。現在の数式は[全式と適用条件](epimaploss-epirecompscore-formulas.md)をご覧ください。今回の更新で式や互換wire IDは変更していません。

### 今回の修復

- Coding repair／resumeのCLI入口を、既存の型付きdispatcherへ接続しました。外部providerがない場合は、作用を起こさずHoldを返します。
- 修復・再開では、試行が消費した予算を新しいbudget objectから復活させないようにしました。freshな修復、checkpoint、別processでの再開と再選択を既存試験で確認しました。
- Clockの不可逆な失敗は保持し、再試行でproviderを呼ぶ前に拒否します。
- Mediaの登録configuration不一致は、spawnやreplay消費の前に実際のlaunch planへ照合します。従来のv2／v3 APIは保持しています。
- Reframeの旧fixtureを実際のexact-v2経路へ合わせ、型・owner・scope・Unknownの検査を維持しました。
- pluginのAGENTSとfacadeを整理しました。通常turnはDGCLを一度実行し、同じturnの同じ入力・binary・snapshotでは診断を参照します。詳細doctorは必要時だけ実行します。暗黙buildや永続prompt cacheは作りません。
- Stop hookのWindows起動をNoLogo／NoProfile／NonInteractiveに揃え、入力の読み直しやJSON再変換を行わないnative転送を維持しました。

### 検証と適用範囲

昇格前の修復snapshotでは、Windows・`--no-default-features`で実行対象となる430／430 integration target runnerと、別枠の16 library unit群を一巡しました。実際に検出した失敗は修復し、失敗対象と影響先だけを再試験しました。最後のworkspace／test型検査は終了コード0でした。

固定索引は438 targetです。別OS・別機能設定が必要な8 targetは未実行です。430／438＝98.17%はrunnerの実行coverageであり、PASS率や製品完成率ではありません。ignore-only runnerを含み、ignored caseや外部backendのopt-inは未実行のままです。昇格前の証跡を、version変更後の再実行結果とは呼びません。Beta identityのbuild・配布・plugin検証は[今回の検証記録](../validation/beta7-onepass-verification-2026-11-06.md)に分けて記録します。

Betaという名称は、全OS・GPU実行・外部backend・独立した正解データ・一般的な効果／性能・実host配信を検証したという意味ではありません。未観測やAssuranceのunknown／blockedは、そのまま残します。[既知の制約](known-limitations.md)と[plugin接続契約](plugin-host-integration.md)もご覧ください。

### 履歴と配布

Alpha.3は`main-alpha.3`、Beta 6.9は[保存記録](epistesys-6.9.0-beta.1-source-snapshot-2026-11-05.md)として保持しています。source、同梱binary、marketplace登録、cache install、有効化、既存chatでの読み込みは別々に確認します。公開sourceに鍵・model weight・個人設定・receipt／replay状態・build cacheは含めません。

## English

Epistesys Beta-7.0 uses source identity `7.0.0-beta.1`. This development tool brings together TLCX typed contexts, finite-logic profiles, EpiMapLoss, EpiRecompScore, resource ownership and Coding/Media connections. See the [complete equations and applicability contracts](epimaploss-epirecompscore-formulas.md). This update does not change formulas or compatible wire IDs.

### Repairs in this update

- Connected coding repair/resume CLI entries to the existing typed dispatcher. Missing external providers return effect-free Hold.
- Prevented a replacement budget object from restoring expenditure owned by a repair/resume attempt. Existing tests observed fresh repair, checkpoint, fresh-process resume and reselection.
- Retained irreversible Clock failures and rejected retries before provider callbacks.
- Compared Media registration configuration against the actual launch plan before spawn or replay consumption, preserving existing v2/v3 APIs.
- Aligned the obsolete Reframe fixture with the actual exact-v2 path while retaining type, owner, scope and Unknown checks.
- Simplified plugin AGENTS and facades. Ordinary turns invoke DGCL once; the diagnostic may be referenced within the same turn for unchanged input, binary and snapshot. Detailed doctors run only when relevant. No implicit build or persistent prompt cache is introduced.
- Standardized Windows Stop startup on NoLogo/NoProfile/NonInteractive while retaining native forwarding without rereading stdin or reserializing JSON.

### Verification and scope

The repaired pre-promotion snapshot completed 430/430 integration-target runners eligible under Windows `--no-default-features`, plus 16 separate library-unit groups. Actual failures were repaired and only failed/affected consumers were rerun. The final workspace/test typecheck exited 0.

The fixed inventory contains 438 targets. Eight requiring another OS or feature profile remain unexecuted. The 430/438 = 98.17% measures runner execution coverage, not a PASS rate or product completeness. It includes ignore-only runners; ignored cases and external-backend opt-ins remain unexecuted. Pre-promotion observations are not relabeled as post-version-change executions. Beta-identity build, packaging and plugin checks are recorded separately in the [verification record](../validation/beta7-onepass-verification-2026-11-06.md).

The Beta name does not establish all-OS, GPU execution, external-backend, independent-gold, general effectiveness/performance or real-host-delivery validation. Unobserved states and Assurance unknown/blocked remain explicit. See [known limitations](known-limitations.md) and the [plugin integration contract](plugin-host-integration.md).

### History and distribution

Alpha.3 remains on `main-alpha.3`; Beta 6.9 retains its [snapshot record](epistesys-6.9.0-beta.1-source-snapshot-2026-11-05.md). Source, packaged binaries, marketplace registration, cache installation, enablement and existing-chat pickup are checked separately. Public source excludes keys, model weights, personal settings, receipt/replay state and build caches.
