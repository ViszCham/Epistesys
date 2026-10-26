param(
    [switch] $BuildAndTest,
    [string] $RepositoryRoot = (Join-Path $PSScriptRoot '..'),
    [string] $Commit = 'HEAD',
    [switch] $DevelopmentSnapshot,
    [string] $ApprovedInventoryPath
)
$ErrorActionPreference = 'Stop'
$MaxMemberBytes = 16 * 1024 * 1024
$MaxPackageBytes = 64 * 1024 * 1024
$StrictUtf8 = [Text.UTF8Encoding]::new($false, $true)
$sourceRoot = (Resolve-Path -LiteralPath $RepositoryRoot).Path
if ((Get-Item -LiteralPath $sourceRoot).Attributes -band [IO.FileAttributes]::ReparsePoint) {
    throw 'reparse_package_source_root'
}
$git = @(Get-Command git -CommandType Application -ErrorAction Stop)[0].Source

function Invoke-PackageGit {
    param([string[]] $Arguments)
    $result = @(& $git -C $sourceRoot @Arguments)
    if ($LASTEXITCODE -ne 0) { throw 'package_git_command_failed' }
    return $result
}

function Test-PackageMember {
    param([string] $Relative)
    if ([string]::IsNullOrWhiteSpace($Relative) -or
        $Relative -match '[\\:\x00-\x1f\x7f]' -or
        $Relative -match '[ .](/|$)' -or
        $Relative -match '(?i)(^|/)(CON|PRN|AUX|NUL|COM[1-9]|LPT[1-9])(\.[^/]*)?(/|$)' -or
        $Relative -match '(^|/)\.\.?(/|$)' -or $Relative.StartsWith('/') -or
        $Relative -match '(^|/)(target|\.git|\.cache|bin|credentials|secrets|receipt-root|replay-ledger|__pycache__)(/|$)' -or
        $Relative -match '(?i)\.(exe|dll|pdb|lib|obj|so|dylib|a|o|bc|spv|cubin|fatbin|bin|secret|token|key|pem|p12|pfx|kdbx|pt|pth|onnx|gguf|safetensors|pyc|pyo)$' -or
        $Relative -match '(?i)(^|/)(\.env[^/]*|id_rsa[^/]*|id_ed25519[^/]*|private[-_][^/]*\.(json|toml|yaml|yml))$') {
        return $false
    }
    return $Relative -match '^scripts/lc631/(Cargo\.(toml|lock)|crates/|workers/|fixtures/)' -or
        $Relative -match '^(schemas/|fixtures/)' -or
        $Relative -match '^validation/dgcl-(python-runtime\.v1\.json|python-requirements\.lock|nlp-model-pins\.v1\.json)$' -or
        $Relative -in @('README.md', '.codex-plugin/plugin.json', 'scripts/run-epistesys.ps1',
            'scripts/run-lc631.ps1', 'scripts/validate-dgcl-schema.py',
            'scripts/media_backend.py', 'scripts/install-media-backend.ps1', 'scripts/test_media_resources.py',
            'scripts/new-alpha2-source-package.ps1', 'scripts/test-epr-source-package.ps1',
            'scripts/test-epr-real-media.ps1',
            'docs/dgcl-operating-profile-and-closure.md', 'docs/dgcl-wire-compatibility.md')
}

function Assert-SafeLocalMember {
    param([string] $Relative)
    $full = [IO.Path]::GetFullPath((Join-Path $sourceRoot $Relative))
    $comparison = if ($IsWindows) { [StringComparison]::OrdinalIgnoreCase } else { [StringComparison]::Ordinal }
    if (-not $full.StartsWith($sourceRoot.TrimEnd([IO.Path]::DirectorySeparatorChar) + [IO.Path]::DirectorySeparatorChar, $comparison)) {
        throw 'package_member_outside_source_root'
    }
    $item = Get-Item -LiteralPath $full -Force
    if ($item.PSIsContainer -or ($item.Attributes -band [IO.FileAttributes]::ReparsePoint)) {
        throw 'non_file_or_reparse_package_member'
    }
    $parent = $item.Directory
    while ($parent -and -not $parent.FullName.Equals($sourceRoot, $comparison)) {
        if ($parent.Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'reparse_package_ancestor' }
        $parent = $parent.Parent
    }
    if (-not $parent) { throw 'package_root_not_reached' }
    return $full
}

function Read-BoundedBytes {
    param([IO.Stream] $Stream)
    $memory = [IO.MemoryStream]::new()
    try {
        $block = [byte[]]::new(65536)
        while (($count = $Stream.Read($block, 0, $block.Length)) -gt 0) {
            if ($memory.Length + $count -gt $MaxMemberBytes) { throw 'package_member_byte_budget_exceeded' }
            $memory.Write($block, 0, $count)
        }
        return ,$memory.ToArray()
    } finally { $memory.Dispose() }
}

