param([switch] $ContractOnly)
$ErrorActionPreference = 'Stop'
$packer = Join-Path $PSScriptRoot 'new-alpha2-source-package.ps1'
$source = Get-Content -LiteralPath $packer -Raw
if ($source -match 'ls-files[^\r\n]*--others') { throw 'untracked_inventory_regression' }
if ($source -notmatch 'DevelopmentSnapshot' -or $source -notmatch 'ApprovedInventoryPath') {
    throw 'explicit_development_approval_contract_missing'
}
if ($ContractOnly) { Write-Output 'EPR packaging contract present'; exit 0 }

$repository = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
$fixture = Join-Path ([IO.Path]::GetTempPath()) ('Epistesys packaging test ' + [guid]::NewGuid().ToString('N'))
if (Test-Path -LiteralPath $fixture) { throw 'owned_packaging_fixture_collision' }
& git clone --quiet --local --no-hardlinks $repository $fixture
if ($LASTEXITCODE -ne 0) { throw 'owned_packaging_fixture_clone_failed' }
$shell = (Get-Process -Id $PID).Path
$baseCommit = (& git -C $fixture rev-parse HEAD) -join ''

function Invoke-TestPacker {
    param([string[]] $Extra, [string] $ExpectedError)
    $info = [Diagnostics.ProcessStartInfo]::new()
    $info.FileName = $shell
    $info.UseShellExecute = $false
    $info.CreateNoWindow = $true
    $info.RedirectStandardOutput = $true
    $info.RedirectStandardError = $true
    foreach ($argument in (@('-NoProfile', '-NonInteractive', '-File', $packer, '-RepositoryRoot', $fixture) + $Extra)) {
        [void]$info.ArgumentList.Add($argument)
    }
    $process = [Diagnostics.Process]::new()
    $process.StartInfo = $info
    [void]$process.Start()
    try {
        $stdout = $process.StandardOutput.ReadToEndAsync()
        $stderr = $process.StandardError.ReadToEndAsync()
        if (-not $process.WaitForExit(120000)) {
            $process.Kill($true)
            $process.WaitForExit()
            throw 'owned_package_test_deadline_exceeded'
        }
        $text = $stdout.GetAwaiter().GetResult()
        $errorText = $stderr.GetAwaiter().GetResult()
        if ($ExpectedError) {
            if ($process.ExitCode -eq 0 -or -not $errorText.Contains($ExpectedError)) {
                throw ('expected_package_rejection_missing: ' + $ExpectedError + ' ' + $errorText)
            }
            return $null
        }
        if ($process.ExitCode -ne 0) { throw ('package_test_failed: ' + $errorText) }
        return $text | ConvertFrom-Json
    } finally { $process.Dispose() }
}

$release = Invoke-TestPacker -Extra @()
if (-not $release.release_candidate -or $release.package_mode -ne 'fixed_commit_release' -or $release.source_commit -ne $baseCommit) {
    throw 'fixed_commit_package_identity_failed'
}
$releaseEntries = Get-Content -LiteralPath (Join-Path $release.package_root 'source-package-inventory.json') -Raw | ConvertFrom-Json
if (@($releaseEntries).Count -lt 100) { throw 'fixed_package_incomplete' }
foreach ($entry in $releaseEntries) {
    if ((Get-FileHash -LiteralPath (Join-Path $release.package_root $entry.path) -Algorithm SHA256).Hash.ToLowerInvariant() -ne $entry.sha256) {
        throw 'fixed_package_member_changed'
    }
}

$injectionRoot = Join-Path $fixture 'scripts/lc631/crates/lc631-analysis/src'
foreach ($name in @('dev-root.key', '.env', 'private-settings.json')) {
    [IO.File]::WriteAllText((Join-Path $injectionRoot $name), 'dummy-fixture-not-a-secret', [Text.UTF8Encoding]::new($false))
}
Invoke-TestPacker -Extra @() -ExpectedError 'release_package_requires_clean_worktree'

function Write-TestApproval {
    param([string] $Name, [object[]] $Additional = @())
    $entries = @(foreach ($entry in $releaseEntries) {
        $file = Get-Item -LiteralPath (Join-Path $fixture $entry.path)
        [ordered]@{ path = $entry.path; bytes = $file.Length; sha256 = (Get-FileHash -LiteralPath $file.FullName -Algorithm SHA256).Hash.ToLowerInvariant() }
    }) + $Additional
    $approvalPath = Join-Path $fixture $Name
    $approval = [ordered]@{ schema_version = 'epistesys-source-package-approval.v1'; base_commit = $baseCommit; entries = $entries }
    [IO.File]::WriteAllText($approvalPath, (ConvertTo-Json -InputObject $approval -Depth 8), [Text.UTF8Encoding]::new($false))
    return $approvalPath
}

