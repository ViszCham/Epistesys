# 日本語: 明示snapshotから新規の開発packageを作る。build/install/deleteは行わない。
# English: Build a fresh development package from explicit pins; never build, install or delete.
#requires -Version 7.2
[CmdletBinding()]
param(
    [string] $RepositoryRoot,
    [string] $BinaryPath,
    [string] $SourceSnapshotPath,
    [string] $Destination,
    [ValidateRange(1,4096)][int] $MaxEntries = 4096,
    [ValidateRange(1,268435456)][long] $MaxBytes = 268435456,
    [ValidateSet('development-debug-no-default-features','development-debug-semantic-gpu')]
    [string] $BuildKind = 'development-debug-no-default-features',
    [switch] $SelfTest
)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$Version = '6.9.0-beta.1'
$PackageProfile = 'development-no-default-features'
$BinaryRelative = 'scripts/lc631/bin/windows-x86_64/lc631.exe'
$Utf8 = [Text.UTF8Encoding]::new($false, $true)
$PathComparison = [StringComparison]::OrdinalIgnoreCase

function Test-BuildProfileMappings {
    $baseline = Get-BuildProfile 'development-debug-no-default-features'
    $gpu = Get-BuildProfile 'development-debug-semantic-gpu'
    Assert-BuildProfileFields $baseline $baseline
    Assert-BuildProfileFields $gpu $gpu
    if ($baseline.feature_set_digest -cne '4cbae0a673844fa42062815e58dfeaa1626e165060acf0e2e68d5b79d49eb9c7' -or
        $gpu.feature_set_digest -cne '93c2f44425eb778795ffa94a7f87dd67a6c8d2a5a0dd34deab28f27524bcd8fa') {
        throw 'package_feature_digest_golden_failed'
    }
    $negative = 0
    foreach ($mutation in @('profile','build_kind','feature_set_id','digest','unknown','duplicate','default','extra')) {
        $mixed = $gpu | ConvertTo-Json -Depth 6 -Compress | ConvertFrom-Json
        switch ($mutation) {
            'profile' { $mixed.profile = $baseline.profile }
            'build_kind' { $mixed.build_kind = $baseline.build_kind }
            'feature_set_id' { $mixed.feature_set_id = $baseline.feature_set_id }
            'digest' { $mixed.feature_set_digest = ('0' * 64) }
            'unknown' { $mixed.feature_set.features = @('unknown-feature') }
            'duplicate' { $mixed.feature_set.features = @('gpu-cubecl-wgpu','gpu-cubecl-wgpu') }
            'default' { $mixed.feature_set.default_features = $true }
            'extra' { $mixed.feature_set | Add-Member -NotePropertyName unexpected -NotePropertyValue $false }
        }
        $rejected = $false
        try { Assert-BuildProfileFields $mixed $gpu } catch { $rejected = $true }
        if (-not $rejected) { throw ('package_feature_mutant_accepted: ' + $mutation) }
        $negative++
    }
    $rejected = $false
    try { $null = Get-BuildProfile 'unknown-build-kind' } catch { $rejected = $true }
    if (-not $rejected) { throw 'package_unknown_build_kind_accepted' }
    return [pscustomobject]@{positive_profiles=2;negative_profile_cases=($negative+1)
        pure_only=$true;package_generated=$false;gpu_execution_observed=$false}
}

function Get-FeatureSetDigest($FeatureSet) {
    $canonical = ConvertTo-Json -InputObject ([ordered]@{
        default_features=$FeatureSet.default_features;features=@($FeatureSet.features)
    }) -Depth 3 -Compress
    return [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData(
        [Text.Encoding]::UTF8.GetBytes($canonical))).ToLowerInvariant()
}

function Get-BuildProfile([string] $Kind) {
    switch -CaseSensitive ($Kind) {
        'development-debug-no-default-features' {
            $profileName='development-no-default-features';$featureId='lc631-cli/no-default-features-v1'
            $features=[string[]]@();$capability='DebugNoGpu';$compat='no-default-features'
        }
        'development-debug-semantic-gpu' {
            $profileName='development-semantic-gpu';$featureId='lc631-cli/semantic-gpu-v1'
            $features=[string[]]@('gpu-cubecl-wgpu');$capability='DebugSemanticGpuBuild';$compat='gpu-cubecl-wgpu'
        }
        default { throw 'package_unknown_build_kind' }
    }
    $set=[pscustomobject]@{default_features=$false;features=$features}
    return [pscustomobject]@{profile=$profileName;build_kind=$Kind;feature_set_id=$featureId
        feature_set=$set;feature_set_digest=(Get-FeatureSetDigest $set)
        capability_profile=$capability;features_compat=$compat}
}

