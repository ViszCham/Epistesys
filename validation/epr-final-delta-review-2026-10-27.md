# EPR final delta review

## 日本語

### 対象と独立性

対象は`20084b43fd66d5b619047afee4da2fdca07397b2`のalpha.3 source候補。これは監査A／B後の追加差分に対する統合担当の再点検であり、独立第三者監査ではない。先行する別担当レビュー、修正後の自己検証、Assuranceの静的surface、OS／backendの実行結果を区別する。最終gateの状態は[検証記録](epr-release-verification-2026-10-27.md)で判定する。

### 差分の批評

| 項目 | 点検した契約と反例 | 結論の範囲 |
| --- | --- | --- |
| C1/C5：rootと入力identity | existing canonical directory、strict UTF-8、Unixのliteral backslash拒否、同名keyのvacant-entry挿入。lexical aliasやlossy変換で別入力を合成しない | 宣言したsource-only入力profile。任意build script／外部include入力の完全閉包ではない |
| C3：修復時刻 | driverのlogical clockと、caller epoch＋monotonic elapsedを下限とするcoordinatorの3回のsample。古いdriver timestampから期限切れpermitを復活させない | shared clock、cross-second、backdating、genuine-future負例。外部clock／revocationの真正性はhost契約 |
| C4/C9：process正例 | test-only contention lock、所有するnative Rust単一root、1,500ms正例と3秒wall上限。500ms descendant負例・全cleanup assertion・製品のUnknown分岐は維持 | 16 harness threads×10回、70/70の対象stress。任意OS負荷でcleanup成功する保証ではない |
| Media catalog差分 | 監査Aのhashからの追加変更はtest内の17-family完全一致とmethod ID重複検査のみ | FaceDetectionとObjectDetectionを分離。familyの精度・話者感情・identityの独立検証へ昇格しない |
| C8：SHA-256 profile | dev/testのsha2 code generationだけを最適化。fresh executable hash、deadline、receipt期限は省略・延長しない | 標準vector、mutation負例、debug/release試験。暗号学的証明や一般性能改善の実証ではない |
| release identity | workspace13 crate、lock、plugin source manifest、source skill、日英文書のalpha.3 identity。外部依存lockは不変 | source候補の整合。cached plugin／host activationを意味しない |

外部PowerShellを用いた中間fixtureは.NET初期化でsleep前に失敗したため採用しなかった。環境allowlistや製品の成功条件を広げず、native helperへforward fixした。最初の固定packageでの`ProcessFailed`／`Hold`と、後続の成功は別々に保持する。その初回根因は未特定であり、単なる再実行からMAX_PATH等を確定しない。後続の`91009e6`正規full helperはexit0・CLI6/6を実観測した。

### source pin

以下は選択したWindows working-tree byteのSHA-256であり、Git blob／他OSの改行形式と自動的に同一視しない。

| Path | SHA-256 |
| --- | --- |
| `scripts/lc631/crates/lc631-analysis/src/process.rs` | `cad5940bfdaedd8f945a80b7774ad7213ead057f0a80f69efc031157628fccb1` |
| `scripts/lc631/crates/lc631-analysis/src/file_repair.rs` | `aa1defd76f3d4b5cf1462e92a349fdaf7a084e679c5a413f5d0d6837d0d861b7` |
| `scripts/lc631/crates/lc631-analysis/src/repair_loop.rs` | `5464401f263de0466b665423702675a9165c24bb8aa03ba7fc15654f3e688e29` |
| `scripts/lc631/crates/lc631-analysis/src/validation_producer.rs` | `381837b232324e9a7d4f7927e7ba5a89cd1ce00acc9876dccb4dd80fdd5655be` |
| `scripts/lc631/crates/lc631-process-supervisor/tests/bounded_runtime.rs` | `945e6e49c32d8f4fa5a7e92622a1763813a225eff3e424619a858251699f2d3c` |

Assuranceで選択した6完全単位は既存観測のbyteと一致し、5件の`blocked_with_diagnostics`・1件の`needs_evidence`、全件`missing_or_unknown`を保持する。21,911-byteのprocess単位や大きいcoordinatorは16KiB制限を超えるため、分割して監査済みとはしない。[231入力のinventory](epr-alpha3-source-input-inventory-2026-10-27.json)もtracked sourceの境界であり、hermetic buildやrepository-wide safetyの証明ではない。