$approvalPath = Write-TestApproval 'approved-development.json'
$development = Invoke-TestPacker -Extra @('-DevelopmentSnapshot', '-ApprovedInventoryPath', $approvalPath)
if ($development.release_candidate -or $development.package_mode -ne 'approved_development_snapshot') {
    throw 'development_snapshot_claimed_as_release'
}
foreach ($name in @('dev-root.key', '.env', 'private-settings.json')) {
    if (Test-Path -LiteralPath (Join-Path $development.package_root ('scripts/lc631/crates/lc631-analysis/src/' + $name))) {
        throw 'unapproved_dummy_secret_entered_package'
    }
}

$keyPath = Join-Path $injectionRoot 'dev-root.key'
$badApproval = Write-TestApproval 'forbidden-development.json' @([ordered]@{
    path = 'scripts/lc631/crates/lc631-analysis/src/dev-root.key'
    bytes = (Get-Item -LiteralPath $keyPath).Length
    sha256 = (Get-FileHash -LiteralPath $keyPath -Algorithm SHA256).Hash.ToLowerInvariant()
})
Invoke-TestPacker -Extra @('-DevelopmentSnapshot', '-ApprovedInventoryPath', $badApproval) -ExpectedError 'unapproved_or_invalid_package_member'

$changed = Join-Path $injectionRoot 'corpus.rs'
[IO.File]::AppendAllText($changed, "`n// owned packaging mutation fixture`n", [Text.UTF8Encoding]::new($false))
Invoke-TestPacker -Extra @('-DevelopmentSnapshot', '-ApprovedInventoryPath', $approvalPath) -ExpectedError 'approved_package_member_changed'
$freshApproval = Write-TestApproval 'approved-changed-development.json'
$changedDevelopment = Invoke-TestPacker -Extra @('-DevelopmentSnapshot', '-ApprovedInventoryPath', $freshApproval)
if ($changedDevelopment.release_candidate) { throw 'changed_development_snapshot_claimed_as_release' }

$traversal = Write-TestApproval 'traversal-development.json' @([ordered]@{
    path = '../outside.rs'; bytes = 0; sha256 = ('0' * 64)
})
Invoke-TestPacker -Extra @('-DevelopmentSnapshot', '-ApprovedInventoryPath', $traversal) -ExpectedError 'unapproved_or_invalid_package_member'
$duplicate = Write-TestApproval 'duplicate-development.json' @([ordered]@{
    path = $releaseEntries[0].path; bytes = $releaseEntries[0].bytes; sha256 = $releaseEntries[0].sha256
})
Invoke-TestPacker -Extra @('-DevelopmentSnapshot', '-ApprovedInventoryPath', $duplicate) -ExpectedError 'unapproved_or_invalid_package_member'

$malformed = Get-Content -LiteralPath $freshApproval -Raw | ConvertFrom-Json
$malformed.entries[0].bytes = [double]$malformed.entries[0].bytes + 0.5
$malformedPath = Join-Path $fixture 'fractional-byte-count.json'
[IO.File]::WriteAllText($malformedPath, (ConvertTo-Json -InputObject $malformed -Depth 8), [Text.UTF8Encoding]::new($false))
Invoke-TestPacker -Extra @('-DevelopmentSnapshot', '-ApprovedInventoryPath', $malformedPath) -ExpectedError 'unapproved_or_invalid_package_member'
$malformed = Get-Content -LiteralPath $freshApproval -Raw | ConvertFrom-Json
$malformed.entries[0].sha256 = $malformed.entries[0].sha256.ToUpperInvariant()
$malformedPath = Join-Path $fixture 'noncanonical-digest.json'
[IO.File]::WriteAllText($malformedPath, (ConvertTo-Json -InputObject $malformed -Depth 8), [Text.UTF8Encoding]::new($false))
Invoke-TestPacker -Extra @('-DevelopmentSnapshot', '-ApprovedInventoryPath', $malformedPath) -ExpectedError 'unapproved_or_invalid_package_member'

[ordered]@{
    schema_version = 'epistesys-epr-packaging-test.v1'; cases_passed = 10
    fixed_commit_release = $true; dirty_release_rejected = $true
    explicit_development_not_release = $true; unapproved_secret_inclusion = $false
    forbidden_approved_member_rejected = $true; changed_approved_bytes_rejected = $true
    fresh_approved_change_accepted = $true; traversal_and_duplicate_rejected = $true
    fixture_root = $fixture; artifact_cleanup = 'left_owned_artifacts_no_deletion'
} | ConvertTo-Json -Compress
