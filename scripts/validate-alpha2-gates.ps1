param([switch] $FreshPackage)
$ErrorActionPreference = 'Stop'
$repoRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
$workspace = Join-Path $repoRoot 'scripts/lc631'
$recordRoot = Join-Path ([IO.Path]::GetTempPath()) ('epistesys-gates-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $recordRoot | Out-Null
$records = [Collections.Generic.List[object]]::new()
function Invoke-RecordedCargo([string] $label, [string[]] $arguments) {
    $log = Join-Path $recordRoot ($label + '.log')
    $started = [Diagnostics.Stopwatch]::StartNew()
    & cargo @arguments *> $log
    $code = $LASTEXITCODE
    $record = [ordered]@{ label=$label; program='cargo'; arguments=$arguments; exit_code=$code;
        elapsed_ms=$started.ElapsedMilliseconds; output_digest=('sha256:' + (Get-FileHash -LiteralPath $log -Algorithm SHA256).Hash.ToLowerInvariant()) }
    $records.Add($record)
    $record | ConvertTo-Json -Compress | Write-Output
    if ($code -ne 0) { Get-Content -LiteralPath $log -Tail 55; throw "mandatory gate failed: $label" }
}
Push-Location -LiteralPath $workspace
try {
    Invoke-RecordedCargo 'fmt' @('fmt','--all','--check')
    Invoke-RecordedCargo 'check' @('check','--workspace','--locked')
    Invoke-RecordedCargo 'no-default' @('test','--workspace','--no-default-features','--locked')
    Invoke-RecordedCargo 'default' @('test','--workspace','--locked')
    Invoke-RecordedCargo 'clippy' @('clippy','--workspace','--all-targets','--locked','--','-D','warnings')
    Invoke-RecordedCargo 'release' @('build','--release','--locked','--offline')
    $env:EPISTESYS_TEST_CONFIGURED_NLP = '1'
    try {
        Invoke-RecordedCargo 'configured-ja-en' @('test','-p','lc631-cli','--test','cli_contract',
            'dgcl_run_configured_stanza_profile_returns_one_candidate_and_keeps_truth_unclaimed','--locked','--','--exact')
    } finally { Remove-Item Env:EPISTESYS_TEST_CONFIGURED_NLP -ErrorAction SilentlyContinue }
} finally { Pop-Location }
if ($FreshPackage) {
    & (Join-Path $PSScriptRoot 'new-alpha2-source-package.ps1') -BuildAndTest
    if ($LASTEXITCODE -ne 0) { throw 'fresh package validation failed' }
}
[IO.File]::WriteAllText((Join-Path $recordRoot 'commands.json'), ($records | ConvertTo-Json -Depth 8), [Text.UTF8Encoding]::new($false))
[ordered]@{ schema_version='epistesys-functional-command-observations.v1'; record_root=$recordRoot;
    commands=$records.Count; all_passed=$true; claim_boundary='command observations only; source-profile gate mapping and audit are separate; not general correctness, research evaluation, host activation or authority' } | ConvertTo-Json -Compress
