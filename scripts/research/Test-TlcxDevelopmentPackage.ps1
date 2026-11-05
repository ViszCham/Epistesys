#requires -Version 7.2
param(
    [string] $PackageRoot,
    [switch] $RunSmoke,
    [switch] $RunPathSmoke,
    [string] $PowerShellPath,
    [string] $ExpectedSourceSnapshotPath,
    [string] $CargoSentinelDirectory,
    [string] $CargoSentinelMarker,
    [ValidateSet('development-debug-no-default-features', 'development-debug-semantic-gpu')]
    [string] $ExpectedBuildKind = 'development-debug-no-default-features',
    [switch] $SelfTest
)

# 日本語: package内容を再読する開発検証。署名・release・host証明ではない。
# English: Re-read development package payloads; no signing, release or host proof.
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$ExpectedVersion = '6.9.0-beta.1'
$BinaryRelative = 'scripts/lc631/bin/windows-x86_64/lc631.exe'
$MetadataPaths = @('.tlcx-development-package.json', 'package-manifest.json',
    'manifests/source.json', 'manifests/platform.json', 'manifests/plugin.json',
    'manifests/runtime-reference.json')

function Get-FeatureSetJson($FeatureSet) {
    $names = @($FeatureSet.PSObject.Properties.Name)
    if ($names.Count -ne 2 -or $names -cnotcontains 'default_features' -or $names -cnotcontains 'features' -or
        $FeatureSet.default_features -isnot [bool] -or $FeatureSet.default_features -or
        $FeatureSet.features -isnot [Array]) { throw 'feature_set_shape' }
    $features = @($FeatureSet.features)
    if ($features.Count -eq 0) { return '{"default_features":false,"features":[]}' }
    if ($features.Count -ne 1 -or $features[0] -isnot [string] -or $features[0] -cne 'gpu-cubecl-wgpu') {
        throw 'unknown_or_duplicate_feature'
    }
    return '{"default_features":false,"features":["gpu-cubecl-wgpu"]}'
}

function Get-FeatureSetDigest($FeatureSet) {
    $hash = [Security.Cryptography.SHA256]::Create()
    try {
        $bytes = [Text.UTF8Encoding]::new($false).GetBytes((Get-FeatureSetJson $FeatureSet))
        return [Convert]::ToHexString($hash.ComputeHash($bytes)).ToLowerInvariant()
    } finally { $hash.Dispose() }
}

function Get-ExpectedBuildProfile([string] $Kind) {
    switch -CaseSensitive ($Kind) {
        'development-debug-no-default-features' {
            $mappedProfile = 'development-no-default-features'
            $id = 'lc631-cli/no-default-features-v1'
            $features = [pscustomobject]@{default_features=$false; features=@()}
            $label = 'DebugNoGpu'; $compiled = $false; $compat = 'no-default-features'
        }
        'development-debug-semantic-gpu' {
            $mappedProfile = 'development-semantic-gpu'
            $id = 'lc631-cli/semantic-gpu-v1'
            $features = [pscustomobject]@{default_features=$false; features=@('gpu-cubecl-wgpu')}
            $label = 'DebugSemanticGpuBuild'; $compiled = $true; $compat = 'gpu-cubecl-wgpu'
        }
        default { throw 'unknown_build_kind' }
    }
    return [pscustomobject]@{build_kind=$Kind; profile=$mappedProfile; feature_set_id=$id; feature_set=$features;
        feature_set_digest=(Get-FeatureSetDigest $features); capability_profile=$label;
        compiled_gpu_expected=$compiled; compatibility_features=$compat}
}

function Assert-BuildIdentity($Object, $Expected) {
    if ($Object.profile -cne $Expected.profile -or $Object.build_kind -cne $Expected.build_kind -or
        $Object.feature_set_id -cne $Expected.feature_set_id -or
        (Get-FeatureSetJson $Object.feature_set) -cne (Get-FeatureSetJson $Expected.feature_set) -or
        $Object.feature_set_digest -isnot [string] -or $Object.feature_set_digest -cnotmatch '^[0-9a-f]{64}$' -or
        $Object.feature_set_digest -cne (Get-FeatureSetDigest $Object.feature_set) -or
        $Object.feature_set_digest -cne $Expected.feature_set_digest) { throw 'build_feature_identity' }
}

function Assert-CompiledProfileView($Envelope, $Expected) {
    if ($Envelope.schema_version -cne 'lc631-wire.v1' -or $Envelope.version -cne $ExpectedVersion) {
        throw 'compiled_feature_envelope'
    }
    $view = $Envelope.payload
    if ($view.schema_version -cne 'epistesys-build-feature-view.v1' -or $view.version -cne $ExpectedVersion) {
        throw 'compiled_feature_schema'
    }
    foreach ($field in @('accelerator_gpu_compiled', 'runtime_semantic_gpu_compiled')) {
        if ($view.$field -isnot [bool] -or $view.$field -ne $Expected.compiled_gpu_expected) { throw 'compiled_feature_mismatch' }
    }
    foreach ($field in @('gpu_execution_observed', 'authority_created')) {
        if ($view.$field -isnot [bool] -or $view.$field) { throw 'compiled_feature_promoted_claim' }
    }
    # Compile-time cfg observations do not observe Cargo default flags, GPU work,
    # physical device availability, performance, source semantics or authority.
    return [pscustomobject]@{accelerator_gpu_compiled=$view.accelerator_gpu_compiled;
        runtime_semantic_gpu_compiled=$view.runtime_semantic_gpu_compiled;
        gpu_execution_observed=$false; authority_created=$false;
        cargo_default_flags_observed=$false}
}

function Get-LiteralSha256([string] $Text) {
    $hash = [Security.Cryptography.SHA256]::Create()
    try { return [Convert]::ToHexString($hash.ComputeHash([Text.UTF8Encoding]::new($false).GetBytes($Text))).ToLowerInvariant() }
    finally { $hash.Dispose() }
}

