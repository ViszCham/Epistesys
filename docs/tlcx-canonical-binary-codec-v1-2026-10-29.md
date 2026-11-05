# TLCX canonical binary codec v1

## 日本語

### 状態と範囲

2026-10-29固定のTLCX-03追加契約。ロードマップの64-byte header／32-byte directoryを変更せず、未指定だったdomain、タグ、minor、文字列／mapの規約を固定する。これは実装・検証・release完了の宣言ではない。既存public JSON、旧MAC、旧replay canonical bytesを変更しない。

### frameとdigest

- magicはASCII `EPITLCX1`。encoderはmajor=1/minor=0を生成。すべてlittle endian。
- header offsets：magic=0、major=8、minor=10、flags=12、payload_len=16、section_count=24、header_len=28、digest=32。header_len=64、header flags=0。
- directory entryはtag:u16、version:u16、flags:u32、offset:u64、length:u64、count:u32、reserved:u32の32 bytes。section flagsはbit0 Criticalだけ、reserved=0。
- directoryはheader直後。payload_lenはdirectoryを除外する。tagはstrict昇順・一意。sectionはdirectory直後から隙間／padding／重複なしに連続し、最後のendがframe全長と一致する。zero-length sectionは許可する。
- SHA-256のpreimageは、37-byteの`b"Epistesys/TLCX/FrameDigest/sha256/v1\0"`＋header[0..32]＋32 zero bytes＋exact directory＋exact payload。domain末尾は1 byteの00。BOMや改行を加えない。
- hash一致はcontent integrityだけで、認証、receipt真正性、意味検証、Authority、実host deliveryではない。

### registryと互換性

| tag | section version | payloadとcount |
| --- | --- | --- |
| 0x0001 SourceUtf8 | 1 | raw UTF-8、count=1。空sourceも有効 |
| 0x0002 StringMap | 1 | canonical key/value列、count=entry数 |

envelope decoderはmajor=1のfuture minor metadataとunknown optional payloadをopaqueのまま保持する。unknown critical tagと、unsupported versionのcritical sectionは拒否する。known tagのunsupported optional versionもopaqueであり、そのtagのtyped値として使わない。unknown flagsは拒否する。typed Source profileはminor=0とCritical SourceUtf8/v1を必須にする。未知optionalのcountの意味は未検証であり、envelope成功を全section意味対応済みと扱わない。

空frame（section_count=0）はenvelopeとして許可するがSource profileはMissingSource。source以外の将来payloadは対応済みと主張しない。旧MACは旧preimageのまま検証し、新FrameDigestへの移行には別の明示migration契約を要する。

### canonical文字列・map・資源

- 一般Stringはu32 little-endianのUTF-8 byte length＋exact bytes。K6 termの最短ULEB128とは別codec。
- StringMapはkey:String＋value:Stringをdirectory.count件並べる。payload内の追加countは持たない。keyはUTF-8 bytesのstrict昇順、duplicate拒否。writerは非意味的なmap順序だけをsortし、decoderは非canonical順序を拒否する。
- Unicode正規化、case fold、改行変換、BOM／NUL除去をしない。ordered list／AST childをmapとしてsortしない。
- frame全体<=16MiB、sections<=64、map entries<=100,000、個別StringとSourceUtf8<=256KiB。count×minimum-record-size、offset＋length、全長をchecked計算し、allocation前に上限を確認する。
- unknown optionalは原bytesを借用して保持し、exact frame bytesを取得できる。未知sectionを捨てた再encodeに旧digest／署名が残るとは主張しない。
- 検査順はheader／全長、directoryの構造、digest、known payload。single-frame slice decoderはtrailing bytesを拒否する。stream readerは一frameだけ読み、次frameのbyteをEOF確認のために消費しない。
- readerはheader確認後、総量上限を検査してからfallible reserveする。部分readや失敗時は完成frameを公開しない。deadline、caller側の取得、aggregate RAM／RSS、disk／child管理はTLCX-05等の上位責務。

### goldenと完了境界

独立した.NET SHA-256計算によるG1はunknown optional tag=0xFFFE/version=1/flags=0/count=1、offset=96、payload=`e697a5`、全99 bytes。FrameDigestは`da5f58ecbb5dd2a8e91e7807841ad5a126ffc2a7cdea5019daa6c27cd4900623`。これはcodec vectorで、Source profileやsemantic completionの証拠ではない。TLCX-03出口にはgolden、負例、bounded reader実経路、互換回帰と現在snapshotの検証が必要。