function Assert-BuildProfileFields($Object, $Expected) {
    if ($Object.profile -cne $Expected.profile -or $Object.build_kind -cne $Expected.build_kind -or
        $Object.feature_set_id -cne $Expected.feature_set_id) { throw 'package_build_profile_mixed' }
    $set=$Object.feature_set
    $keys=@($set.PSObject.Properties.Name)
    if ($keys.Count -ne 2 -or $keys -cnotcontains 'default_features' -or $keys -cnotcontains 'features' -or
        $set.default_features -isnot [bool] -or $set.default_features -or
        $set.features -isnot [array]) { throw 'package_feature_set_shape' }
    $seen=[Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    foreach ($feature in $set.features) {
        if ($feature -isnot [string] -or $feature -cne 'gpu-cubecl-wgpu' -or -not $seen.Add($feature)) {
            throw 'package_feature_unknown_or_duplicate'
        }
    }
    $actual=@($set.features);$expectedFeatures=@($Expected.feature_set.features)
    if ($actual.Count -ne $expectedFeatures.Count) { throw 'package_feature_set_mixed' }
    for ($index=0;$index -lt $actual.Count;$index++) {
        if ($actual[$index] -cne $expectedFeatures[$index]) { throw 'package_feature_set_mixed' }
    }
    if ($Object.feature_set_digest -isnot [string] -or $Object.feature_set_digest -cnotmatch '^[0-9a-f]{64}$' -or
        $Object.feature_set_digest -cne (Get-FeatureSetDigest $set) -or
        $Object.feature_set_digest -cne $Expected.feature_set_digest) { throw 'package_feature_digest_mismatch' }
}

function Assert-LocalAbsolute([string] $Path) {
    if ($Path -notmatch '^[A-Za-z]:[/\\]' -or $Path -match '[\x00-\x1f]' -or
        $Path.Substring(2).Contains(':')) { throw 'package_local_drive_absolute_required' }
    $drive = [IO.DriveInfo]::new($Path.Substring(0,3))
    if ($drive.DriveType -in @([IO.DriveType]::Network,[IO.DriveType]::Unknown,
        [IO.DriveType]::NoRootDirectory)) { throw 'package_network_or_unavailable_drive' }
}

function Assert-OrdinaryPath([string] $Path, [bool] $RequireFile,
    [bool] $AllowBinaryHardLink = $false) {
    Assert-LocalAbsolute $Path
    $item = Get-Item -LiteralPath $Path -Force -ErrorAction Stop
    if (($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -or
        ($RequireFile -and $item.PSIsContainer)) { throw 'package_link_or_wrong_kind' }
    if ($item.PSObject.Properties['LinkType'] -and $item.LinkType) {
        $binaryException = $AllowBinaryHardLink -and $RequireFile -and
            $item.LinkType -ceq 'HardLink' -and
            [IO.Path]::GetFullPath($Path).Equals($script:ExplicitBinaryIdentity,$PathComparison)
        if (-not $binaryException) { throw 'package_linked_file' }
    }
    $parent = if ($item.PSIsContainer) { $item.Parent } else { $item.Directory }
    $depth = 0
    while ($null -ne $parent) {
        if (++$depth -gt 128 -or ($parent.Attributes -band [IO.FileAttributes]::ReparsePoint) -or
            ($parent.PSObject.Properties['LinkType'] -and $parent.LinkType)) {
            throw 'package_reparse_ancestor'
        }
        $parent = $parent.Parent
    }
    return $item.FullName
}

function Assert-RelativeSource([string] $Path) {
    if ([string]::IsNullOrWhiteSpace($Path) -or $Path.Length -gt 240 -or
        $Path -cnotmatch '^[A-Za-z0-9_.\-/]+$' -or $Path.StartsWith('/') -or
        $Path -match '(^|/)(\.|\.\.|)(/|$)' -or $Path -match '[ .](/|$)' -or
        ($Path -split '/').Count -gt 32 -or
        $Path -match '(?i)(^|/)(CON|PRN|AUX|NUL|COM[1-9]|LPT[1-9])(\.[^/]*)?(/|$)') {
        throw 'package_unsafe_relative_path'
    }
    if ($Path -match '(?i)(^|/)(\.git|\.cache|cache|target|bin|__pycache__|models?|weights?|keys?|\.ssh|\.venv|node_modules|credentials|secrets|receipts?|receipt-root|replay-ledger)(/|$)' -or
        $Path -match '(?i)(^|/)(\.env[^/]*|id_rsa[^/]*|id_ed25519[^/]*|private[-_][^/]*)$' -or
        $Path -match '(?i)\.(exe|dll|pdb|lib|obj|so|dylib|a|o|bin|key|pem|p12|pfx|pt|pth|onnx|gguf|safetensors|pyc)$') {
        throw 'package_forbidden_source_member'
    }
    $allowedRoot = $Path -match '^(docs|schemas|hooks|skills)/' -or
        $Path -match '^scripts/lc631/(Cargo\.(toml|lock)$|crates/|workers/|fixtures/)' -or
        $Path -match '^scripts/(run-[A-Za-z0-9-]+\.ps1$|research/[A-Za-z0-9_.-]+\.(ps1|py)$)' -or
        $Path -in @('.codex-plugin/plugin.json','README.md','LICENSE','LICENSE.md','.gitattributes')
    $allowedSuffix = $Path -match '(?i)\.(rs|toml|lock|json|md|ps1|sh|py|txt|yaml|yml)$' -or
        $Path -in @('LICENSE','.gitattributes')
    if (-not $allowedRoot -or -not $allowedSuffix) { throw 'package_source_not_allowlisted' }
}

function Resolve-SourceMember([string] $Relative) {
    Assert-RelativeSource $Relative
    $path = [IO.Path]::GetFullPath((Join-Path $SourceRoot $Relative))
    if (-not $path.StartsWith($SourceRoot + [IO.Path]::DirectorySeparatorChar, $PathComparison)) {
        throw 'package_member_outside_repository'
    }
    return Assert-OrdinaryPath $path $true
}

function Assert-Pin($Pin, [long] $Limit) {
    if ($Pin.sha256 -isnot [string] -or $Pin.sha256 -cnotmatch '^[0-9a-f]{64}$' -or
        ($Pin.bytes -isnot [int] -and $Pin.bytes -isnot [long]) -or
        $Pin.bytes -lt 0 -or $Pin.bytes -gt $Limit) { throw 'package_invalid_pin' }
}

function Read-ObservedFile([string] $Path, [long] $Limit, [bool] $Text,
    [string] $CopyTo = '', [bool] $AllowBinaryHardLink = $false) {
    $safe = Assert-OrdinaryPath $Path $true $AllowBinaryHardLink
    $stream = [IO.File]::Open($safe, [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::Read)
    $output = $null
    $memory = $null
    $hash = [Security.Cryptography.IncrementalHash]::CreateHash([Security.Cryptography.HashAlgorithmName]::SHA256)
    try {
        if ($stream.Length -gt $Limit) { throw 'package_member_byte_limit' }
        if ($CopyTo) {
            $output = [IO.File]::Open($CopyTo, [IO.FileMode]::CreateNew,
                [IO.FileAccess]::Write, [IO.FileShare]::None)
        }
        if ($Text) { $memory = [IO.MemoryStream]::new() }
        $buffer = [byte[]]::new(65536)
        $total = 0L
        while (($count = $stream.Read($buffer, 0, $buffer.Length)) -gt 0) {
            if ($count -gt $Limit - $total) { throw 'package_member_growth_limit' }
            $total += $count
            $hash.AppendData($buffer, 0, $count)
            if ($null -ne $output) { $output.Write($buffer, 0, $count) }
            if ($null -ne $memory) { $memory.Write($buffer, 0, $count) }
        }
        if ($null -ne $output) { $output.Flush($true) }
        $textValue = if ($Text) { $Utf8.GetString($memory.ToArray()) } else { $null }
        return [pscustomobject]@{
            bytes = $total; sha256 = [Convert]::ToHexString($hash.GetHashAndReset()).ToLowerInvariant()
            text = $textValue
        }
    } finally {
        $stream.Dispose(); $hash.Dispose()
        if ($null -ne $output) { $output.Dispose() }
        if ($null -ne $memory) { $memory.Dispose() }
    }
}

function Assert-MatchesPin($Observed, $Pin) {
    if ($Observed.bytes -ne $Pin.bytes -or $Observed.sha256 -cne $Pin.sha256) {
        throw 'package_snapshot_content_changed'
    }
}

function Assert-PublicText([string] $Text) {
    if ($Text -match '(?i)[A-Z]:[/\\]Users[/\\][^/\\\s]+|/(home|Users)/[^/\s]+/' -or
        $Text -match 'BEGIN (RSA |EC |OPENSSH )?PRIVATE KEY') {
        throw 'package_private_material_detected'
    }
}

function Assert-WindowsX64Executable([string] $Path) {
    $safe = Assert-OrdinaryPath $Path $true $true
    $stream = [IO.File]::Open($safe,[IO.FileMode]::Open,[IO.FileAccess]::Read,[IO.FileShare]::Read)
    $reader = [IO.BinaryReader]::new($stream)
    try {
        $header = $reader.ReadBytes(64)
        if ($header.Length -ne 64 -or $header[0] -ne 0x4d -or $header[1] -ne 0x5a) {
            throw 'package_invalid_executable'
        }
        $offset = [BitConverter]::ToUInt32($header,60)
        if ($offset -lt 64 -or $offset -gt 1048576 -or $offset -gt $stream.Length - 6) {
            throw 'package_invalid_executable_header'
        }
        $stream.Position = $offset
        $pe = $reader.ReadBytes(6)
        if ($pe.Length -ne 6 -or $pe[0] -ne 0x50 -or $pe[1] -ne 0x45 -or $pe[2] -ne 0 -or $pe[3] -ne 0 -or
            [BitConverter]::ToUInt16($pe,4) -ne 0x8664) { throw 'package_wrong_executable_platform' }
    } finally { $reader.Dispose(); $stream.Dispose() }
}

function Add-DirectoryNames([string] $Relative) {
    $parts = $Relative -split '/'
    $current = ''
    for ($index = 0; $index -lt $parts.Count - 1; $index++) {
        $current = if ($current) { $current + '/' + $parts[$index] } else { $parts[$index] }
        [void] $Directories.Add($current)
    }
}

function Json-Bytes($Value) {
    return ,$Utf8.GetBytes((ConvertTo-Json -InputObject $Value -Depth 12 -Compress))
}

function Ensure-OutputParent([string] $Relative) {
    $parts = $Relative -split '/'
    $parent = $DestinationRoot
    for ($index = 0; $index -lt $parts.Count - 1; $index++) {
        $parent = Join-Path $parent $parts[$index]
        if (-not (Test-Path -LiteralPath $parent)) {
            [void] (New-Item -ItemType Directory -Path $parent -ErrorAction Stop)
        }
        $null = Assert-OrdinaryPath $parent $false
    }
    return Join-Path $DestinationRoot $Relative
}

if ($SelfTest) { Test-BuildProfileMappings; return }
$BuildProfile = Get-BuildProfile $BuildKind
$PackageProfile = $BuildProfile.profile
if (-not $IsWindows) { throw 'package_windows_profile_required' }
foreach ($absolute in @($RepositoryRoot,$BinaryPath,$SourceSnapshotPath,$Destination)) {
    Assert-LocalAbsolute $absolute
}
$script:ExplicitBinaryIdentity = [IO.Path]::GetFullPath($BinaryPath)
$SourceRoot = (Assert-OrdinaryPath $RepositoryRoot $false).TrimEnd([char[]]@('\','/'))
if (-not (Get-Item -LiteralPath $SourceRoot).PSIsContainer) { throw 'package_repository_not_directory' }
$DestinationRoot = [IO.Path]::GetFullPath($Destination).TrimEnd([char[]]@('\','/'))
if ($DestinationRoot.Equals($SourceRoot,$PathComparison) -or
    $DestinationRoot.StartsWith($SourceRoot + [IO.Path]::DirectorySeparatorChar,$PathComparison)) {
    throw 'package_destination_inside_repository'
}
if (Test-Path -LiteralPath $DestinationRoot) { throw 'package_destination_exists' }
$DestinationParent = Split-Path -Parent $DestinationRoot
$null = Assert-OrdinaryPath $DestinationParent $false
$BinaryFull = Assert-OrdinaryPath $BinaryPath $true $true
$SnapshotFull = Assert-OrdinaryPath $SourceSnapshotPath $true
$snapshotObservation = Read-ObservedFile $SnapshotFull 4194304 $true
$snapshot = ConvertFrom-Json -InputObject $snapshotObservation.text -Depth 16
if ($snapshot.schema_version -cne 'epistesys-tlcx-development-source-snapshot.v1' -or
    $snapshot.version -cne $Version -or $snapshot.profile -cne $PackageProfile -or
    @($snapshot.source_files).Count -eq 0 -or @($snapshot.source_files).Count -gt $MaxEntries - 7) {
    throw 'package_snapshot_contract_invalid'
}
Assert-BuildProfileFields $snapshot $BuildProfile
Assert-Pin $snapshot.binary 134217728
$Files = [Collections.Generic.List[object]]::new()
$Names = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
$Directories = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
$SourceRows = [Collections.Generic.List[object]]::new()
$PayloadBytes = 0L
foreach ($entry in $snapshot.source_files) {
    Assert-Pin $entry 16777216
    $relative = [string] $entry.path
    $full = Resolve-SourceMember $relative
    if (-not $Names.Add($relative)) { throw 'package_duplicate_portable_path' }
    $observed = Read-ObservedFile $full $entry.bytes $true
    Assert-MatchesPin $observed $entry
    Assert-PublicText $observed.text
    if ($entry.bytes -gt $MaxBytes - $PayloadBytes) { throw 'package_total_byte_limit' }
    $PayloadBytes += $entry.bytes
    $SourceRows.Add([ordered]@{path=$relative;sha256=$entry.sha256;bytes=$entry.bytes})
    $Files.Add([ordered]@{path=$relative;sha256=$entry.sha256;bytes=$entry.bytes;role='source'})
    Add-DirectoryNames $relative
}
$Required = @('.codex-plugin/plugin.json','hooks/hooks.json','hooks/run-stop-hook.ps1',
    'hooks/run-stop-hook.sh','skills/lc631-coding-projector/SKILL.md',
    'skills/lc631-conversation-projector/SKILL.md','scripts/run-epistesys.ps1',
    'scripts/run-lc631.ps1','scripts/lc631/Cargo.toml',
    'schemas/lc631-program-analysis.v1.schema.json','schemas/epistesys-canonical-summary.v1.schema.json')
foreach ($relative in $Required) {
    if (-not $Names.Contains($relative)) { throw ('package_required_member_missing: ' + $relative) }
}
$plugin = Read-ObservedFile (Resolve-SourceMember '.codex-plugin/plugin.json') 16777216 $true
if (($plugin.text | ConvertFrom-Json).version -cne $Version) { throw 'package_plugin_version_mismatch' }
$binaryObserved = Read-ObservedFile $BinaryFull $snapshot.binary.bytes $false '' $true
Assert-MatchesPin $binaryObserved $snapshot.binary
Assert-WindowsX64Executable $BinaryFull
if ($snapshot.binary.bytes -le 0 -or $snapshot.binary.bytes -gt $MaxBytes - $PayloadBytes) {
    throw 'package_binary_or_total_limit'
}
$PayloadBytes += $snapshot.binary.bytes
$Files.Add([ordered]@{path=$BinaryRelative;sha256=$snapshot.binary.sha256;bytes=$snapshot.binary.bytes;role='binary'})
Add-DirectoryNames $BinaryRelative
$PackageId = [guid]::NewGuid().ToString('D')
$Metadata = [ordered]@{}
$Metadata['.tlcx-development-package.json'] = Json-Bytes ([ordered]@{
    schema_version='epistesys-tlcx-development-package-marker.v1';version=$Version;profile=$PackageProfile;package_id=$PackageId
    build_kind=$BuildKind;feature_set_id=$BuildProfile.feature_set_id;feature_set=$BuildProfile.feature_set
    feature_set_digest=$BuildProfile.feature_set_digest})
$Metadata['manifests/source.json'] = Json-Bytes ([ordered]@{
    schema_version='epistesys-tlcx-development-source-manifest.v1';version=$Version;profile=$PackageProfile;files=@($SourceRows.ToArray())
    build_kind=$BuildKind;feature_set_id=$BuildProfile.feature_set_id;feature_set=$BuildProfile.feature_set
    feature_set_digest=$BuildProfile.feature_set_digest})
$Metadata['manifests/platform.json'] = Json-Bytes ([ordered]@{
    schema_version='epistesys-tlcx-development-platform-manifest.v1';version=$Version;profile=$PackageProfile
    platform='windows-x86_64';binary=[ordered]@{path=$BinaryRelative;sha256=$snapshot.binary.sha256;bytes=$snapshot.binary.bytes}
    features=$BuildProfile.features_compat;build_kind=$BuildKind;capability_profile=$BuildProfile.capability_profile
    feature_set_id=$BuildProfile.feature_set_id;feature_set=$BuildProfile.feature_set
    feature_set_digest=$BuildProfile.feature_set_digest})
$Metadata['manifests/plugin.json'] = Json-Bytes ([ordered]@{
    schema_version='epistesys-tlcx-development-plugin-manifest.v1';version=$Version;profile=$PackageProfile
    build_kind=$BuildKind;feature_set_id=$BuildProfile.feature_set_id;feature_set=$BuildProfile.feature_set
    feature_set_digest=$BuildProfile.feature_set_digest
    plugin_manifest='.codex-plugin/plugin.json';hooks_manifest='hooks/hooks.json'
    skills=@('skills/lc631-coding-projector/SKILL.md','skills/lc631-conversation-projector/SKILL.md')
    launchers=@('scripts/run-epistesys.ps1','scripts/run-lc631.ps1')})
$Metadata['manifests/runtime-reference.json'] = Json-Bytes ([ordered]@{
    schema_version='epistesys-tlcx-development-runtime-reference.v1';version=$Version;profile=$PackageProfile
    build_kind=$BuildKind;feature_set_id=$BuildProfile.feature_set_id;feature_set=$BuildProfile.feature_set
    feature_set_digest=$BuildProfile.feature_set_digest
    runtime_bundled=$false;model_weights_bundled=$false;receipt_roots_bundled=$false
    configured_nlp='UnavailableRequiresExplicitRuntime';host_integration='PendingHostObservation'})
$Metadata['package-manifest.json'] = Json-Bytes ([ordered]@{
    schema_version='epistesys-tlcx-development-package.v1';version=$Version;profile=$PackageProfile;package_id=$PackageId
    build_kind=$BuildKind;feature_set_id=$BuildProfile.feature_set_id;feature_set=$BuildProfile.feature_set
    feature_set_digest=$BuildProfile.feature_set_digest
    source_snapshot_sha256=$snapshotObservation.sha256;files=@($Files.ToArray())
    payload_entries=$Files.Count;payload_bytes=$PayloadBytes;release_ready=$false;install_ready=$false})
$TotalBytes = $PayloadBytes
foreach ($relative in $Metadata.Keys) {
    Add-DirectoryNames $relative
    $TotalBytes += $Metadata[$relative].Length
}
$TotalEntries = $Files.Count + $Metadata.Count + $Directories.Count
if ($TotalBytes -gt $MaxBytes -or $TotalEntries -gt $MaxEntries) { throw 'package_generated_total_limit' }
# Only after every preflight check, create a fresh destination. Retain partial failures.
[void] (New-Item -ItemType Directory -Path $DestinationRoot -ErrorAction Stop)
foreach ($file in $Files) {
    $target = Ensure-OutputParent $file.path
    $origin = if ($file.role -eq 'binary') { $BinaryFull } else { Resolve-SourceMember $file.path }
    $copy = Read-ObservedFile $origin $file.bytes $false $target ($file.role -ceq 'binary')
    Assert-MatchesPin $copy $file
    $readback = Read-ObservedFile $target $file.bytes $false
    Assert-MatchesPin $readback $file
}
# A separate after-copy observation detects drift; it is not atomic build provenance.
foreach ($entry in $snapshot.source_files) {
    $after = Read-ObservedFile (Resolve-SourceMember $entry.path) $entry.bytes $false
    Assert-MatchesPin $after $entry
}
Assert-MatchesPin (Read-ObservedFile $BinaryFull $snapshot.binary.bytes $false '' $true) $snapshot.binary
Assert-MatchesPin (Read-ObservedFile $SnapshotFull 4194304 $false) $snapshotObservation
foreach ($relative in $Metadata.Keys) {
    $target = Ensure-OutputParent $relative
    $output = [IO.File]::Open($target,[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::None)
    try {
        $output.Write($Metadata[$relative],0,$Metadata[$relative].Length)
        $output.Flush($true)
    } finally { $output.Dispose() }
}
[ordered]@{schema_version='epistesys-tlcx-development-package-result.v1';package_root=$DestinationRoot
    package_id=$PackageId;payload_entries=$Files.Count;total_entries=$TotalEntries;total_bytes=$TotalBytes
    binary_copies=1;build_kind=$BuildKind;release_ready=$false;install_ready=$false
    profile=$PackageProfile;feature_set_id=$BuildProfile.feature_set_id;feature_set=$BuildProfile.feature_set
    feature_set_digest=$BuildProfile.feature_set_digest
    claim_boundary='Development byte-copy checkpoint only; actual package smoke, build provenance and release/host gates are separate.'
} | ConvertTo-Json -Compress