function Get-GivenK6SmokeCases {
    $root = '0403016602030161030162'
    $drop = '0403016601030161'
    $ns = '{"id_hex":"$N","revision_hex":"$R"}'
    $edgeDrop = '{"from_hex":"' + $root + '","to_hex":"' + $drop + '","namespace":' + $ns + '}'
    $edgeRestore = '{"from_hex":"' + $drop + '","to_hex":"' + $root + '","namespace":' + $ns + '}'
    $edgeUnsupported = '{"from_hex":"' + $root + '","to_hex":"ff","namespace":' + $ns + '}'
    $template = '{"schema_version":"epistesys-given-k6-path.v1","root_hex":"0403016602030161030162","final_hex":"$FINAL","edges":$EDGES,"namespace":' + $ns + ',"scope":{"id_hex":"$S1","baseline_hex":"$S2","root_goal_hex":"$S3","probes_hex":"$S4","policy_hex":"$S5"},"probes":[{"obligation_hex":"$O","property_hex":"$P1","path":[0],"kind":"symbol","weight_numerator":1,"weight_denominator":1,"hard":true},{"obligation_hex":"$O","property_hex":"$P2","path":[1],"kind":"symbol","weight_numerator":3,"weight_denominator":1,"hard":true}]}'
    $tokens = [ordered]@{'$N'=('06'*32);'$R'=('07'*32);'$S1'=('01'*32);'$S2'=('02'*32);'$S3'=('03'*32);
        '$S4'=('04'*32);'$S5'=('05'*32);'$O'=('0a'*32);'$P1'=('0b'*32);'$P2'=('0c'*32)}
    $cases = @(
        [pscustomobject]@{name='restore'; final_hex=$root; edges=('['+$edgeDrop+','+$edgeRestore+']');
            source_sha256='7f6ada2e5b2ae5c9a6688949c1031ee43bdc00d310473c9b5ed4942b749168fb'; bytes=1702;
            gate='pass_in_given_finite_profile'; final_loss='0'; history_count=2; evaluations=6; uncertainties=3;
            final_digest='414e496e0883ff88092ab48feaf3d9430321ed1cebba54391fc26a20d41f8c4c'},
        [pscustomobject]@{name='hard_defect'; final_hex=$drop; edges=('['+$edgeDrop+']');
            source_sha256='5c3682d3ddcf39a0bc08edaf6c94d0021bd4e2bf5a3830618d23ee0077126e7e'; bytes=1458;
            gate='fail_in_given_finite_profile'; final_loss='3/4'; history_count=1; evaluations=4; uncertainties=2;
            final_digest='05a8aaac2a9fcedc1a2d51abfd8a94b5094285f8c925504fbd825e65ff7ac256'},
        [pscustomobject]@{name='unsupported'; final_hex='ff'; edges=('['+$edgeUnsupported+']');
            source_sha256='e137536dd9fc07fdccbf8a145b63596dabfe0e725558fa0ac647504399c06367'; bytes=1430;
            gate='needs_evidence'; final_loss=$null; history_count=1; evaluations=0; uncertainties=2;
            final_digest='e885c028fbe111f0c0ccf7a2f76360c504b57d3aecc0d6481a6dce774718891d'}
    )
    foreach ($case in $cases) {
        $config = $template.Replace('$FINAL', $case.final_hex).Replace('$EDGES', $case.edges)
        foreach ($key in $tokens.Keys) { $config = $config.Replace($key, $tokens[$key]) }
        # Raw config and artifact goldens were computed independently with Python hashlib.
        if ([Text.Encoding]::UTF8.GetByteCount($config) -ne $case.bytes -or
            (Get-LiteralSha256 $config) -cne $case.source_sha256) { throw 'path_literal_golden_mismatch' }
        $case | Add-Member -NotePropertyName config -NotePropertyValue $config
    }
    return $cases
}

function Assert-GivenK6PathResult($Value, $Case) {
    $fields = @('schema_version','profile','input_basis','semantic_validation','source_revision_hex',
        'artifact_digest_scheme','root_artifact_digest_hex','final_artifact_digest_hex','original_text_to_k6_verified',
        'host_final_bound','authority_created','output_commit_allowed','history','final_observation',
        'actual_evaluation_count','unique_defect_count','defects','uncertainty_count','gate')
    $actualFields = @($Value.PSObject.Properties.Name)
    if ($actualFields.Count -ne $fields.Count -or @($fields | Where-Object { $actualFields -cnotcontains $_ }).Count -ne 0 -or
        $Value.schema_version -cne 'epistesys-given-k6-path-result.v1' -or $Value.profile -cne 'given-k6-path-v1' -or
        $Value.input_basis -cne 'given' -or $Value.semantic_validation -cne 'unverified' -or
        $Value.source_revision_hex -cne $Case.source_sha256 -or
        $Value.artifact_digest_scheme -cne 'sha256-domain-nul-lowercase-hex-v1' -or
        $Value.root_artifact_digest_hex -cne '414e496e0883ff88092ab48feaf3d9430321ed1cebba54391fc26a20d41f8c4c' -or
        $Value.final_artifact_digest_hex -cne $Case.final_digest -or $Value.gate -cne $Case.gate) { throw 'path_result_identity_or_gate' }
    foreach ($flag in @('original_text_to_k6_verified','host_final_bound','authority_created','output_commit_allowed')) {
        if ($Value.$flag -isnot [bool] -or $Value.$flag) { throw 'path_result_promoted_claim' }
    }
    if ($Value.history -isnot [Array] -or @($Value.history).Count -ne $Case.history_count -or
        $Value.actual_evaluation_count -ne $Case.evaluations -or $Value.defects -isnot [Array] -or
        $Value.unique_defect_count -ne @($Value.defects).Count -or
        $Value.uncertainty_count -ne $Case.uncertainties) { throw 'path_result_counts' }
    foreach ($field in @('actual_evaluation_count','unique_defect_count','uncertainty_count')) {
        if ($Value.$field -isnot [int] -and $Value.$field -isnot [long]) { throw 'path_result_count_type' }
    }
    $final = $Value.final_observation
    if ($final.target_artifact_digest_hex -cne $Case.final_digest) { throw 'path_final_target' }
    if ($null -eq $Case.final_loss) {
        if ($null -ne $final.loss -or $final.gap -cne 'unsupported' -or $Value.uncertainty_count -le 0) {
            throw 'path_unsupported_was_zeroed'
        }
        if ($Value.unique_defect_count -ne 0 -or $null -ne $Value.history[0].loss -or
            $Value.history[0].gap -cne 'unsupported' -or
            $Value.history[0].target_artifact_digest_hex -cne $Case.final_digest) { throw 'path_unsupported_history' }
    } else {
        if ($null -ne $final.gap -or $null -eq $final.loss -or $final.loss.lower -cne $Case.final_loss -or
            $final.loss.upper -cne $Case.final_loss -or $Value.unique_defect_count -ne 1) { throw 'path_exact_loss_or_defect' }
        $first = $Value.history[0]
        if ($first.target_artifact_digest_hex -cne '05a8aaac2a9fcedc1a2d51abfd8a94b5094285f8c925504fbd825e65ff7ac256' -or
            $null -ne $first.gap -or $first.loss.lower -cne '3/4' -or $first.loss.upper -cne '3/4') { throw 'path_drop_history' }
        if ($Case.name -ceq 'restore') {
            $restored = $Value.history[1]
            if ($restored.target_artifact_digest_hex -cne $Case.final_digest -or $null -ne $restored.gap -or
                $restored.loss.lower -cne '0' -or $restored.loss.upper -cne '0') { throw 'path_restore_history' }
        }
    }
    return [pscustomobject]@{case=$Case.name; schema_version=$Value.schema_version; gate=$Value.gate;
        source_revision_hex=$Value.source_revision_hex; input_basis='given'; semantic_validation='unverified';
        actual_evaluation_count=$Value.actual_evaluation_count; final_loss=$final.loss; final_gap=$final.gap;
        original_text_to_k6_verified=$false; host_final_bound=$false; authority_created=$false; output_commit_allowed=$false}
}

