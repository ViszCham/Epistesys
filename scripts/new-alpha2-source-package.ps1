param([switch] $BuildAndTest)
$ErrorActionPreference = 'Stop'
$sourceRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
$packageRoot = Join-Path ([IO.Path]::GetTempPath()) ('Epistesys source package ' + [guid]::NewGuid().ToString('N'))
if (Test-Path -LiteralPath $packageRoot) { throw 'fresh package destination already exists' }
$paths = @(& git -C $sourceRoot ls-files --cached --others --exclude-standard)
if ($LASTEXITCODE -ne 0) { throw 'source file inventory failed' }
$selected = @($paths | Where-Object { $_ -notmatch '(^|/)__pycache__/|\.pyc$' } | Where-Object {
    $_ -match '^scripts/lc631/(Cargo\.(toml|lock)|crates/|workers/|fixtures/)' -or
    $_ -match '^(schemas/|fixtures/)' -or
    $_ -match '^validation/dgcl-(python-runtime\.v1\.json|python-requirements\.lock|nlp-model-pins\.v1\.json)$' -or
    $_ -in @('README.md', '.codex-plugin/plugin.json', 'scripts/run-epistesys.ps1', 'scripts/run-lc631.ps1',
        'scripts/validate-dgcl-schema.py', 'docs/dgcl-operating-profile-and-closure.md', 'docs/dgcl-wire-compatibility.md')
} | Sort-Object -Unique)
if ($selected.Count -lt 100) { throw 'incomplete source package inventory' }
$entries = @()
$excludedMarkerDigests = @('61783fba699c0ca087e473023c9e437496fa9e694f44ce8a82df9f1bc28c75b9',
    'd3cacf29cc0aa478d29093ad282739304cdd5014958aeec5e037fb9823667dc7')
foreach ($relative in $selected) {
    if ($relative -match '(^|/)(target|\.git|\.cache|bin|credentials|secrets|receipt-root|replay-ledger)(/|$)' -or
        $relative -match '\.(exe|dll|pdb|secret|token|pt|pth|onnx)$' -or $relative -match '(^|/)\.\.(/|$)') {
        throw 'excluded file entered package allowlist'
    }
    $source = [IO.Path]::GetFullPath((Join-Path $sourceRoot $relative))
    if (-not $source.StartsWith($sourceRoot + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) { throw 'source escaped repository' }
    $item = Get-Item -LiteralPath $source
    if ($item.PSIsContainer -or ($item.Attributes -band [IO.FileAttributes]::ReparsePoint)) { throw 'non-file/reparse package member' }
    $parent = $item.Directory
    while ($parent -and $parent.FullName -ne $sourceRoot) {
        if ($parent.Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'reparse package ancestor' }
        $parent = $parent.Parent
    }
    $text = [IO.File]::ReadAllText($source)
    $excludedMarker = $false
    foreach ($word in @([regex]::Matches($text, '[A-Za-z]+').Value | Sort-Object -Unique)) {
        $wordDigest = [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes($word.ToLowerInvariant()))).ToLowerInvariant()
        if ($wordDigest -in $excludedMarkerDigests) { $excludedMarker = $true; break }
    }
    if ($excludedMarker -or $text -match 'C:[/\\]Users[/\\]Y[/\\]|BEGIN (RSA |EC |OPENSSH )?PRIVATE KEY') { throw "private material in package member: $relative" }
    $entries += [ordered]@{ path=$relative; bytes=$item.Length; sha256=(Get-FileHash -LiteralPath $source -Algorithm SHA256).Hash.ToLowerInvariant() }
}
New-Item -ItemType Directory -Path $packageRoot | Out-Null
foreach ($entry in $entries) {
    $destination = Join-Path $packageRoot $entry.path
    if (Test-Path -LiteralPath $destination) { throw 'fresh package member collision' }
    New-Item -ItemType Directory -Force -Path (Split-Path -Parent $destination) | Out-Null
    Copy-Item -LiteralPath (Join-Path $sourceRoot $entry.path) -Destination $destination
    if ((Get-FileHash -LiteralPath $destination -Algorithm SHA256).Hash.ToLowerInvariant() -ne $entry.sha256) { throw 'copied member digest changed' }
}
$canonical = $entries | ConvertTo-Json -Depth 8 -Compress
$hash = [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes($canonical))).ToLowerInvariant()
[IO.File]::WriteAllText((Join-Path $packageRoot 'source-package-inventory.json'), ($entries | ConvertTo-Json -Depth 8), [Text.UTF8Encoding]::new($false))
if ($BuildAndTest) {
    Push-Location -LiteralPath (Join-Path $packageRoot 'scripts/lc631')
    try {
        cargo build --release --locked --offline
        if ($LASTEXITCODE -ne 0) { throw 'fresh-package release build failed' }
        cargo test -p lc631-cli --test dgcl_package_cli --release --locked --offline -- --test-threads=1
        if ($LASTEXITCODE -ne 0) { throw 'fresh-package production tests failed' }
    } finally { Pop-Location }
}
[ordered]@{ schema_version='epistesys-source-package.v1'; package_root=$packageRoot; files=$entries.Count;
    source_members_digest=('sha256:' + $hash); built_and_tested=[bool]$BuildAndTest;
    model_weights_bundled=$false; trust_roots_bundled=$false; claim_boundary='source-only package; existing pinned toolchain/registry cache; not host installation or clean-machine provisioning' } | ConvertTo-Json -Compress
