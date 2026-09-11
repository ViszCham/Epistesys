# Epistesys 6.3.2-alpha.1 initial clone release record

## 日本語

EPI-08で作成したEpistesys 6.3.2-alpha.1のroot commitは`0a009ea117709b541d0b8c037865a4cea43de22e`です。remoteが空でmain branchを持たなかったため、このcommitをmainのbootstrapとしてpushし、private設定とdefault branch=mainを確認しました。

この記録は、initial cloneのcommitと、その後のmerge可能な変更を区別するためのものです。Epistesysは、選別したv6.3.1 source、Epistesys identity、日英の入口文書、検証・制約・採否記録を含みます。package binary、target、cache、credentials、receipt root、replay ledger、親Git履歴は含みません。

このcommitは新規履歴であり、移植元の履歴を引き継ぎません。実装能力はsourceとlocal validationの範囲で記録し、external receipt、host activation、general performance、formal proof、release approvalへ拡張しません。

## English

The Epistesys 6.3.2-alpha.1 root commit created in EPI-08 is `0a009ea117709b541d0b8c037865a4cea43de22e`. Because the remote was empty and had no main branch, this commit was pushed as the main bootstrap; private visibility and default branch `main` were confirmed.

This record distinguishes the initial clone commit from subsequent mergeable changes. Epistesys contains selected v6.3.1 source, Epistesys identity, bilingual entry documents, and verification, limitation, and disposition records. It contains no package binary, target, cache, credential, receipt root, replay ledger, or parent Git history.

The commit starts a new history and does not inherit source history. Implementation capabilities are recorded within source and local-validation scope and are not expanded into external receipts, host activation, general performance, formal proof, or release approval.