function Read-CommitBlob {
    param([string] $Relative, [long] $ExpectedBytes)
    $info = [Diagnostics.ProcessStartInfo]::new()
    $info.FileName = $git
    $info.UseShellExecute = $false
    $info.CreateNoWindow = $true
    $info.RedirectStandardOutput = $true
    $info.RedirectStandardError = $true
    foreach ($argument in @('-C', $sourceRoot, 'cat-file', 'blob', ($resolvedCommit + ':' + $Relative))) {
        [void] $info.ArgumentList.Add($argument)
    }
    $process = [Diagnostics.Process]::new()
    $process.StartInfo = $info
    $started = $false
    $memory = [IO.MemoryStream]::new([int]$ExpectedBytes)
    try {
        [void] $process.Start()
        $started = $true
        $errorRead = $process.StandardError.ReadToEndAsync()
        $copy = $process.StandardOutput.BaseStream.CopyToAsync($memory)
        if (-not $copy.Wait(10000)) { throw 'package_git_blob_timeout' }
        if (-not $process.WaitForExit(10000)) { throw 'package_git_blob_timeout' }
        [void] $errorRead.GetAwaiter().GetResult()
        if ($process.ExitCode -ne 0 -or $memory.Length -ne $ExpectedBytes) { throw 'package_git_blob_failed' }
        return ,$memory.ToArray()
    } finally {
        if ($started -and -not $process.HasExited) { $process.Kill($true); $process.WaitForExit() }
        $memory.Dispose()
        $process.Dispose()
    }
}

function Get-BytesDigest {
    param([byte[]] $Bytes)
    return [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($Bytes)).ToLowerInvariant()
}

function Assert-NoPrivateMaterial {
    param([string] $Relative, [byte[]] $Bytes)
    $text = $StrictUtf8.GetString($Bytes)
    $excludedMarkerDigests = @('61783fba699c0ca087e473023c9e437496fa9e694f44ce8a82df9f1bc28c75b9',
        'd3cacf29cc0aa478d29093ad282739304cdd5014958aeec5e037fb9823667dc7')
    foreach ($word in @([regex]::Matches($text, '[A-Za-z]+').Value | Sort-Object -Unique)) {
        $digest = Get-BytesDigest -Bytes ([Text.Encoding]::UTF8.GetBytes($word.ToLowerInvariant()))
        if ($digest -in $excludedMarkerDigests) { throw ('private_material_in_package_member: ' + $Relative) }
    }
    if ($text -match 'C:[/\\]Users[/\\]Y[/\\]|BEGIN (RSA |EC |OPENSSH )?PRIVATE KEY') {
        throw ('private_material_in_package_member: ' + $Relative)
    }
}

if ($Commit -notmatch '^[A-Za-z0-9][A-Za-z0-9._/-]*$') { throw 'invalid_package_commit_reference' }
$resolvedCommit = (Invoke-PackageGit -Arguments @('rev-parse', '--verify', '--end-of-options', ($Commit + '^{commit}'))) -join ''
if ($resolvedCommit -notmatch '^[0-9a-f]{40}$') { throw 'invalid_resolved_package_commit' }
if ([bool]$DevelopmentSnapshot -ne (-not [string]::IsNullOrWhiteSpace($ApprovedInventoryPath))) {
    throw 'development_snapshot_requires_explicit_approved_inventory'
}
if (-not $DevelopmentSnapshot) {
    $dirty = @(Invoke-PackageGit -Arguments @('status', '--porcelain=v1', '--untracked-files=normal'))
    if ($dirty.Count -ne 0) { throw 'release_package_requires_clean_worktree' }
}

