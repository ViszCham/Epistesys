# TLCX-04 legacy preimage compatibility / 旧preimage互換台帳

## 日本語

### 契約・適用範囲

2026-10-29 JST。対象は6.3.2-alpha.3由来の旧JSON・receipt・replay境界である。TLCX-03の新binaryは`Epistesys/TLCX/FrameDigest/sha256/v1\0`という別domainを使う。本台帳の旧hash/MACへそのdomainを付けない。`sha256:`文字列の一致、再serialize、FrameDigestの再計算は認証ではない。

旧公開command・field・schemaの意味を変更しない。内部typed viewへの変換と、現在policyによる真正性検証を分離する。旧JSONを保持する必要があるconsumerは原bytesを保持し、typed viewからの再serializeだけで原bytes保存と称さない。compact JSONは下表の具体的なserializer/preimageであって、任意JSONの普遍的canonicalizationではない。

本sliceは旧writer/preimageを置換しない。新`wire::legacy_codec`は明示した旧versionのbounded入出力adapterであり、receipt検証を既存`ReceiptVerifier`へ渡す。replay entryのdigest一致は単一entryの非認証integrityだけであり、chain順序、trusted head、rollback、現在権限、exact host配信を確認しない。journal移行はTLCX-07、通常runtimeへの最終接続は34/40、実hostは35/43で別途再検証する。

### Preimage・消費先・移行方針

pathは`scripts/lc631/crates/lc631-<crate>/src/`からの相対表記。下表の既存familyは旧codecのまま保持する。新binaryのdigestを同じ型・同じfieldへ暗黙代入しない。

| Family / producer | 正確な旧encoding | 実consumer / 方針 |
| --- | --- | --- |
| receipt-kernel `canonical_claims` | schema, issuer, classの既存Rust Debug名, subject, scope, nonce, payload, issued, expiry, parentをNULで区切る。整数は十進、欠値は`none` | `ReceiptIssuer::issue`と`ReceiptVerifier::verify`のHMAC-SHA256。JSONへ変更しない |
| receipt-kernel `UntrustedReceipt::receipt_digest` | 上記claims text + NUL + 既存`sha256:`付きMAC text | verifier replay guard、parent receipt、各authority/evidence consumer。単なるdigestからVerifiedReceiptを生成しない |
| host `ReplayEntryBasisV1` | struct宣言順のcompact JSON。schema, sequence, previous, artifact, turn, candidate, output, callback, seed | `read_and_verify_ledger`→`parse_and_verify_ledger`のv1 branch。v2のauthenticity fieldを含めない |
| host `ReplayEntryBasisV2` | v1 field順の後にauthenticity receipt digestを追加したcompact JSON | appendと`read_and_verify_ledger`→`parse_and_verify_ledger`のv2 branch。entry自身のhashはpreimageに含めない |
| host seed / scope | 生user text、scope textのUTF-8 hashと、既存関数の明示format文字列 | `verified_host_seed`、seed attestation、permit。scope正規化を後付けしない |
| host output / callback | exact candidate/output/callback textのUTF-8 hashと、既存binding format | precommit/output verifier。trim、改行統一、Unicode正規化をしない |
| host `stop_hook` | session、turn identity、exact text、raw callback payloadの既存format/hash | post-hoc観測。precommit/sinkの代わりにしない |
| analysis `instruction_grammar::legacy_json_digest` | `serde_json::to_writer`がAST DTOを出すcompact JSON bytesをstream hash | `legacy_instruction_ast_digest`、AST→Program IR。String版とのbyte一致を維持 |
| analysis `dgcl_projection` | requirement/contractのcompact JSON、revision+NUL+content等の既存format | requirement realization、projection ledger。new AstIdやFrameDigestと別物 |
| analysis `completion_plan` | 既存translation/input structのcompact JSON | plan生成・入力digest再検査。field順と省略規則を固定 |
| analysis `validation_producer` | plan、観測tuple、lifecycle inputのcompact JSON、既存run format | validation producerとclaim consumer。旧authority/receiptの再利用を許可しない |
| analysis `closure_connection` / `connection_graph` | plan、expected JSON value、trace digest input、接続tupleのcompact JSON | connection/trace verifier。expected value自体を再定義しない |
| analysis `production_connection` | 登録connection spec tupleのcompact JSONおよびdriver bytes | production registration/source binding。登録digestだけでruntime観測にしない |
| analysis `closure_resume` | checkpoint bodyのcompact JSONと別digest、trusted-head DTO bytes | checkpoint readerとfresh resume検査。旧readerは保持、新journalは07 |
| analysis `evidence_lifecycle` | targets配列のcompact JSON、lease/intentの既存format | snapshot invalidation、evidence/authority再検査。保存形式で有効期限を復活させない |
| analysis `repair_loop` / `file_repair` | legacy replacement payload、rootを伴う既存NUL format、closure/claims JSON | intent/apply/validation/resume。旧no-rootとroot-bound branchを混ぜない |
| analysis `language_worker` | worker request JSON、観測tuple JSON、manifest/lock raw text | configured backend/source evidence。別request/versionへ流用しない |
| analysis `dgcl_gold` | corpus DTO/tupleのcompact JSON | gold registrationと評価。新binary保存はcorpus revisionの暗黙更新でない |
| analysis `pipeline` | directional lossの既存compact JSON | translation envelope payload binding。payloadのhashはsemantic truthでない |
| analysis source/tool families | `closure`、`rust`、`python`、`cargo_world`、`process`、`validation`、`release`のexact text/既存format | 個別source、argv、environment、cwd、tool、release consumer。新canonical helperへ一括置換しない |
| wire `alpha2_release` | 既存identity/gate format文字列 | alpha.2 gate verifier。source readinessとhost/general claimsを分離 |
| wire `WireEnvelope::to_pretty_json` | pretty JSON | 表示専用。署名/MAC preimageとして使わない |
| substrate codec / typed derivation / context | 各々の新domainと明示encoding | 新typed identityのみ。上記旧familyへ暗黙代入しない |

