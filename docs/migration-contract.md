# Epistesys 6.3.2-alpha.1 migration contract

## 日本語

Epistesys（エピステシス）は、Labyrinth-Codex v6.3.1の実装挙動を継承する独立したクローンです（初期登録時はprivate）。初回版は`6.3.2-alpha.1`とし、新しいGit履歴で保存します。元の履歴、旧環境のcache、credentials、receipt root、replay ledger、個人設定は継承しません。

継承するのは、固定したv6.3.1 source revision、必要なRust workspace、test、fixture、schema、launcher、hook、skill、実装説明です。変更はEpistesysのidentity、独立起動に必要なpath、非開示情報の除外、ビルドと検証に必要な調整へ限定します。新しい推論機構やEpistesys-7の機能は初回cloneの範囲外です。

上流の固有プロジェクト名、内部関係、由来、非公開URL、個人用資料、個人用状態、非開示の設計情報は持ち込みません。名称の置換だけでは分離完了とせず、本文、リンク、図、JSON、コメント、fixture、埋込み文字列、metadata、binaryのsource検査を行います。権利表示とlicense表示は削除しません。

標語は次を継承します。

> **シードは仕様ではない。** \
> **シード以前からコミットまで、すべての射影に証跡を。** \
> Epistesysは、解釈・権限・証拠・計算・検証・出力コミットを、ひとつの境界付き制御経路として扱います。

初回cloneの完了は、機能全体の完成、一般性能、形式証明、host-level enforcement、外部backendの検証済み状態を意味しません。継承した未観測、Unavailable、Hold、Clarify、fallback、既知の制約は残します。commit、push、tag、plugin install、host reloadは、この契約から自動的には許可されません。

## English

Epistesys (Japanese: エピステシス) is an independent clone (initially registered as private) that inherits the implementation behavior of Labyrinth-Codex v6.3.1. The initial version is `6.3.2-alpha.1` and will be saved with a new Git history. Original history, old caches, credentials, receipt roots, replay ledgers, and personal settings are not inherited.

The inheritance set is the pinned v6.3.1 source revision, required Rust workspace, tests, fixtures, schemas, launchers, hooks, skills, and implementation notes. Changes are limited to Epistesys identity, paths required for independent startup, removal of non-disclosable information, and build or verification adjustments. New reasoning mechanisms and Epistesys-7 features are outside the initial clone.

Upstream-specific project names, relationships, provenance, private URLs, personal materials, personal state, and non-disclosable design information are not transferred. Renaming alone does not complete separation; inspect prose, links, diagrams, JSON, comments, fixtures, embedded strings, metadata, and binaries. Do not remove required rights or license notices.

The slogans are inherited:

> **Seed is not spec.** \
> **From pre-seed to commit, every projection leaves a witness.** \
> Epistesys governs interpretation, authority, evidence, computation, validation, and output commitment as one bounded control path.

Initial clone completion does not mean complete functionality, general performance, formal proof, host-level enforcement, or validated external backends. Retain inherited unobserved, Unavailable, Hold, Clarify, fallback, and known-limit states. Commit, push, tag, plugin installation, and host reload are not automatically authorized by this contract.