$members = [Collections.Generic.SortedDictionary[string, object]]::new([StringComparer]::Ordinal)
$portableNames = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
$approvalDigest = $null
$materialBytes = 0L
if ($DevelopmentSnapshot) {
    $approvalItem = Get-Item -LiteralPath $ApprovedInventoryPath -Force
    if ($approvalItem.PSIsContainer -or ($approvalItem.Attributes -band [IO.FileAttributes]::ReparsePoint) -or $approvalItem.Length -gt 4 * 1024 * 1024) {
        throw 'invalid_package_approval_file'
    }
    $approvalBytes = [IO.File]::ReadAllBytes($approvalItem.FullName)
    $approval = $StrictUtf8.GetString($approvalBytes) | ConvertFrom-Json
    if ($approval.schema_version -ne 'epistesys-source-package-approval.v1' -or
        $approval.base_commit -ne $resolvedCommit -or @($approval.entries).Count -lt 100 -or @($approval.entries).Count -gt 4096) {
        throw 'package_approval_binding_or_count_invalid'
    }
    $approvalDigest = Get-BytesDigest -Bytes $approvalBytes
    foreach ($entry in $approval.entries) {
        $relative = [string] $entry.path
        if (-not (Test-PackageMember $relative) -or -not $portableNames.Add($relative) -or
            $entry.sha256 -cnotmatch '^[0-9a-f]{64}$' -or
            ($entry.bytes -isnot [int] -and $entry.bytes -isnot [long]) -or
            $entry.bytes -lt 0 -or $entry.bytes -gt $MaxMemberBytes) {
            throw 'unapproved_or_invalid_package_member'
        }
        $full = Assert-SafeLocalMember $relative
        $stream = [IO.File]::Open($full, [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::Read)
        try { $bytes = Read-BoundedBytes $stream } finally { $stream.Dispose() }
        if ($bytes.Length -ne $entry.bytes -or (Get-BytesDigest $bytes) -ne $entry.sha256) {
            throw ('approved_package_member_changed: ' + $relative)
        }
        $materialBytes += $bytes.Length
        if ($materialBytes -gt $MaxPackageBytes) { throw 'source_package_total_budget_exceeded' }
        $members.Add($relative, $bytes)
    }
} else {
    $records = @(Invoke-PackageGit -Arguments @('-c', 'core.quotePath=false', 'ls-tree', '-r', '--format=%(objectmode)%x09%(objectsize)%x09%(path)', $resolvedCommit, '--'))
    foreach ($record in $records) {
        $parts = $record -split "`t", 3
        if ($parts.Count -ne 3) { throw 'invalid_commit_tree_record' }
        $relative = $parts[2]
        if (-not (Test-PackageMember $relative)) { continue }
        if ($parts[0] -notin @('100644', '100755')) { throw 'non_regular_commit_package_member' }
        $size = 0L
        if (-not [long]::TryParse($parts[1], [ref]$size) -or $size -lt 0 -or $size -gt $MaxMemberBytes) {
            throw 'package_member_byte_budget_exceeded'
        }
        $materialBytes += $size
        if ($materialBytes -gt $MaxPackageBytes) { throw 'source_package_total_budget_exceeded' }
        if (-not $portableNames.Add($relative)) { throw 'portable_package_member_collision' }
        $members.Add($relative, (Read-CommitBlob $relative $size))
    }
}
if ($members.Count -lt 100 -or $members.Count -gt 4096) { throw 'incomplete_source_package_inventory' }
$total = 0L
$entries = @(foreach ($pair in $members.GetEnumerator()) {
    [byte[]] $bytes = $pair.Value
    $total += $bytes.Length
    if ($total -gt $MaxPackageBytes) { throw 'source_package_total_budget_exceeded' }
    Assert-NoPrivateMaterial $pair.Key $bytes
    [ordered]@{ path = $pair.Key; bytes = $bytes.Length; sha256 = (Get-BytesDigest $bytes) }
})
$canonical = ConvertTo-Json -InputObject $entries -Depth 8 -Compress
$hash = Get-BytesDigest ([Text.Encoding]::UTF8.GetBytes($canonical))
$packageRoot = Join-Path ([IO.Path]::GetTempPath()) ('Epistesys source package ' + [guid]::NewGuid().ToString('N'))
if (Test-Path -LiteralPath $packageRoot) { throw 'fresh_package_destination_exists' }
[void](New-Item -ItemType Directory -Path $packageRoot)
foreach ($entry in $entries) {
    $destination = Join-Path $packageRoot $entry.path
    [void](New-Item -ItemType Directory -Force -Path (Split-Path -Parent $destination))
    $stream = [IO.File]::Open($destination, [IO.FileMode]::CreateNew, [IO.FileAccess]::Write, [IO.FileShare]::None)
    try { $stream.Write($members[$entry.path]); $stream.Flush($true) } finally { $stream.Dispose() }
    if ((Get-FileHash -LiteralPath $destination -Algorithm SHA256).Hash.ToLowerInvariant() -ne $entry.sha256) {
        throw 'copied_package_member_digest_changed'
    }
}
[IO.File]::WriteAllText((Join-Path $packageRoot 'source-package-inventory.json'),
    (ConvertTo-Json -InputObject $entries -Depth 8), [Text.UTF8Encoding]::new($false))
if ($BuildAndTest) {
    Push-Location -LiteralPath (Join-Path $packageRoot 'scripts/lc631')
    try {
        cargo build --release --locked --offline
        if ($LASTEXITCODE -ne 0) { throw 'fresh_package_release_build_failed' }
        cargo test -p lc631-cli --test dgcl_package_cli --release --locked --offline -- --test-threads=1
        if ($LASTEXITCODE -ne 0) { throw 'fresh_package_production_tests_failed' }
    } finally { Pop-Location }
}
[ordered]@{
    schema_version = 'epistesys-source-package.v1'; package_root = $packageRoot; files = $entries.Count
    source_members_digest = ('sha256:' + $hash); built_and_tested = [bool]$BuildAndTest
    model_weights_bundled = $false; trust_roots_bundled = $false
    package_mode = $(if ($DevelopmentSnapshot) { 'approved_development_snapshot' } else { 'fixed_commit_release' })
    source_commit = $resolvedCommit; approved_inventory_digest = $approvalDigest
    release_candidate = (-not $DevelopmentSnapshot)
    claim_boundary = 'source-only package; development approvals are not source-release readiness; build/test uses existing pinned toolchain and registry cache, not host activation or clean-machine provisioning'
} | ConvertTo-Json -Compress