### Adapter・strict consumer

- adapterは`ReceiptV1`、`ReplayV1`、`ReplayV2`をcallerが明示選択し、本文のschemaと一致しない場合は拒否する。versionを推測して順次fallbackしない。
- 入力lengthの上限をparse前に検査する。caller上限は旧profileの上限以下を明示する。共通hard ceilingは16MiBであり、旧2MiB stdin等を拡張する許可ではない。これは既にメモリにあるbytesのadmissionで、ファイルread・deadline・aggregate RSSはTLCX-05の責務。
- receiptの既存`deny_unknown_fields`、schemaの`additionalProperties:false`を維持する。新fieldが必要なら新version/viewを選び、旧DTOへ押し込まない。新adapterのreplay DTOもstrictであり、従来host readerを緩めたり変更したりしない。
- decode/roundtrip/digest確認はuntrustedのまま。`VerifiedReceipt`を返す唯一のadapter経路は既存verifierへreceipt、現在policy、mutable replay guardを渡す経路。expiry、issuer、class、subject、scope、payload、parent、nonceの検査を省略しない。

### 独立vector・負例・未閉鎖

独立.NET HMAC-SHA256/SHA256計算の固定vector：key bytes `00..1f`、issuer `legacy-fixture`、class `HostSeed`、subject `sha256:subject`、scope `host/user-span`、nonce `000102030405060708090a0b0c0d0e0f`、payload `sha256:`+64個の`a`、issued10、expires20、parentなし。claims textは200 UTF-8 bytes。MAC=`sha256:e021688fd12425959b0f1dd04abdfe0ae392f7c6ef90dd739589b4276a819d65`、receipt digest=`sha256:8d2ad1400cf65fdf6481476f2851c628feec426291cc55a923b75bf931bbbeed`。このkeyは合成試験専用で、本番credentialではない。

必須負例はwrong key/domain、claimsの再hash、class spellingの変更、旧versionへの新field、V1/V2 preimage交差、expiry/scope/replay、duplicate JSON key、不正UTF-8、trailing JSON、上限超過。正例は固定MACと旧AST streaming bytesの一致、原bytes保持、legacy typed roundtripである。

