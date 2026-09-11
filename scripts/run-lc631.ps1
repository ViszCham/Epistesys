param(
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]] $Lc631Args
)

$ErrorActionPreference = 'Stop'
$Workspace = Join-Path $PSScriptRoot 'lc631/Cargo.toml'
$Packaged = Join-Path $PSScriptRoot 'lc631/bin/windows-x86_64/lc631.exe'

if (Test-Path -LiteralPath $Packaged -PathType Leaf) {
    & $Packaged @Lc631Args
} else {
    cargo run --quiet --locked --manifest-path $Workspace -- @Lc631Args
}
exit $LASTEXITCODE