## English

### Scope and independence

Scope: the alpha.3 source candidate at `20084b43fd66d5b619047afee4da2fdca07397b2`. This is the integrator's post-A/B delta review, not independent third-party assurance. Separate earlier different-contributor reviews, self-verification after fixes, Assurance static surfaces, and actual OS/backend observations. The [verification record](epr-release-verification-2026-10-27.md) determines final gate status.

### Delta critique

| Item | Contract/counterexample examined | Conclusion boundary |
| --- | --- | --- |
| C1/C5: root/input identity | Existing canonical directories, strict UTF-8, rejection of literal Unix backslashes, and vacant-entry insertion; no merging through lexical aliases or lossy conversion | Declared source-only input profile, not complete arbitrary build-script/external-include closure |
| C3: repair time | Driver logical clock and three coordinator samples floored by caller epoch plus monotonic elapsed; stale adapter timestamps cannot revive expired permits | Shared-clock/cross-second/backdating/genuine-future controls; external clock/revocation authenticity remains a host contract |
| C4/C9: process positives | Test-only contention locks, an owned native single-root helper, 1,500ms positive budget and 3s wall ceiling; preserve the 500ms descendant negative, every cleanup assertion, and product Unknown branches | 70/70 targeted observations at 16 harness threads across ten rounds, not guaranteed cleanup under arbitrary OS load |
| Media catalog delta | Changes after audit A's pin are test-only exact 17-family coverage and method-ID uniqueness | Separate FaceDetection/ObjectDetection, without promoting family accuracy, emotion, or identity into independently validated observations |
| C8: SHA-256 profile | Optimize dev/test sha2 code generation only; never omit fresh executable hashes or extend deadlines/receipt validity | Standard vectors, mutation negatives, debug/release tests; neither cryptographic proof nor general performance evidence |
| Release identity | Alpha.3 across 13 workspace crates, lock, plugin source manifest, source skills and bilingual documents; external dependency lock unchanged | Source-candidate consistency, not cached-plugin installation or host activation |

An intermediate external-PowerShell fixture failed during .NET initialization before sleep and was not adopted. Forward-fix to the native helper without widening the product environment or success predicates. Preserve the first fixed-package `ProcessFailed`/`Hold` observations separately from later passes. Their initial cause remains unresolved; retries do not establish MAX_PATH or another cause. The later `91009e6` normal full helper was actually observed exit0 with six passing CLI cases.

### Source pins

The following pin selected Windows working-tree bytes; do not silently equate them with Git blobs or another OS's newline profile.

| Path | SHA-256 |
| --- | --- |
| `scripts/lc631/crates/lc631-analysis/src/process.rs` | `cad5940bfdaedd8f945a80b7774ad7213ead057f0a80f69efc031157628fccb1` |
| `scripts/lc631/crates/lc631-analysis/src/file_repair.rs` | `aa1defd76f3d4b5cf1462e92a349fdaf7a084e679c5a413f5d0d6837d0d861b7` |
| `scripts/lc631/crates/lc631-analysis/src/repair_loop.rs` | `5464401f263de0466b665423702675a9165c24bb8aa03ba7fc15654f3e688e29` |
| `scripts/lc631/crates/lc631-analysis/src/validation_producer.rs` | `381837b232324e9a7d4f7927e7ba5a89cd1ce00acc9876dccb4dd80fdd5655be` |
| `scripts/lc631/crates/lc631-process-supervisor/tests/bounded_runtime.rs` | `945e6e49c32d8f4fa5a7e92622a1763813a225eff3e424619a858251699f2d3c` |

The six complete Assurance selections still match their observed bytes: preserve five `blocked_with_diagnostics`, one `needs_evidence`, and `missing_or_unknown` throughout. The 21,911-byte process unit and larger coordinator exceed 16KiB and are not fragmented into a claimed audit. The [231-input inventory](epr-alpha3-source-input-inventory-2026-10-27.json) likewise describes tracked-source boundaries, not hermetic builds or repository-wide safety.
