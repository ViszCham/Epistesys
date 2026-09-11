[CmdletBinding()]
param(
    [string]$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..\..')).Path,
    [string]$Corpus = (Join-Path (Split-Path -Parent $PSScriptRoot) 'fixtures\paired-semantic-corpus.v1.json'),
    [string]$V630Binary = (Join-Path $RepoRoot 'scripts\reasoning-helper\bin\windows-x86_64\reasoning-helper.exe'),
    [string]$V631Binary = (Join-Path $PSScriptRoot 'lc631\target\release\lc631.exe'),
    [string]$Output
)

$ErrorActionPreference = 'Stop'
$OutputEncoding = [Text.UTF8Encoding]::new($false)
[Console]::OutputEncoding = $OutputEncoding
function Get-Sha256Hex([byte[]]$Bytes) {
    $Hasher = [Security.Cryptography.SHA256]::Create()
    try {
        [BitConverter]::ToString($Hasher.ComputeHash($Bytes)).Replace('-', '').ToLowerInvariant()
    } finally {
        $Hasher.Dispose()
    }
}
foreach ($required in @($Corpus, $V630Binary, $V631Binary)) {
    if (-not (Test-Path -LiteralPath $required -PathType Leaf)) {
        throw "Required paired-regression input is missing: $required"
    }
}
$fixture = Get-Content -LiteralPath $Corpus -Raw -Encoding UTF8 | ConvertFrom-Json
$results = [System.Collections.Generic.List[object]]::new()
foreach ($case in $fixture.cases) {
    $v630Raw = & $V630Binary decision_envelope --prompt ([string]$case.seed) --mode ([string]$case.mode) --compute full --format json
    if ($LASTEXITCODE -ne 0) {
        throw "v6.3.0 execution failed for $($case.id)"
    }
    $v631Raw = & $V631Binary lc631-tl-doctor --prompt ([string]$case.seed)
    if ($LASTEXITCODE -ne 0) {
        throw "v6.3.1 execution failed for $($case.id)"
    }
    $v630 = $v630Raw | ConvertFrom-Json
    $v631 = $v631Raw | ConvertFrom-Json
    $v631ChainComplete = $v631.payload.chain_complete -eq $true
    $v631WitnessPresent =
        [int]$v631.payload.obligation_count -ge 1 -and
        [int]$v631.payload.witness_count -ge 1
    $v631UnboundFailExplicit =
        -not $v631ChainComplete -and
        $v631.payload.source_roundtrip_exact -eq $true -and
        [string]$v631.payload.gate -eq 'clarify' -and
        [int]$v631.payload.parse_defect_count -ge 1
    $checks = [ordered]@{
        v630_identity = $v630.engine_version -eq '6.3.0'
        v630_full_compute = $v630.compute_mode -eq 'full'
        v630_semantic_parity = $v630.semantic_parity -eq $true
        v630_stage_floor = [int]$v630.executed_stage_count -ge 67
        v630_mode_preserved = $v630.projection_mode -eq [string]$case.mode
        v630_host_boundary_visible = $v630.authority.authority_input_binding_status -eq 'host_receipt_unavailable'
        v631_identity = ([string]$v631.version).StartsWith('6.3.1')
        v631_shadow_boundary = $v631.shadow_only -eq $true
        v631_witness_chain = $v631WitnessPresent
        v631_unbound_chain_fail_explicit = $v631UnboundFailExplicit
        v631_no_fixed_threshold = $v631.payload.fixed_numeric_threshold_used -eq $false
    }
    $passed = -not ($checks.Values -contains $false)
    $results.Add([pscustomobject][ordered]@{
        id = [string]$case.id
        family = [string]$case.family
        mode = [string]$case.mode
        seed_sha256 = 'sha256:' + (Get-Sha256Hex ([Text.Encoding]::UTF8.GetBytes([string]$case.seed)))
        checks = $checks
        passed = $passed
        v630_gate = [string]$v630.gate_decision
        v630_output_gate = [string]$v630.output_commit_decision
        v631_projection_gate = [string]$v631.payload.gate
        v631_chain_complete = $v631ChainComplete
    })
}
$familySummary = [ordered]@{}
foreach ($group in ($results | Group-Object family)) {
    $familySummary[$group.Name] = [ordered]@{
        cases = $group.Count
        passed = @($group.Group | Where-Object passed).Count
    }
}
$passedCount = @($results | Where-Object passed).Count
$report = [ordered]@{
    schema_version = 'lc631-paired-semantic-regression.v1'
    check_semantics_revision = 'lc631-paired-checks.v2'
    corpus_sha256 = 'sha256:' + (Get-FileHash -LiteralPath $Corpus -Algorithm SHA256).Hash.ToLowerInvariant()
    case_count = $results.Count
    minimum_case_count = [int]$fixture.minimum_case_count
    passed_count = $passedCount
    failed_count = $results.Count - $passedCount
    family_summary = $familySummary
    strict_pass = ($results.Count -ge [int]$fixture.minimum_case_count) -and ($passedCount -eq $results.Count)
    chain_policy = 'witness_presence_and_fail_explicit_unbound_state_are_required; chain_complete_remains_an_observation'
    compared_versions = @('6.3.0', '6.3.1-shadow')
    cases = $results
    claim_boundary = 'Paired structural regression across this fixed corpus is not universal semantic equivalence, answer-quality proof, or host-output binding.'
}
$json = $report | ConvertTo-Json -Depth 12
if ($Output) {
    $parent = Split-Path -Parent $Output
    if ($parent -and -not (Test-Path -LiteralPath $parent -PathType Container)) {
        New-Item -ItemType Directory -Path $parent -Force | Out-Null
    }
    [IO.File]::WriteAllText($Output, $json + [Environment]::NewLine, [Text.UTF8Encoding]::new($false))
}
$json
if (-not $report.strict_pass) { exit 2 }