この文書やdigest一致だけで全call siteの動的実行を観測したとはしない。各sliceのcurrent test receiptを出口台帳へ別記する。host G13、外部rollback anchor、TLCX-05資源broker、TLCX-07 journal移行、将来consumer接続は未観測のまま保持する。

---

## English

### Contract and scope

2026-10-29 JST. This covers legacy JSON, receipt and replay boundaries inherited from 6.3.2-alpha.3. TLCX-03 binary frames use the separate domain `Epistesys/TLCX/FrameDigest/sha256/v1\0`; never prefix legacy hashes/MACs with it. Equal `sha256:` strings, reserialization or recomputed FrameDigests are not authentication.

Preserve old public commands, fields and schema meanings. Separate conversion to typed internal views from current-policy authenticity checks. Consumers requiring original JSON bytes retain those bytes; reserializing typed data does not prove preservation of original bytes. Compact JSON below identifies a particular serializer/preimage, not a universal canonicalization of arbitrary JSON.

This slice does not replace legacy writers/preimages. New `wire::legacy_codec` is an explicitly versioned bounded legacy adapter that delegates receipt verification to the existing `ReceiptVerifier`. Replay-entry digest agreement is unauthenticated single-entry integrity only, not chain order, trusted-head, rollback, current-authority or exact-host-delivery verification. Journal migration belongs to TLCX-07; normal-runtime wiring is revalidated in 34/40 and actual hosts in 35/43.

### Preimages, consumers and migration policy

Paths are relative to `scripts/lc631/crates/lc631-<crate>/src/`. Preserve these existing families under their old codecs; never implicitly replace their fields/types with a new binary digest.

| Family / producer | Exact legacy encoding | Actual consumer / policy |
| --- | --- | --- |
| receipt-kernel `canonical_claims` | NUL-delimited schema, issuer, existing Rust Debug class name, subject, scope, nonce, payload, issued, expiry and parent; decimal integers and `none` for absence | HMAC-SHA256 in `ReceiptIssuer::issue` and `ReceiptVerifier::verify`; never change to JSON |
| receipt-kernel `UntrustedReceipt::receipt_digest` | Claims text above + NUL + existing `sha256:`-prefixed MAC text | Replay guard, parent receipts and authority/evidence consumers; no VerifiedReceipt from a digest alone |
| host `ReplayEntryBasisV1` | Compact JSON in declared struct order: schema, sequence, previous, artifact, turn, candidate, output, callback, seed | v1 branch of `read_and_verify_ledger` → `parse_and_verify_ledger`; exclude v2 authenticity field |
| host `ReplayEntryBasisV2` | Same v1 field order followed by authenticity receipt digest | Append and v2 `read_and_verify_ledger` → `parse_and_verify_ledger` replay verification; entry hash is not its own preimage |
| host seed / scope | UTF-8 hashes of raw user/scope text and existing function-specific formats | Verified seed, seed attestation and permits; no retroactive scope normalization |
| host output / callback | Exact candidate/output/callback UTF-8 and existing binding formats | Precommit/output verifier; no trimming, newline or Unicode normalization |
| host `stop_hook` | Existing session/turn formats, exact text and raw callback payload hashes | Post-hoc observation, never a substitute for precommit/sink |
| analysis `instruction_grammar::legacy_json_digest` | Stream hash of AST DTO compact JSON from `serde_json::to_writer` | `legacy_instruction_ast_digest` and AST→Program IR; preserve equality to String-based bytes |
| analysis `dgcl_projection` | Requirement/contract compact JSON and existing revision+NUL+content formats | Requirement realization/projection ledger; distinct from new AstId/FrameDigest |
| analysis `completion_plan` | Existing translation/input struct compact JSON | Plan construction/input checks; preserve order and omission rules |
| analysis `validation_producer` | Plan/observation-tuple/lifecycle-input compact JSON and existing run formats | Validation producers/claim consumers; no restoration of old authority/receipts |
| analysis `closure_connection` / `connection_graph` | Plan, expected JSON value, trace input and connection-tuple compact JSON | Connection/trace verifiers; never redefine expected values |
| analysis `production_connection` | Registered-spec tuple compact JSON and driver bytes | Registration/source binding; registration digest is not runtime observation |
| analysis `closure_resume` | Checkpoint-body compact JSON plus separate digest; trusted-head DTO bytes | Checkpoint reader/fresh-resume checks; preserve old reader, new journal in 07 |
| analysis `evidence_lifecycle` | Targets-array compact JSON and existing lease/intent formats | Snapshot invalidation and evidence/authority checks; migration never revives expiry |
| analysis `repair_loop` / `file_repair` | Legacy replacement payload, existing root-bound NUL formats, closure/claims JSON | Intent/apply/validation/resume; keep no-root and root-bound branches distinct |
| analysis `language_worker` | Worker request JSON, observation-tuple JSON and raw manifest/lock text | Configured-backend/source evidence; no reuse across request/version |
| analysis `dgcl_gold` | Corpus DTO/tuple compact JSON | Gold registration/evaluation; binary storage does not silently update corpus revision |
| analysis `pipeline` | Existing directional-loss compact JSON | Translation-envelope payload binding; hash is not semantic truth |
| analysis source/tool families | Exact text/existing formats in `closure`, `rust`, `python`, `cargo_world`, `process`, `validation`, `release` | Source/argv/environment/cwd/tool/release consumers; no bulk substitution by a new canonical helper |
| wire `alpha2_release` | Existing identity/gate format strings | Alpha.2 gate verifier; separate source readiness from host/general claims |
| wire `WireEnvelope::to_pretty_json` | Pretty JSON | Display only, never a MAC/signature preimage |
| substrate codec / typed derivation / context | Their separate new domains and explicit encodings | New typed identities only; no implicit substitution into old families |

