---
name: lc631-audit
description: Audit Epistesys 6.3.2-alpha.1 inherited closure levels, accelerator receipts, media authority, and promotion blockers.
---

# Epistesys Audit

## 日本語

最小の該当commandだけを実行します。

- `lc631-v630-guard --repo <root>`: 継承元baseline境界
- `lc631-doctor --repo <root>`: 統合parallel report
- `lc631-rpa-doctor --prompt <task> --repo <repo> --source-file <source>`: RPA-00〜39のRust／Python／Assembly program-analysis shadow
- `lc631-gpu-doctor --execute`: 2,048 evaluation、8 lane decision parity、bounded readback、device timestamp、callback
- `lc631-gpu-stress --iterations 64 --workers 8`: overlap、SM activity、VRAM
- `lc631-media-doctor`: backend readiness
- `lc631-media-execute`: authenticated source／backend receipt付きlocal mediaのcandidate execution receipt
- `run-paired-semantic-regression.ps1`: 64 seed structural regression
- `Stop` hook／`lc631-host-stop-hook`: authenticated v2 output bindingとPLUGIN_DATA durable replay。fresh host pickupは別観測
- `lc631-promotion-gate`: fail-closed replacement predicate

Declared、Typed、Reachable、Executed、Consumed、Contained、HostBoundを分離します。Unavailableをsuccessへ変換せず、SM activityをoccupancy、exact local bindingをhost post-send callback、local testをremote CIへ昇格させません。

---

## English

Run the narrowest applicable command:

- `lc631-v630-guard --repo <root>` for the inherited baseline boundary.
- `lc631-doctor --repo <root>` for the integrated shadow report.
- `lc631-rpa-doctor --prompt <task> --repo <repo> --source-file <source>` for the RPA-00 through RPA-39 Rust/Python/Assembly program-analysis shadow.
- `lc631-gpu-doctor --execute` for 2,048 evaluations, eight-lane decision parity, bounded readback, device timing, and callback registration.
- `lc631-gpu-stress --iterations 64 --workers 8` for overlap, SM activity, and VRAM observation.
- `lc631-media-doctor` for source and backend readiness.
- `lc631-media-execute` for a candidate execution receipt with authenticated source and backend receipts.
- `run-paired-semantic-regression.ps1` for the fixed 64-seed structural regression.
- the `Stop` hook / `lc631-host-stop-hook` for authenticated v2 output binding and PLUGIN_DATA durable replay; fresh host pickup remains a separate observation.
- `lc631-promotion-gate` for fail-closed release predicates.

Report Declared, Typed, Reachable, Executed, Consumed, Contained, and HostBound separately. Never convert Unavailable into success; SM activity into occupancy; exact local binding into a host post-send callback; or local tests into remote CI evidence.
