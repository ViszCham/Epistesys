#Requires -Version 7.4
param()
$ErrorActionPreference = 'Stop'
$schemaPath = Join-Path $PSScriptRoot '../../schemas/lc631-journal-bundle-inspect.v1.schema.json'
$schemaPath = (Resolve-Path -LiteralPath $schemaPath -ErrorAction Stop).Path
$script:caseCount = 0

function New-PositiveReport {
    [ordered]@{
        schema_version = 'lc631-journal-bundle-inspect.v1'
        status = 'inspected_unanchored'; hold_code = $null
        segment_count = 1; record_count = 2; segment_input_bytes = 512
        latest_record_sequence = 2; local_head_frame_digest_bytes = @(0..31)
        head_state = 'unanchored'; rollback_resistance = 'unavailable'
        fresh_revalidation_required = $true; external_anchor_observed = $false
        authority_created = $false; completion_restored = $false
        output_commit_allowed = $false; diagnostic_only = $true
        claim_boundary = 'local byte schema only; not proof or permission'
    }
}
function New-HoldReport {
    $report = New-PositiveReport
    $report.status = 'hold'; $report.hold_code = 'bundle_rebuild_required'
    foreach ($field in @('segment_count','record_count','segment_input_bytes',
            'latest_record_sequence','local_head_frame_digest_bytes','head_state')) {
        $report[$field] = $null
    }
    $report
}
function Assert-Shape {
    param($Report, [bool] $Expected, [string] $Label)
    $wire = $Report | ConvertTo-Json -Depth 12 -Compress
    $actual = Test-Json -Json $wire -SchemaFile $schemaPath -ErrorAction SilentlyContinue
    if ($actual -ne $Expected) { throw "Bundle schema case failed: $Label" }
    $script:caseCount++
}

Assert-Shape (New-PositiveReport) $true 'positive local observation'
Assert-Shape (New-HoldReport) $true 'held unobserved values'
$report = New-HoldReport; $report.hold_code = 'unexpected_write_may_have_applied'
Assert-Shape $report $true 'uncertain effects stay visible'
foreach ($field in @('authority_created','completion_restored','output_commit_allowed','external_anchor_observed')) {
    $report = New-PositiveReport; $report[$field] = $true
    Assert-Shape $report $false "forbidden promotion $field"
}
foreach ($field in @('segment_count','record_count','segment_input_bytes','latest_record_sequence')) {
    foreach ($number in @(0,1)) {
        $report = New-HoldReport; $report[$field] = $number
        Assert-Shape $report $false "unobserved is not numeric $field $number"
    }
}
foreach ($case in @(
    @('status','accepted'), @('rollback_resistance','verified'),
    @('diagnostic_only',$false), @('schema_version','other.v1'),
    @('segment_count',1001), @('record_count',131073),
    @('record_count',$true), @('segment_input_bytes',16777217),
    @('proof_verified',$true), @('hold_code','resource_hold')
)) {
    $report = New-PositiveReport; $report[$case[0]] = $case[1]
    Assert-Shape $report $false "invalid field $($case[0])"
}
$report = New-PositiveReport; $report.local_head_frame_digest_bytes = @(0..30)
Assert-Shape $report $false 'short digest'
$invalidBytes = @(0..31); $invalidBytes[0] = 256
$report = New-PositiveReport; $report.local_head_frame_digest_bytes = $invalidBytes
Assert-Shape $report $false 'out-of-range digest byte with correct length'
$report = New-PositiveReport; $report.record_count = $null
Assert-Shape $report $false 'positive observation requires record count'
$report = New-HoldReport; $report.hold_code = $null
Assert-Shape $report $false 'hold requires a reason'
$positive = New-PositiveReport
foreach ($field in @($positive.Keys)) {
    $report = New-PositiveReport; $report.Remove($field)
    Assert-Shape $report $false "missing required $field"
}
if ($script:caseCount -ne 46) { throw 'Schema test plan was unexpectedly empty, repeated or incomplete' }
[ordered]@{
    cases = $script:caseCount; validator = 'PowerShell Test-Json'
    powershell_version = $PSVersionTable.PSVersion.ToString()
    scope = 'synthetic wire vectors only; not semantic truth, proof or authority'
} | ConvertTo-Json -Compress