### Adapter and strict consumers

- Callers explicitly select `ReceiptV1`, `ReplayV1` or `ReplayV2`; mismatched body schemas fail. Never guess a version by fallback attempts.
- Check input length before parsing. Callers explicitly preserve their old profile limit. The shared hard ceiling is 16MiB, not permission to expand old 2MiB stdin profiles. This admits bytes already in memory; file reads, deadlines and aggregate RSS belong to TLCX-05.
- Preserve receipt `deny_unknown_fields` and schema `additionalProperties:false`. New fields require new versions/views, not insertion into old DTOs. The new replay adapter DTO is strict without weakening or changing the existing host reader.
- Decode, roundtrip and digest checks remain untrusted. The only adapter path returning `VerifiedReceipt` delegates receipt, current policy and mutable replay guard to the existing verifier. Never skip expiry, issuer, class, subject, scope, payload, parent or nonce checks.

### Independent vector, negatives and open boundaries

Fixed vector independently computed with .NET HMAC-SHA256/SHA256: key bytes `00..1f`, issuer `legacy-fixture`, class `HostSeed`, subject `sha256:subject`, scope `host/user-span`, nonce `000102030405060708090a0b0c0d0e0f`, payload `sha256:` followed by 64 `a` characters, issued10, expires20, no parent. Claims text is 200 UTF-8 bytes. MAC=`sha256:e021688fd12425959b0f1dd04abdfe0ae392f7c6ef90dd739589b4276a819d65`; receipt digest=`sha256:8d2ad1400cf65fdf6481476f2851c628feec426291cc55a923b75bf931bbbeed`. This is a synthetic test key, not a production credential.

Required negatives cover wrong key/domain, rehashed claims, changed class spelling, new fields in old versions, V1/V2 preimage crossover, expiry/scope/replay, duplicate JSON keys, invalid UTF-8, trailing JSON and limit excess. Positives preserve the fixed MAC, old AST streaming bytes, original bytes and legacy typed roundtrips.

Neither this document nor digest agreement establishes dynamic execution of every call site. Record current test receipts separately in the slice exit ledger. Retain unobserved host G13, external rollback anchors, TLCX-05 resource broker, TLCX-07 journal migration and future consumer wiring as open boundaries.
