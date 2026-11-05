param(
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]] $Lc631Args
)

$ErrorActionPreference = 'Stop'
$Workspace = Join-Path $PSScriptRoot 'lc631/Cargo.toml'
$Packaged = Join-Path $PSScriptRoot 'lc631/bin/windows-x86_64/lc631.exe'

if (Test-Path -LiteralPath $Packaged -PathType Leaf) {
    & $Packaged @Lc631Args
    exit $LASTEXITCODE
}

# 日本語: 通常起動から暗黙build・target生成をしない。開発時のみ明示opt-in。
# English: Normal launches never create build targets. Development is explicit.
if ($env:EPISTESYS_ALLOW_DEVELOPMENT_BUILD -cne '1') {
    throw 'Packaged Epistesys binary is unavailable. No build was started. Install a matching package, or explicitly enable the development build with a shared target.'
}
$DevelopmentTarget = $env:EPISTESYS_DEVELOPMENT_TARGET
if ([string]::IsNullOrWhiteSpace($DevelopmentTarget) -or
    -not (Split-Path -Path $DevelopmentTarget -IsAbsolute)) {
    throw 'Development builds require an absolute EPISTESYS_DEVELOPMENT_TARGET. No build was started.'
}
$DevelopmentTarget = [IO.Path]::GetFullPath($DevelopmentTarget)
$RootPath = [IO.Path]::GetPathRoot($DevelopmentTarget)
$WorkspaceDirectory = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot 'lc631'))
$RepositoryDirectory = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
foreach ($ForbiddenTarget in @($RootPath, $WorkspaceDirectory, $RepositoryDirectory, $PSScriptRoot)) {
    if ($DevelopmentTarget.TrimEnd('\', '/') -ieq $ForbiddenTarget.TrimEnd('\', '/')) {
        throw 'The development target must be a dedicated build directory, not a volume, repository or source root.'
    }
}
$TargetAncestor = $DevelopmentTarget
while (-not [string]::IsNullOrEmpty($TargetAncestor)) {
    if (Test-Path -LiteralPath $TargetAncestor) {
        $TargetItem = Get-Item -LiteralPath $TargetAncestor -Force -ErrorAction Stop
        if (-not $TargetItem.PSIsContainer -or
            ($TargetItem.Attributes -band [IO.FileAttributes]::ReparsePoint)) {
            throw 'Development target ancestors must be ordinary directories, not files or reparse points.'
        }
    }
    $TargetAncestor = [IO.Path]::GetDirectoryName($TargetAncestor)
}
if (-not (Test-Path -LiteralPath $Workspace -PathType Leaf)) {
    throw 'Development Cargo manifest is missing. No build was started.'
}
cargo run --offline --quiet --locked --manifest-path $Workspace --target-dir $DevelopmentTarget --package lc631-cli --bin lc631 -- @Lc631Args
exit $LASTEXITCODE
