#Requires -Version 7.4
param()
$ErrorActionPreference = 'Stop'
$schema = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '../../schemas/epistesys-canonical-summary.v1.schema.json')).Path
$script:cases = 0
function New-Summary {
    [ordered]@{
        schema_version = 'epistesys-canonical-summary.v1'
        status = 'not_connected'; missing_stage = 'context_lowering'; task = 'reasoning'
        source_bytes = 1; source_digest = ('a' * 64)
        implementation_complete = $false; authority_created = $false; host_delivery_observed = $false
    }
}
function Assert-Summary($value, [bool] $expected, [string] $label) {
    $json = $value | ConvertTo-Json -Depth 8 -Compress
    $valid = Test-Json -Json $json -SchemaFile $schema -ErrorAction SilentlyContinue
    if ($valid -ne $expected) { throw "canonical summary schema: $label" }
    $script:cases++
}
Assert-Summary (New-Summary) $true 'bounded source'
foreach ($task in @('mathematics','reasoning','explanation','coding','media')) {
    $value = New-Summary; $value.task = $task
    Assert-Summary $value $true "task $task"
}
foreach ($status in @('needs_evidence','clarify','hold','conflict')) {
    $value = New-Summary; $value.status = $status; $value.missing_stage = $null
    Assert-Summary $value $true "state $status"
}
$value = New-Summary
$value.missing_stage = 'artifact_store_capability'; $value.task = $null
$value.source_bytes = $null; $value.source_digest = $null
Assert-Summary $value $true 'unconfigured artifact capability is not observed zero'
foreach ($field in @('implementation_complete','authority_created','host_delivery_observed')) {
    $value = New-Summary; $value[$field] = $true
    Assert-Summary $value $false "promotion $field"
}
foreach ($case in @(
    @('source_bytes',0), @('source_bytes',262145), @('source_bytes',$true),
    @('source_digest',('A'*64)), @('source_digest',('a'*63)),
    @('source_digest',$null), @('task',$null), @('status','accepted'),
    @('missing_stage',$null), @('schema_version','other.v1'), @('proof_verified',$true)
)) {
    $value = New-Summary; $value[$case[0]] = $case[1]
    Assert-Summary $value $false "invalid $($case[0])"
}
$value = New-Summary; $value.Remove('task')
Assert-Summary $value $false 'missing field'
$value = New-Summary; $value.source_bytes = 262144
Assert-Summary $value $true 'source boundary'
$value = New-Summary; $value.missing_stage = 'artifact_store_capability'
Assert-Summary $value $false 'artifact capability cannot mix source observations'
[pscustomobject]@{schema='epistesys-canonical-summary.v1';cases=$script:cases;result='shape_checks_passed';semantic_evidence=$false}|ConvertTo-Json -Compress
