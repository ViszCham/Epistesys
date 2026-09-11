# Epistesys documentation

## 日本語

Epistesys 6.3.2-alpha.1の文書は、clone契約、採否、実行検証、証拠境界、既知の制約を入口から追跡できるように整理します。説明文は日本語を前半、対応する英語を後半に置きます。

### 実装能力の入口

Epistesysはv6.3.1のsource cloneとして、receipt検証、TL/TLDG、256×8 world budget、RPA-00〜39、candidate-only Media、Host replay v2を実行可能な形で継承しています。build/test/clippy/CLIの検証記録は、単なる計画ではなくcloneで観測した実行結果です。外部receiptやhost pickupが未成立でも、これらのsource経路を未実装とは扱いません。

### 最初に読む文書

- [README](../README.md)：Epistesysの位置付けと起動方法
- [Migration contract](migration-contract.md)：何を継承し何を除外するか
- [Adoption manifest](adoption-manifest.md)：allowlistと非開示情報の検査方針
- [Clone verification](clone-verification.md)：build・test・clippy・CLIの観測
- [Assurance-Compiler status](assurance-compiler-status.md)：16KiB以下Rust単位の補助監査
- [Known limitations](known-limitations.md)：未観測、Unavailable、Hold、Clarify、fallback
- [RepoSeiri audit](reposeiri-audit.md)：repository scopeの構成・文言・hold記録
- [Initial clone release](initial-clone-release-record.md)：initial commitとmain bootstrap

### 継承元記録の扱い

同じdirectoryのv6.3.1設計・監査文書は、実装の由来と歴史的状態を保存するためのものです。そこにある旧line名、旧package identity、旧検証結果は、Epistesys 6.3.2-alpha.1の現在状態へ自動昇格しません。現在のクローンidentityと検証は、root README、clone verification、known limitationsを基準にします。

### 主張境界

local test、低loss、schemaの存在、GPU実行、source hash、Assurance statusは、それぞれの観測範囲を超えて正しさ、一般性能、形式証明、host activation、権限、release approvalを生成しません。未観測範囲を削除せず、次の検証条件と一緒に記録します。

## English

Epistesys 6.3.2-alpha.1 documentation is organized so that clone contract, disposition, execution validation, evidence boundaries, and known limitations can be followed from the entry point. Explanatory prose places Japanese first and equivalent English second.

### Entry point for implemented capabilities

As a v6.3.1 source clone, Epistesys inherits executable surfaces for receipt verification, TL/TLDG, the 256×8 world budget, RPA-00 through RPA-39, candidate-only Media, and Host replay v2. Build, test, clippy, and CLI records are observed executions in the clone rather than plans. Missing external receipts or host pickup do not make these source paths nonexistent.

### Read first

- [README](../README.md): Epistesys position and startup
- [Migration contract](migration-contract.md): what is inherited and excluded
- [Adoption manifest](adoption-manifest.md): allowlist and non-disclosable-information policy
- [Clone verification](clone-verification.md): build, test, clippy, and CLI observations
- [Assurance-Compiler status](assurance-compiler-status.md): bounded review of Rust units no larger than 16 KiB
- [Known limitations](known-limitations.md): unobserved, Unavailable, Hold, Clarify, and fallback states
- [RepoSeiri audit](reposeiri-audit.md): repository-scope structure, wording, and hold record
- [Initial clone release](initial-clone-release-record.md): initial commit and main bootstrap

### Treatment of inherited records

The v6.3.1 design and audit documents in this directory preserve implementation provenance and historical states. Their older line names, package identities, and validation results do not automatically promote into the current Epistesys 6.3.2-alpha.1 state. The root README, clone verification, and known limitations define the current clone identity and evidence.

### Claim boundary

Local tests, low loss, schema presence, GPU execution, source hashes, and Assurance status do not create correctness, general performance, formal proof, host activation, authority, or release approval beyond their observed scopes. Retain unobserved ranges with the conditions required for their next verification.
