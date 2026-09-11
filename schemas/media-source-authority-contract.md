# LC631 Media Source Authority Contract

## 日本語

YouTube URL単体はmedia bytes取得権限ではない。default routeはmetadata-only、またはユーザーが提供したlocal fileのsource snapshotを`lc631-authenticated-receipt.v1`へ束縛する経路である。`user_asserts_rights=true`、existing path、repository文書だけではMediaSourceAuthorityにならない。外部取得物は`ExternalUserManagedAcquisition`としてlocal fileへ到着し、bytes・digest・scopeが再検証されてから検査する。人物trackは匿名で、face action unitは感情・意図・本人同定を確定しない。二クラスタ音響特徴はspeaker identity／diarizationではなく、frame差分はaction recognitionではなく、時間的共存はsemantic alignmentではない。

## English

A YouTube URL alone is not authority to acquire media bytes. The default route is metadata-only, or a route that binds the source snapshot of a user-supplied local file into `lc631-authenticated-receipt.v1`. `user_asserts_rights=true`, an existing path, or repository prose cannot create MediaSourceAuthority. Externally acquired content arrives as an `ExternalUserManagedAcquisition` local file; its bytes, digest, and scope are re-verified before inspection. Person tracks remain anonymous, and facial action units do not commit emotion, intent, or identity. Two-cluster acoustic features are not speaker identity or diarization, frame difference is not action recognition, and temporal co-presence is not semantic alignment.