$ExpectedBuild = Get-ExpectedBuildProfile $ExpectedBuildKind
$ExpectedProfile = $ExpectedBuild.profile

function Assert-RelativePath([string] $Path) {
    if ([string]::IsNullOrWhiteSpace($Path) -or $Path.Length -gt 240 -or
        $Path -match '[\\:\x00-\x1f]' -or $Path.StartsWith('/') -or
        $Path -match '(^|/)(\.|\.\.|)(/|$)' -or $Path.EndsWith('/') -or
        $Path -match '(^|/)(\.git|\.cache|cache|target|models?|weights?|receipts?|keys?|\.ssh|\.env|\.venv|node_modules)(/|$)' -or
        $Path -match '(^|/)(con|prn|aux|nul|com[0-9]|lpt[0-9])([./]|$)' -or
        $Path -match '[. ](/|$)') { throw 'unsafe_relative_path' }
    if ($Path -ne $BinaryRelative -and $Path -match '\.(exe|dll|pdb|pem|key|pfx|p12|safetensors|gguf|pt|pth|onnx|bin|pyc)$') {
        throw 'forbidden_payload_extension'
    }
}

function Assert-PlainItem([string] $Path) {
    if (-not $IsWindows -or $Path -notmatch '^[A-Za-z]:[\\/]') { throw 'local_windows_path_required' }
    $drive = [IO.DriveInfo]::new([IO.Path]::GetPathRoot($Path))
    if ($drive.DriveType -in @([IO.DriveType]::Network, [IO.DriveType]::Unknown, [IO.DriveType]::NoRootDirectory)) {
        throw 'network_or_unknown_drive_rejected'
    }
    $item = Get-Item -LiteralPath $Path -Force
    $current = $item
    $depth = 0
    while ($null -ne $current) {
        if (++$depth -gt 128 -or ($current.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) {
            throw 'linked_or_deep_path'
        }
        if ($current.PSObject.Properties['LinkType'] -and $current.LinkType) { throw 'linked_path' }
        $current = if ($current.PSIsContainer) { $current.Parent } else { $current.Directory }
    }
    return $item
}

function Read-BoundedText([string] $Path, [int] $LimitBytes = 2MB) {
    $item = Assert-PlainItem $Path
    if ($LimitBytes -lt 1 -or $LimitBytes -gt 4MB -or $item.PSIsContainer -or $item.Length -gt $LimitBytes) { throw 'metadata_size' }
    $stream = [IO.File]::Open($item.FullName, 'Open', 'Read', 'Read')
    try {
        $buffer = [byte[]]::new($LimitBytes + 1)
        $length = 0
        while ($length -lt $buffer.Length) {
            $n = $stream.Read($buffer, $length, $buffer.Length - $length)
            if ($n -eq 0) { break }
            $length += $n
        }
        if ($length -gt $LimitBytes) { throw 'metadata_grew_over_limit' }
        $utf8 = [Text.UTF8Encoding]::new($false, $true)
        return $utf8.GetString($buffer, 0, $length)
    } finally { $stream.Dispose() }
}

function Read-BoundedJson([string] $Path, [int] $LimitBytes = 2MB) {
    return ((Read-BoundedText $Path $LimitBytes) | ConvertFrom-Json)
}

function Get-PayloadObservation([string] $Path) {
    $item = Assert-PlainItem $Path
    if ($item.PSIsContainer -or $item.Length -gt 256MB) { throw 'payload_size' }
    $stream = [IO.File]::Open($item.FullName, 'Open', 'Read', 'Read')
    $hash = [Security.Cryptography.IncrementalHash]::CreateHash([Security.Cryptography.HashAlgorithmName]::SHA256)
    try {
        $length = $stream.Length
        $buffer = [byte[]]::new(64KB)
        [long]$read = 0
        while (($count = $stream.Read($buffer, 0, $buffer.Length)) -gt 0) {
            if ($count -gt 256MB - $read) { throw 'payload_read_limit' }
            $read += $count
            $hash.AppendData($buffer, 0, $count)
        }
        $digest = [Convert]::ToHexString($hash.GetHashAndReset()).ToLowerInvariant()
        if ($read -ne $length -or $stream.Length -ne $length) { throw 'payload_changed' }
        return [pscustomobject]@{ bytes = $length; sha256 = $digest }
    } finally { $hash.Dispose(); $stream.Dispose() }
}

function Assert-Identity($Object, [string] $Schema) {
    if ($Object.schema_version -cne $Schema -or $Object.version -cne $ExpectedVersion -or
        $Object.profile -cne $ExpectedProfile) { throw 'metadata_identity' }
    Assert-BuildIdentity $Object $ExpectedBuild
}

function Test-PackageStructure([string] $Root) {
    if (-not [IO.Path]::IsPathFullyQualified($Root)) { throw 'absolute_package_root_required' }
    $rootItem = Assert-PlainItem $Root
    if (-not $rootItem.PSIsContainer) { throw 'package_root_not_directory' }
    $rootFull = $rootItem.FullName.TrimEnd([IO.Path]::DirectorySeparatorChar)
    $repo = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..')).TrimEnd([IO.Path]::DirectorySeparatorChar)
    if ($rootFull.Equals($repo, [StringComparison]::OrdinalIgnoreCase) -or
        $rootFull.StartsWith($repo + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) {
        throw 'package_must_be_outside_repository'
    }
    $marker = Read-BoundedJson (Join-Path $rootFull $MetadataPaths[0])
    $manifest = Read-BoundedJson (Join-Path $rootFull $MetadataPaths[1])
    Assert-Identity $marker 'epistesys-tlcx-development-package-marker.v1'
    Assert-Identity $manifest 'epistesys-tlcx-development-package.v1'
    $id = [guid]::Empty
    if (-not [guid]::TryParse($marker.package_id, [ref]$id) -or $id -eq [guid]::Empty -or
        $marker.package_id -cne $manifest.package_id -or $manifest.release_ready -isnot [bool] -or
        $manifest.release_ready -or $manifest.install_ready -isnot [bool] -or $manifest.install_ready) {
        throw 'invalid_development_marker'
    }
    $files = @($manifest.files)
    foreach ($field in @('payload_entries', 'payload_bytes')) {
        if ($manifest.$field -isnot [long] -and $manifest.$field -isnot [int]) { throw 'invalid_payload_total_type' }
    }
    if ($files.Count -eq 0 -or $files.Count -gt 4090) { throw 'payload_entry_cap' }
    $known = [Collections.Generic.Dictionary[string,object]]::new([StringComparer]::OrdinalIgnoreCase)
    $binaryCount = 0
    [long]$payloadBytes = 0
    foreach ($file in $files) {
        Assert-RelativePath $file.path
        if ($known.ContainsKey($file.path) -or $MetadataPaths -contains $file.path) { throw 'duplicate_payload_path' }
        if ($file.role -notin @('source', 'binary') -or $file.sha256 -cnotmatch '^[0-9a-f]{64}$' -or
            $file.bytes -isnot [long] -and $file.bytes -isnot [int] -or $file.bytes -lt 0) { throw 'invalid_payload_record' }
        if ($file.role -eq 'binary') {
            if ($file.path -cne $BinaryRelative -or $file.bytes -eq 0) { throw 'binary_location' }
            $binaryCount++
        } elseif ($file.path -eq $BinaryRelative) { throw 'binary_role' }
        $actual = Get-PayloadObservation (Join-Path $rootFull $file.path)
        if ($actual.bytes -ne $file.bytes -or $actual.sha256 -cne $file.sha256) { throw 'payload_hash_or_length' }
        if ($actual.bytes -gt 256MB - $payloadBytes) { throw 'package_byte_cap' }
        $payloadBytes += $actual.bytes
        $known.Add($file.path, $file)
    }
    if ($binaryCount -ne 1 -or $manifest.payload_entries -ne $files.Count -or $manifest.payload_bytes -ne $payloadBytes) {
        throw 'payload_totals_or_binary_count'
    }
    $required = @('.codex-plugin/plugin.json', 'hooks/hooks.json', 'hooks/run-stop-hook.ps1', 'hooks/run-stop-hook.sh',
        'skills/lc631-coding-projector/SKILL.md', 'skills/lc631-conversation-projector/SKILL.md',
        'scripts/run-epistesys.ps1', 'scripts/run-lc631.ps1', 'scripts/lc631/Cargo.toml',
        'schemas/lc631-program-analysis.v1.schema.json', 'schemas/epistesys-canonical-summary.v1.schema.json')
    foreach ($path in $required) { if (-not $known.ContainsKey($path)) { throw 'missing_required_payload' } }
    $docs = @{}
    foreach ($kind in @('source', 'platform', 'plugin', 'runtime-reference')) {
        $value = Read-BoundedJson (Join-Path $rootFull "manifests/$kind.json")
        $schema = if ($kind -eq 'runtime-reference') { 'epistesys-tlcx-development-runtime-reference.v1' }
            else { "epistesys-tlcx-development-$kind-manifest.v1" }
        Assert-Identity $value $schema
        $docs[$kind] = $value
    }
    $sourceRows = @($docs['source'].files)
    if ($sourceRows.Count -ne $files.Count - 1) { throw 'source_manifest_count' }
    $seenSource = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    foreach ($file in $sourceRows) {
        Assert-RelativePath $file.path
        if (-not $seenSource.Add($file.path) -or -not $known.ContainsKey($file.path)) { throw 'source_manifest_path' }
        $expected = $known[$file.path]
        if ($expected.role -cne 'source' -or $file.sha256 -cne $expected.sha256 -or $file.bytes -ne $expected.bytes) {
            throw 'source_manifest_binding'
        }
    }
    $platform = $docs['platform']
    $binary = $known[$BinaryRelative]
    if ($platform.platform -cne 'windows-x86_64' -or $platform.features -cne $ExpectedBuild.compatibility_features -or
        $platform.build_kind -cne $ExpectedBuildKind -or $platform.capability_profile -cne $ExpectedBuild.capability_profile -or
        $platform.binary.path -cne $BinaryRelative -or $platform.binary.sha256 -cne $binary.sha256 -or
        $platform.binary.bytes -ne $binary.bytes) { throw 'platform_manifest_binding' }
    $plugin = $docs['plugin']
    if ($plugin.plugin_manifest -cne '.codex-plugin/plugin.json' -or $plugin.hooks_manifest -cne 'hooks/hooks.json') {
        throw 'plugin_manifest_binding'
    }
    foreach ($path in @($plugin.skills) + @($plugin.launchers)) {
        Assert-RelativePath $path
        if (-not $known.ContainsKey($path)) { throw 'plugin_reference_missing' }
    }
    if (@($plugin.skills).Count -ne 2 -or @($plugin.launchers).Count -ne 2) { throw 'plugin_reference_count' }
    $runtime = $docs['runtime-reference']
    foreach ($name in @('runtime_bundled', 'model_weights_bundled', 'receipt_roots_bundled')) {
        if ($runtime.$name -isnot [bool] -or $runtime.$name) { throw 'runtime_bundle_claim' }
    }
    if ($runtime.configured_nlp -cne 'UnavailableRequiresExplicitRuntime' -or
        $runtime.host_integration -cne 'PendingHostObservation') { throw 'runtime_reference_claim' }
    if ((Read-BoundedJson (Join-Path $rootFull '.codex-plugin/plugin.json')).version -cne $ExpectedVersion) {
        throw 'plugin_version'
    }
    # Independently enumerate every file/directory; never recursively follow links.
    $pending = [Collections.Generic.Stack[string]]::new()
    $pending.Push($rootFull)
    $visited = 0
    $actualFiles = 0
    [long]$totalBytes = 0
    while ($pending.Count -gt 0) {
        $directory = $pending.Pop()
        foreach ($entry in [IO.Directory]::EnumerateFileSystemEntries($directory)) {
            if (++$visited -gt 4096) { throw 'total_entry_cap' }
            $item = Assert-PlainItem $entry
            $relative = [IO.Path]::GetRelativePath($rootFull, $item.FullName).Replace('\', '/')
            Assert-RelativePath $relative
            if ($item.PSIsContainer) { $pending.Push($item.FullName); continue }
            $actualFiles++
            if ($item.Length -gt 256MB - $totalBytes) { throw 'total_byte_cap' }
            $totalBytes += $item.Length
            if (-not $known.ContainsKey($relative) -and $MetadataPaths -notcontains $relative) { throw 'unlisted_file' }
        }
    }
    if ($actualFiles -ne $files.Count + 6) { throw 'actual_file_count' }
    if ($ExpectedSourceSnapshotPath) {
        if (-not [IO.Path]::IsPathFullyQualified($ExpectedSourceSnapshotPath)) { throw 'absolute_snapshot_required' }
        $snapshot = Read-BoundedJson $ExpectedSourceSnapshotPath 4MB
        Assert-Identity $snapshot 'epistesys-tlcx-development-source-snapshot.v1'
        if ((Get-PayloadObservation $ExpectedSourceSnapshotPath).sha256 -cne $manifest.source_snapshot_sha256 -or
            @($snapshot.source_files).Count -ne $sourceRows.Count -or $snapshot.binary.sha256 -cne $binary.sha256 -or
            $snapshot.binary.bytes -ne $binary.bytes) { throw 'expected_snapshot_binding' }
        $seenExpected = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
        foreach ($file in @($snapshot.source_files)) {
            Assert-RelativePath $file.path
            if (-not $seenExpected.Add($file.path) -or -not $known.ContainsKey($file.path)) { throw 'expected_snapshot_path' }
            $actual = $known[$file.path]
            if ($actual.role -cne 'source' -or $actual.sha256 -cne $file.sha256 -or $actual.bytes -ne $file.bytes) {
                throw 'expected_snapshot_payload'
            }
        }
    }
    $metadataHashes = [ordered]@{}
    foreach ($path in $MetadataPaths) { $metadataHashes[$path] = (Get-PayloadObservation (Join-Path $rootFull $path)).sha256 }
    return [pscustomobject]@{ root_private = $rootFull; payload_entries = $files.Count; total_entries = $visited;
        total_bytes = $totalBytes; binary_sha256 = $binary.sha256; metadata_hashes = $metadataHashes }
}

function Invoke-BoundedLauncher([string] $Root, [string[]] $CommandArguments, [switch] $BuildProfileQuery, $PathCase = $null) {
    if ($BuildProfileQuery -and $null -ne $PathCase) { throw 'ambiguous_smoke_contract' }
    $shell = if ($PowerShellPath) { (Assert-PlainItem $PowerShellPath).FullName } else { [Environment]::ProcessPath }
    if ([IO.Path]::GetFileName($shell) -ine 'pwsh.exe' -or -not $IsWindows) { throw 'smoke_requires_windows_pwsh' }
    # Structural preflight happens before ANY launcher invocation: never exercise its Cargo fallback.
    $null = Assert-PlainItem (Join-Path $Root $BinaryRelative)
    if (-not $CargoSentinelDirectory -or -not $CargoSentinelMarker) { throw 'cargo_sentinel_required' }
    if (-not [IO.Path]::IsPathFullyQualified($CargoSentinelDirectory) -or
        -not [IO.Path]::IsPathFullyQualified($CargoSentinelMarker)) { throw 'absolute_sentinel_paths_required' }
    $sentinel = Assert-PlainItem $CargoSentinelDirectory
    $markerParent = Assert-PlainItem ([IO.Path]::GetDirectoryName([IO.Path]::GetFullPath($CargoSentinelMarker)))
    if (-not $sentinel.PSIsContainer -or -not $markerParent.PSIsContainer -or
        (Test-Path -LiteralPath $CargoSentinelMarker)) { throw 'cargo_sentinel_not_fresh' }
    if ($sentinel.FullName.StartsWith($Root + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase) -or
        [IO.Path]::GetFullPath($CargoSentinelMarker).StartsWith($Root + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) {
        throw 'cargo_sentinel_must_be_external'
    }
    $null = Assert-PlainItem (Join-Path $sentinel.FullName 'cargo.cmd')
    $expectedSentinel = '@echo off' + "`r`n" + '> "%TLCX_CARGO_SENTINEL_MARKER%" echo invoked' + "`r`n" + 'exit /b 97' + "`r`n"
    if ((Read-BoundedText (Join-Path $sentinel.FullName 'cargo.cmd') 256) -cne $expectedSentinel) { throw 'cargo_sentinel_contract' }
    $start = [Diagnostics.ProcessStartInfo]::new($shell)
    foreach ($argument in @('-NoProfile', '-NonInteractive', '-File', (Join-Path $Root 'scripts/run-epistesys.ps1')) + $CommandArguments) {
        $start.ArgumentList.Add($argument)
    }
    $start.WorkingDirectory = $Root
    $start.UseShellExecute = $false
    $start.CreateNoWindow = $true
    $start.RedirectStandardInput = $true
    $start.StandardInputEncoding = [Text.UTF8Encoding]::new($false)
    $start.RedirectStandardOutput = $true
    $start.RedirectStandardError = $true
    # No inherited runtime/model/credential settings and no Cargo search path.
    $start.Environment.Clear()
    $start.Environment['SystemRoot'] = $env:SystemRoot
    $start.Environment['PATH'] = $sentinel.FullName + ';' + [IO.Path]::GetDirectoryName($shell) + ';' + (Join-Path $env:SystemRoot 'System32')
    $start.Environment['PATHEXT'] = '.COM;.EXE;.BAT;.CMD'
    $start.Environment['TLCX_CARGO_SENTINEL_MARKER'] = [IO.Path]::GetFullPath($CargoSentinelMarker)
    $start.Environment['POWERSHELL_TELEMETRY_OPTOUT'] = '1'
    $process = [Diagnostics.Process]::Start($start)
    $outBuffer = [byte[]]::new(4096)
    $errBuffer = [byte[]]::new(4096)
    $stdout = [IO.MemoryStream]::new()
    $stderr = [IO.MemoryStream]::new()
    $clock = [Diagnostics.Stopwatch]::StartNew()
    $outRead = $process.StandardOutput.BaseStream.ReadAsync($outBuffer, 0, $outBuffer.Length)
    $errRead = $process.StandardError.BaseStream.ReadAsync($errBuffer, 0, $errBuffer.Length)
    $outDone = $false
    $errDone = $false
    try {
        if ($null -ne $PathCase) {
            $inputBytes = [Text.UTF8Encoding]::new($false).GetBytes($PathCase.config)
            if ($inputBytes.Length -gt 4096 -or (Get-LiteralSha256 $PathCase.config) -cne $PathCase.source_sha256) {
                throw 'path_stdin_contract'
            }
            $write = $process.StandardInput.BaseStream.WriteAsync($inputBytes, 0, $inputBytes.Length)
            while (-not $write.IsCompleted) {
                if ($clock.ElapsedMilliseconds -ge 20000) { throw 'smoke_stdin_deadline' }
                [Threading.Thread]::Sleep(5)
            }
            $write.GetAwaiter().GetResult()
        }
        $process.StandardInput.Close() # Strict EOF; UTF-8 literal bytes only, no added newline/BOM.
        while (-not ($process.HasExited -and $outDone -and $errDone)) {
            if ($clock.ElapsedMilliseconds -ge 20000) { throw 'smoke_deadline' }
            if (Test-Path -LiteralPath $CargoSentinelMarker) { throw 'cargo_was_invoked' }
            if (-not $outDone -and $outRead.IsCompleted) {
                $n = $outRead.GetAwaiter().GetResult()
                if ($n -eq 0) { $outDone = $true } else {
                    if ($stdout.Length + $n -gt 64KB) { throw 'smoke_stdout_cap' }
                    $stdout.Write($outBuffer, 0, $n)
                    $outRead = $process.StandardOutput.BaseStream.ReadAsync($outBuffer, 0, $outBuffer.Length)
                }
            }
            if (-not $errDone -and $errRead.IsCompleted) {
                $n = $errRead.GetAwaiter().GetResult()
                if ($n -eq 0) { $errDone = $true } else {
                    if ($stderr.Length + $n -gt 16KB) { throw 'smoke_stderr_cap' }
                    $stderr.Write($errBuffer, 0, $n)
                    $errRead = $process.StandardError.BaseStream.ReadAsync($errBuffer, 0, $errBuffer.Length)
                }
            }
            [Threading.Thread]::Sleep(5)
        }
        if ($process.ExitCode -ne 0) { throw 'smoke_nonzero_exit' }
        if (Test-Path -LiteralPath $CargoSentinelMarker) { throw 'cargo_was_invoked' }
        $utf8 = [Text.UTF8Encoding]::new($false, $true)
        $summary = $utf8.GetString($stdout.ToArray()) | ConvertFrom-Json
        if ($null -ne $PathCase) {
            $checkedPath = Assert-GivenK6PathResult $summary $PathCase
            return [pscustomobject]@{command=$CommandArguments[0]; exit_code=$process.ExitCode;
                stdout_bytes=$stdout.Length; stderr_bytes=$stderr.Length; path_result=$checkedPath;
                interpretation='actual_packaged_given_k6_path_not_natural_language_or_host_final'}
        }
        if ($BuildProfileQuery) {
            $view = Assert-CompiledProfileView $summary $ExpectedBuild
            return [pscustomobject]@{command=$CommandArguments[0]; exit_code=$process.ExitCode;
                stdout_bytes=$stdout.Length; stderr_bytes=$stderr.Length; compiled_features=$view;
                interpretation='actual_binary_compile_cfg_only_not_gpu_execution_or_build_flags'}
        }
        if ($summary.schema_version -cne 'epistesys-canonical-summary.v1' -or
            $summary.status -notin @('not_connected', 'clarify', 'needs_evidence', 'hold', 'conflict')) { throw 'smoke_summary' }
        foreach ($flag in @('implementation_complete', 'authority_created', 'host_delivery_observed')) {
            if ($summary.$flag -isnot [bool] -or $summary.$flag) { throw 'smoke_promoted_claim' }
        }
        return [pscustomobject]@{ command = $CommandArguments[0]; exit_code = $process.ExitCode;
            status = $summary.status; stdout_bytes = $stdout.Length; stderr_bytes = $stderr.Length;
            interpretation = 'packaged_development_command_executed_pending_stages_retained' }
    } finally {
        if (-not $process.HasExited) { $process.Kill($true); $null = $process.WaitForExit(1000) }
        $process.StandardOutput.Dispose(); $process.StandardError.Dispose()
        $stdout.Dispose(); $stderr.Dispose(); $process.Dispose()
    }
}

function Assert-PureRejection([scriptblock] $Action) {
    $rejected = $false
    try { & $Action } catch { $rejected = $true }
    if (-not $rejected) { throw 'selftest_invalid_profile_or_view_accepted' }
}

function Test-BuildProfilesPure {
    $old = Get-ExpectedBuildProfile 'development-debug-no-default-features'
    $gpu = Get-ExpectedBuildProfile 'development-debug-semantic-gpu'
    # Independent hashlib goldens for exact UTF-8 JSON, no newline/BOM.
    if ($old.feature_set_digest -cne '4cbae0a673844fa42062815e58dfeaa1626e165060acf0e2e68d5b79d49eb9c7' -or
        $gpu.feature_set_digest -cne '93c2f44425eb778795ffa94a7f87dd67a6c8d2a5a0dd34deab28f27524bcd8fa' -or
        $old.profile -cne 'development-no-default-features' -or $old.capability_profile -cne 'DebugNoGpu' -or
        $gpu.profile -cne 'development-semantic-gpu' -or $gpu.capability_profile -cne 'DebugSemanticGpuBuild') {
        throw 'selftest_profile_golden'
    }
    foreach ($expected in @($old, $gpu)) {
        Assert-BuildIdentity $expected $expected
        $mutant = $expected | ConvertTo-Json -Depth 5 | ConvertFrom-Json
        $mutant.profile = 'wrong-profile'
        Assert-PureRejection { Assert-BuildIdentity $mutant $expected }
        $mutant = $expected | ConvertTo-Json -Depth 5 | ConvertFrom-Json
        $mutant.feature_set_id = 'wrong-feature-set'
        Assert-PureRejection { Assert-BuildIdentity $mutant $expected }
        $mutant = $expected | ConvertTo-Json -Depth 5 | ConvertFrom-Json
        $mutant.feature_set_digest = '0' * 64
        Assert-PureRejection { Assert-BuildIdentity $mutant $expected }
        $mutant = $expected | ConvertTo-Json -Depth 5 | ConvertFrom-Json
        $mutant.feature_set.default_features = $true
        Assert-PureRejection { Assert-BuildIdentity $mutant $expected }
        $mutant = $expected | ConvertTo-Json -Depth 5 | ConvertFrom-Json
        $mutant.feature_set.features = @('unknown-feature')
        Assert-PureRejection { Assert-BuildIdentity $mutant $expected }
        $mutant.feature_set.features = @('gpu-cubecl-wgpu', 'gpu-cubecl-wgpu')
        Assert-PureRejection { Assert-BuildIdentity $mutant $expected }
        $view = [pscustomobject]@{schema_version='lc631-wire.v1'; version=$ExpectedVersion;
            payload=[pscustomobject]@{schema_version='epistesys-build-feature-view.v1'; version=$ExpectedVersion;
                accelerator_gpu_compiled=$expected.compiled_gpu_expected;
                runtime_semantic_gpu_compiled=$expected.compiled_gpu_expected;
                gpu_execution_observed=$false; authority_created=$false}}
        $null = Assert-CompiledProfileView $view $expected
        $view.payload.runtime_semantic_gpu_compiled = -not $expected.compiled_gpu_expected
        Assert-PureRejection { $null = Assert-CompiledProfileView $view $expected }
        $view.payload.runtime_semantic_gpu_compiled = $expected.compiled_gpu_expected
        $view.payload.accelerator_gpu_compiled = -not $expected.compiled_gpu_expected
        Assert-PureRejection { $null = Assert-CompiledProfileView $view $expected }
        $view.payload.accelerator_gpu_compiled = $expected.compiled_gpu_expected
        $view.payload.gpu_execution_observed = $true
        Assert-PureRejection { $null = Assert-CompiledProfileView $view $expected }
        $view.payload.gpu_execution_observed = $false
        $view.payload.authority_created = $true
        Assert-PureRejection { $null = Assert-CompiledProfileView $view $expected }
        $view.payload.authority_created = $false
        $view.payload.accelerator_gpu_compiled = 'false'
        Assert-PureRejection { $null = Assert-CompiledProfileView $view $expected }
    }
    Assert-PureRejection { Assert-BuildIdentity $old $gpu }
    Assert-PureRejection { Assert-BuildIdentity $gpu $old }
    Assert-PureRejection { $null = Get-FeatureSetJson ([pscustomobject]@{default_features=$false;features=@();extra=1}) }
    Assert-PureRejection { $null = Get-ExpectedBuildProfile 'unknown-build-kind' }
    return [pscustomobject]@{positive_profile_cases=2; positive_mocked_compile_views=2;
        negative_profile_cases=16; negative_mocked_compile_views=10;
        pure_only=$true; actual_binary_queries=0; gpu_executions=0}
}

function Test-GivenK6PathPure {
    $cases = @(Get-GivenK6SmokeCases)
    $rootDigest = '414e496e0883ff88092ab48feaf3d9430321ed1cebba54391fc26a20d41f8c4c'
    $dropDigest = '05a8aaac2a9fcedc1a2d51abfd8a94b5094285f8c925504fbd825e65ff7ac256'
    foreach ($case in $cases) {
        $final = [pscustomobject]@{target_artifact_digest_hex=$case.final_digest;loss=$null;gap='unsupported'}
        $history = @([pscustomobject]@{target_artifact_digest_hex=$case.final_digest;loss=$null;gap='unsupported'})
        $defects = @()
        if ($case.name -cne 'unsupported') {
            $final = [pscustomobject]@{target_artifact_digest_hex=$case.final_digest;
                loss=[pscustomobject]@{lower=$case.final_loss;upper=$case.final_loss};gap=$null}
            $history = @([pscustomobject]@{target_artifact_digest_hex=$dropDigest;
                loss=[pscustomobject]@{lower='3/4';upper='3/4'};gap=$null})
            $defects = @([pscustomobject]@{mock_only=$true})
            if ($case.name -ceq 'restore') {
                $history += [pscustomobject]@{target_artifact_digest_hex=$rootDigest;
                    loss=[pscustomobject]@{lower='0';upper='0'};gap=$null}
            }
        }
        $mock = [pscustomobject]@{schema_version='epistesys-given-k6-path-result.v1';profile='given-k6-path-v1';
            input_basis='given';semantic_validation='unverified';source_revision_hex=$case.source_sha256;
            artifact_digest_scheme='sha256-domain-nul-lowercase-hex-v1';root_artifact_digest_hex=$rootDigest;
            final_artifact_digest_hex=$case.final_digest;original_text_to_k6_verified=$false;host_final_bound=$false;
            authority_created=$false;output_commit_allowed=$false;history=$history;final_observation=$final;
            actual_evaluation_count=$case.evaluations;unique_defect_count=$defects.Count;defects=$defects;
            uncertainty_count=$case.uncertainties;gate=$case.gate}
        $null = Assert-GivenK6PathResult $mock $case
        $mutant = $mock | ConvertTo-Json -Depth 8 | ConvertFrom-Json
        $mutant.source_revision_hex = '0' * 64
        Assert-PureRejection { $null = Assert-GivenK6PathResult $mutant $case }
        $mutant = $mock | ConvertTo-Json -Depth 8 | ConvertFrom-Json
        $mutant.gate = 'pass'
        Assert-PureRejection { $null = Assert-GivenK6PathResult $mutant $case }
        $mutant = $mock | ConvertTo-Json -Depth 8 | ConvertFrom-Json
        $mutant.semantic_validation = 'verified'
        Assert-PureRejection { $null = Assert-GivenK6PathResult $mutant $case }
        $mutant = $mock | ConvertTo-Json -Depth 8 | ConvertFrom-Json
        $mutant.authority_created = $true
        Assert-PureRejection { $null = Assert-GivenK6PathResult $mutant $case }
        $mutant = $mock | ConvertTo-Json -Depth 8 | ConvertFrom-Json
        $mutant.host_final_bound = $true
        Assert-PureRejection { $null = Assert-GivenK6PathResult $mutant $case }
        $mutant = $mock | ConvertTo-Json -Depth 8 | ConvertFrom-Json
        $mutant.final_observation.loss = [pscustomobject]@{lower='1';upper='1'}
        Assert-PureRejection { $null = Assert-GivenK6PathResult $mutant $case }
        # U entries count observations; successful numeric probe evaluations are
        # a separate metric. Unsupported observations still invoke the producer.
        foreach ($wrongUncertainty in @(0, $case.evaluations, [string]$case.uncertainties)) {
            $mutant = $mock | ConvertTo-Json -Depth 8 | ConvertFrom-Json
            $mutant.uncertainty_count = $wrongUncertainty
            Assert-PureRejection { $null = Assert-GivenK6PathResult $mutant $case }
        }
        $mutant = $mock | ConvertTo-Json -Depth 8 | ConvertFrom-Json
        $mutant.actual_evaluation_count = $case.evaluations + 1
        Assert-PureRejection { $null = Assert-GivenK6PathResult $mutant $case }
    }
    return [pscustomobject]@{literal_raw_hash_goldens=3; positive_mock_path_views=3;negative_mock_path_views=30;
        pure_only=$true;actual_path_processes=0;package_executions=0;host_observations=0}
}

if ($SelfTest) {
    $profileTests = Test-BuildProfilesPure
    $pathTests = Test-GivenK6PathPure
    $negative = @('../escape', '/absolute', 'C:/absolute', 'a//b', 'a/./b', 'a/../b', 'target/a.rs',
        'keys/private.pem', 'models/model.gguf', 'a:stream', 'a\b', 'a/nul.txt', 'a./b', 'a /b')
    foreach ($path in $negative) {
        $rejected = $false
        try { Assert-RelativePath $path } catch { $rejected = $true }
        if (-not $rejected) { throw 'selftest_unsafe_path_accepted' }
    }
    Assert-RelativePath '.codex-plugin/plugin.json'
    Assert-RelativePath 'scripts/lc631/crates/lc631-receipt-kernel/src/lib.rs'
    [pscustomobject]@{ self_test_negative_paths = $negative.Count; structural_only = $true; profile_tests=$profileTests; given_k6_path_tests=$pathTests }
    if (-not $PackageRoot) { return }
}
if ($RunPathSmoke -and -not $RunSmoke) { throw 'path_smoke_requires_run_smoke' }
if (-not $PackageRoot) { throw 'package_root_required' }
$before = Test-PackageStructure $PackageRoot
$smoke = @()
$compiledFeatureChecked = $false
$pathSmokeExecuted = 0
if ($RunSmoke) {
    $smoke += Invoke-BoundedLauncher $before.root_private @('epistesys-build-profile') -BuildProfileQuery
    $compiledFeatureChecked = $true
    $smoke += Invoke-BoundedLauncher $before.root_private @('epistesys-solve', '--prompt', 'Explain result.', '--task', 'reasoning')
    $smoke += Invoke-BoundedLauncher $before.root_private @('epistesys-inspect', '--artifact', 'development-missing')
    $smoke += Invoke-BoundedLauncher $before.root_private @('epistesys-resume', '--artifact', 'development-missing')
    if ($RunPathSmoke) {
        $pathCases = @(Get-GivenK6SmokeCases)
        if ($pathCases.Count -ne 3) { throw 'path_smoke_cases_missing' }
        foreach ($pathCase in $pathCases) {
            $smoke += Invoke-BoundedLauncher $before.root_private @('epistesys-path-check', '--config-stdin') -PathCase $pathCase
            $pathSmokeExecuted++
        }
    }
    $after = Test-PackageStructure $PackageRoot
    if ($before.binary_sha256 -cne $after.binary_sha256 -or $before.total_bytes -ne $after.total_bytes) { throw 'package_changed_during_smoke' }
    foreach ($path in $MetadataPaths) {
        if ($before.metadata_hashes[$path] -cne $after.metadata_hashes[$path]) { throw 'metadata_changed_during_smoke' }
    }
}
[pscustomobject]@{ schema_version = 'epistesys-tlcx-development-package-test.v1'; version = $ExpectedVersion;
    profile = $ExpectedProfile; payload_entries = $before.payload_entries; total_entries = $before.total_entries;
    build_kind=$ExpectedBuildKind; feature_set_id=$ExpectedBuild.feature_set_id;
    feature_set=$ExpectedBuild.feature_set; feature_set_digest=$ExpectedBuild.feature_set_digest;
    binary_compile_cfg_checked=$compiledFeatureChecked; cargo_default_flags_observed=$false; gpu_execution_observed=$false;
    given_k6_path_smoke_requested=[bool]$RunPathSmoke;
    given_k6_path_cases_executed=$pathSmokeExecuted;
    total_bytes = $before.total_bytes; binary_sha256 = $before.binary_sha256; smoke = $smoke;
    structural_payload_checked = $true; release_ready = $false; install_ready = $false;
    host_delivery_observed = $false; claim = 'development_artifact_checks_only_not_build_provenance_or_semantic_closure' } | ConvertTo-Json -Depth 6
