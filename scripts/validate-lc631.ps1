param(
    [switch] $IncludeGpuObservation
)

$ErrorActionPreference = 'Stop'
$PluginRoot = Split-Path -Parent $PSScriptRoot
$RepoRoot = Resolve-Path (Join-Path $PluginRoot '..\..')
$Manifest = Join-Path $PSScriptRoot 'lc631/Cargo.toml'

cargo fmt --manifest-path $Manifest --all -- --check
if ($LASTEXITCODE -ne 0) { throw 'lc631 format validation failed' }
cargo check --manifest-path $Manifest --workspace --no-default-features
if ($LASTEXITCODE -ne 0) { throw 'lc631 no-default check failed' }
cargo check --manifest-path $Manifest --workspace
if ($LASTEXITCODE -ne 0) { throw 'lc631 default check failed' }
cargo test --manifest-path $Manifest --workspace --no-default-features
if ($LASTEXITCODE -ne 0) { throw 'lc631 no-default tests failed' }
cargo test --manifest-path $Manifest --workspace
if ($LASTEXITCODE -ne 0) { throw 'lc631 default tests failed' }
cargo clippy --manifest-path $Manifest --workspace --all-targets --no-default-features -- -D warnings
if ($LASTEXITCODE -ne 0) { throw 'lc631 no-default clippy failed' }
cargo clippy --manifest-path $Manifest --workspace --all-targets -- -D warnings
if ($LASTEXITCODE -ne 0) { throw 'lc631 default clippy failed' }
cargo build --manifest-path $Manifest --release --locked
if ($LASTEXITCODE -ne 0) { throw 'lc631 locked release build failed' }
& (Join-Path $PSScriptRoot 'run-lc631.ps1') lc631-doctor --repo $RepoRoot
if ($LASTEXITCODE -ne 0) { throw 'lc631 integrated doctor failed' }
& (Join-Path $PSScriptRoot 'run-lc631.ps1') lc631-tldg-doctor --prompt 'Parse natural language and fn main() {} in one substrate.'
if ($LASTEXITCODE -ne 0) { throw 'lc631 TLDG doctor failed' }
& (Join-Path $PSScriptRoot 'run-lc631.ps1') lc631-tldg-adversarial
if ($LASTEXITCODE -ne 0) { throw 'lc631 TLDG adversarial validation failed' }
$RpaFixture = Join-Path $PSScriptRoot 'lc631/fixtures/rpa/valid.rs'
& (Join-Path $PSScriptRoot 'run-lc631.ps1') lc631-rpa-doctor --prompt 'Audit Rust program analysis.' --repo (Split-Path -Parent $Manifest) --source-file $RpaFixture
if ($LASTEXITCODE -ne 0) { throw 'lc631 RPA doctor failed' }
if ($IncludeGpuObservation) {
    & (Join-Path $PSScriptRoot 'run-lc631.ps1') lc631-gpu-doctor --execute
    if ($LASTEXITCODE -ne 0) { throw 'lc631 GPU observation failed' }
    $TldgGpuRaw = & (Join-Path $PSScriptRoot 'run-lc631.ps1') lc631-tldg-gpu-doctor --execute --prompt 'Parse natural language and fn main() {} in one substrate.' | Out-String
    if ($LASTEXITCODE -ne 0) { throw 'lc631 TLDG GPU parity failed' }
    $TldgGpu = $TldgGpuRaw | ConvertFrom-Json
    if (-not $TldgGpu.payload.real_gpu_observed -or -not $TldgGpu.payload.all_lane_parity) {
        throw 'lc631 TLDG real GPU parity predicates failed'
    }
    $TldgGpuRaw | Write-Output
    $TldgReleaseRaw = & (Join-Path $PSScriptRoot 'run-lc631.ps1') lc631-tldg-release-gate --execute --prompt 'Parse natural language and fn main() {} in one substrate.' | Out-String
    if ($LASTEXITCODE -ne 0) { throw 'lc631 TLDG source release gate failed' }
    $TldgRelease = $TldgReleaseRaw | ConvertFrom-Json
    if ($TldgRelease.payload.release_complete -or $TldgRelease.payload.blocked_reasons.Count -eq 0) {
        throw 'lc631 TLDG legacy-boolean release route did not remain fail-closed'
    }
    $TldgReleaseRaw | Write-Output
}
