param(
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]] $EpistesysArgs
)

$ErrorActionPreference = 'Stop'
$runner = Join-Path $PSScriptRoot 'run-lc631.ps1'
if (-not (Test-Path -LiteralPath $runner -PathType Leaf)) {
    throw "Epistesys runner is missing: $runner"
}
& $runner @EpistesysArgs
exit $LASTEXITCODE