## English

### Status and scope

This additive TLCX-03 contract is fixed on 2026-10-29. It preserves the roadmap's 64-byte header and 32-byte directory while specifying previously unspecified domains, tags, minor versions and string/map rules. It does not declare implementation, verification or release completion. Preserve existing public JSON, legacy MACs and legacy replay canonical bytes.

### Frames and digests

- Magic is ASCII `EPITLCX1`. The encoder emits major=1/minor=0. All numbers use little endian.
- Header offsets: magic=0, major=8, minor=10, flags=12, payload_len=16, section_count=24, header_len=28, digest=32. Require header_len=64 and header flags=0.
- Each 32-byte directory entry contains tag:u16, version:u16, flags:u32, offset:u64, length:u64, count:u32, reserved:u32. Only section-flag bit0 Critical is permitted; reserved=0.
- The directory immediately follows the header; payload_len excludes it. Tags are strictly ascending and unique. Sections start after the directory and remain contiguous without gaps, padding or overlap; the final end equals frame length. Zero-length sections are permitted.
- SHA-256 preimage: the 37 bytes `b"Epistesys/TLCX/FrameDigest/sha256/v1\0"`, header[0..32], 32 zero bytes, exact directory and exact payload. The domain ends with one 00 byte, without a BOM or newline.
- Hash equality establishes content integrity only, not authentication, receipt authenticity, semantic validation, Authority or actual host delivery.

### Registry and compatibility

| Tag | Section version | Payload and count |
| --- | --- | --- |
| 0x0001 SourceUtf8 | 1 | Raw UTF-8, count=1; an empty source is valid |
| 0x0002 StringMap | 1 | Canonical key/value sequence, count=entry count |

The envelope decoder preserves future-minor metadata under major=1 and unknown optional payloads as opaque bytes. Reject unknown critical tags and critical sections with unsupported versions. Unsupported optional versions of known tags are also opaque, never usable as their tag's typed values. Reject unknown flags. Typed Source admission requires minor=0 and a Critical SourceUtf8/v1 section. The meaning of an unknown optional section's count remains unverified; envelope success is not semantic support for every section.

An empty frame with section_count=0 is permitted as an envelope, but Source admission returns MissingSource. Do not claim future payload support. Verify legacy MACs using legacy preimages; migrating to FrameDigest requires a separate explicit migration contract.

### Canonical strings, maps and resources

- General String encoding is u32 little-endian UTF-8 byte length followed by exact bytes, distinct from K6 terms' shortest ULEB128 codec.
- StringMap contains directory.count key:String/value:String pairs, without an additional payload count. Keys use strict UTF-8-byte order and reject duplicates. The writer sorts only nonsemantic map order; the decoder rejects noncanonical order.
- Never normalize Unicode, fold case, change newlines, or remove BOM/NUL bytes. Do not sort ordered lists or AST children as maps.
- Entire frames are at most 16MiB, section count at most 64, map entries at most 100,000, and individual Strings/SourceUtf8 at most 256KiB. Check count×minimum-record-size, offset+length and total sizes before allocation.
- Borrow original unknown-optional bytes and expose exact original frame bytes. Dropping unknown sections and reencoding cannot preserve the old digest/signature by assertion.
- Check header/total size, directory structure, digest, then known payloads. Exact single-frame slice decoding rejects trailing bytes. A stream reader reads one frame only and never consumes the next frame to check EOF.
- After validating a header, the reader checks total limits before fallible reserve. Partial reads and failures never publish a completed frame. Deadlines, caller acquisition, aggregate RAM/RSS and disk/child supervision belong to TLCX-05 or other upper layers.

### Golden vector and completion boundary

G1, independently calculated with .NET SHA-256, contains unknown optional tag=0xFFFE/version=1/flags=0/count=1, offset=96 and payload `e697a5`, totaling 99 bytes. FrameDigest is `da5f58ecbb5dd2a8e91e7807841ad5a126ffc2a7cdea5019daa6c27cd4900623`. It is a codec vector, not evidence of Source-profile or semantic completion. TLCX-03 exit still requires golden/negative tests, a real bounded-reader path, compatibility regressions and current-snapshot validation.
