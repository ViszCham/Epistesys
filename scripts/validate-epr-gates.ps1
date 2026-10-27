[CmdletBinding()]
param(
    [string] $TargetDirectory,
    [string] $RecordDirectory,
    [switch] $ConfiguredNlp,
    [switch] $Gpu,
    [switch] $FreshPackage,
    [string] $ApprovedInventoryPath
)
$ErrorActionPreference = 'Stop'
$taskRepository = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
$taskWorkspace = Join-Path $taskRepository 'scripts/lc631'
if (-not $TargetDirectory) { $TargetDirectory = Join-Path $taskRepository '.cache/epr-final-target' }
if (-not $RecordDirectory) { $RecordDirectory = Join-Path $taskRepository ('.cache/epr-gates-' + [guid]::NewGuid().ToString('N')) }
if (Test-Path -LiteralPath $RecordDirectory) { throw 'A fresh gate record directory is required' }
[void](New-Item -ItemType Directory -Path $RecordDirectory)
$taskRecords = [Collections.Generic.List[object]]::new()
function Invoke-EprRecorded([string] $Label, [string] $Program, [string[]] $Arguments) {
    $taskLog = Join-Path $RecordDirectory ($Label + '.log')
    $taskTimer = [Diagnostics.Stopwatch]::StartNew()
    & $Program @Arguments *> $taskLog
    $taskExit = $LASTEXITCODE
    $taskRecord = [ordered]@{ label=$Label; program=$Program; arguments=$Arguments; exit_code=$taskExit
        elapsed_ms=$taskTimer.ElapsedMilliseconds; output_sha256=(Get-FileHash -LiteralPath $taskLog).Hash.ToLowerInvariant() }
    $taskRecords.Add($taskRecord)
    $taskRecord | ConvertTo-Json -Compress | Write-Output
    [IO.File]::WriteAllText((Join-Path $RecordDirectory 'commands.json'), ($taskRecords | ConvertTo-Json -Depth 8), [Text.UTF8Encoding]::new($false))
    if ($taskExit -ne 0) { Get-Content -LiteralPath $taskLog -Tail 70; throw ('Required EPR command failed: ' + $Label) }
}
Push-Location -LiteralPath $taskWorkspace
try {
    Invoke-EprRecorded 'format' 'cargo' @('fmt','--all','--check')
    Invoke-EprRecorded 'check' 'cargo' @('check','--workspace','--locked','--offline','--target-dir',$TargetDirectory)
    Invoke-EprRecorded 'no-default-tests' 'cargo' @('test','--workspace','--no-default-features','--locked','--offline','--target-dir',$TargetDirectory)
    Invoke-EprRecorded 'default-tests' 'cargo' @('test','--workspace','--locked','--offline','--target-dir',$TargetDirectory)
    Invoke-EprRecorded 'strict-clippy' 'cargo' @('clippy','--workspace','--all-targets','--locked','--offline','--target-dir',$TargetDirectory,'--','-D','warnings')
    Invoke-EprRecorded 'release' 'cargo' @('build','--workspace','--release','--locked','--offline','--target-dir',$TargetDirectory)
    Invoke-EprRecorded 'release-parser' 'cargo' @('test','-p','lc631-analysis','--test','epr_parser_progress','--release','--locked','--offline','--target-dir',$TargetDirectory,'--','--test-threads=1')
    Invoke-EprRecorded 'worker-lock-tests' 'python' @('-B','workers/test_dgcl_nlp_worker.py')
    Invoke-EprRecorded 'pinned-byte-tests' 'python' @('-B','workers/test_pinned_bytes.py')
    Invoke-EprRecorded 'media-wire-tests' 'python' @('-B','workers/test_media_backend.py')
    Invoke-EprRecorded 'media-resource-tests' 'python' @('-B','../test_media_resources.py')
    if ($ConfiguredNlp) {
        $taskPriorNlp = [Environment]::GetEnvironmentVariable('EPISTESYS_TEST_CONFIGURED_NLP','Process')
        try {
            $env:EPISTESYS_TEST_CONFIGURED_NLP = '1'
            Invoke-EprRecorded 'configured-ja-en' 'cargo' @('test','-p','lc631-cli','--test','cli_contract','dgcl_run_configured_stanza_profile_returns_one_candidate_and_keeps_truth_unclaimed','--locked','--offline','--target-dir',$TargetDirectory,'--','--exact')
        } finally { [Environment]::SetEnvironmentVariable('EPISTESYS_TEST_CONFIGURED_NLP',$taskPriorNlp,'Process') }
    }
    if ($Gpu) {
        Invoke-EprRecorded 'actual-gpu' 'cargo' @('test','-p','lc631-accelerator','--test','epr_numeric_contract','--release','--locked','--offline','--target-dir',$TargetDirectory,'epr_06_actual_gpu_parity_including_empty_edges','--','--ignored','--nocapture','--test-threads=1')
    }
} finally { Pop-Location }
if ($FreshPackage) {
    $taskPackageArguments = @('-NoProfile','-File',(Join-Path $PSScriptRoot 'new-alpha2-source-package.ps1'),'-BuildAndTest')
    if ($ApprovedInventoryPath) { $taskPackageArguments += @('-DevelopmentSnapshot','-ApprovedInventoryPath',$ApprovedInventoryPath) }
    Invoke-EprRecorded 'fresh-package' ((Get-Process -Id $PID).Path) $taskPackageArguments
}
[ordered]@{ schema_version='epistesys-epr-command-record.v1'; records=$taskRecords.Count; all_requested_commands_passed=$true
    configured_nlp_requested=[bool]$ConfiguredNlp; actual_gpu_requested=[bool]$Gpu; fresh_package_requested=[bool]$FreshPackage
    record_directory=$RecordDirectory; claim_boundary='Exact command observations only; source/hash/CI/audit/host/research closure remain separate gates.' } | ConvertTo-Json -Compress
