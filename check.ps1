[CmdletBinding()]
param(
    [switch]$SkipWeb,
    [switch]$Offline
)

$ErrorActionPreference = 'Stop'

function Invoke-Cargo {
    param([string[]]$CargoArguments)

    Write-Host "cargo $($CargoArguments -join ' ')" -ForegroundColor Cyan
    & cargo @CargoArguments
    if ($LASTEXITCODE -ne 0) {
        throw "Cargo check failed (exit $LASTEXITCODE)."
    }
}

Push-Location $PSScriptRoot
try {
    $lockedArguments = @('--locked')
    if ($Offline) {
        $lockedArguments += '--offline'
    }

    Invoke-Cargo -CargoArguments @('fmt', '--all', '--', '--check')
    Invoke-Cargo -CargoArguments (@('check', '--workspace', '--all-targets', '--all-features') + $lockedArguments)
    Invoke-Cargo -CargoArguments (@('clippy', '--workspace', '--all-targets', '--all-features') + $lockedArguments + @('--', '-D', 'warnings'))
    Invoke-Cargo -CargoArguments (@('test', '--workspace', '--all-targets', '--all-features') + $lockedArguments)
    Invoke-Cargo -CargoArguments (@('test', '--workspace', '--doc') + $lockedArguments)

    if (-not $SkipWeb) {
        Invoke-Cargo -CargoArguments (@('check', '--workspace', '--all-targets', '--all-features', '--target', 'wasm32-unknown-unknown') + $lockedArguments)
    }

    Write-Host 'All requested checks passed.' -ForegroundColor Green
}
finally {
    Pop-Location
}
