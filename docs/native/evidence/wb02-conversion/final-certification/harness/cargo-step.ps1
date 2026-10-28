#Requires -Version 7.0
param([Parameter(Mandatory)][string]$ArgumentsJson, [switch]$DirectRustc)
$ErrorActionPreference = 'Stop'
if ($DirectRustc) { Remove-Item Env:RUSTC_WRAPPER -ErrorAction SilentlyContinue }
$arguments = @($ArgumentsJson | ConvertFrom-Json)
& cargo @arguments
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
