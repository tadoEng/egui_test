#Requires -Version 7.0
param([Parameter(Mandatory)][string]$RunDirectory, [Parameter(Mandatory)][string]$CheckoutRoot, [Parameter(Mandatory)][string]$OwnedRootPath)
$ErrorActionPreference = 'Stop'
$checkout = $CheckoutRoot
$agentRoot = $PSScriptRoot
$state = Join-Path $RunDirectory 'tool-state.json'
$steps = [Collections.Generic.List[object]]::new()
function Invoke-Step([string]$Label, [string[]]$CargoArguments, [switch]$DirectRustc) {
    $arguments = @('-NoProfile', '-File', (Join-Path $agentRoot 'cargo-step.ps1'), '-ArgumentsJson', (ConvertTo-Json -InputObject $CargoArguments -Compress))
    if ($DirectRustc) { $arguments += '-DirectRustc' }
    & (Join-Path $agentRoot 'invoke-guarded.ps1') -Checkout $checkout -TargetDirectory (Join-Path $checkout '.codex-target') -ReportDirectory $RunDirectory -Label $Label -CommandArguments $arguments
    if ($LASTEXITCODE -ne 0) { throw "Guarded phase failed: $Label" }
    $steps.Add(@{label=$Label; assessment='PASS'})
}
try {
    . (Join-Path $agentRoot 'start-owned-cache.ps1') -RunDirectory $RunDirectory -OwnedRootPath $OwnedRootPath
    Invoke-Step 'focused-tests' @('test','-p','lab-fixture-mesh','--all-targets','--locked','--offline')
    Invoke-Step 'focused-clippy' @('clippy','-p','lab-fixture-mesh','--all-targets','--locked','--offline','--','-D','warnings')
    & (Join-Path $agentRoot 'invoke-guarded.ps1') -Checkout $checkout -TargetDirectory (Join-Path $checkout '.codex-target') -ReportDirectory $RunDirectory -Label 'canonical-native' -CommandArguments @('-NoProfile','-File','check.ps1','-Offline','-SkipWeb')
    if ($LASTEXITCODE -ne 0) { throw 'Native canonical failed' }
    $steps.Add(@{label='canonical-native';assessment='PASS'})
    Invoke-Step 'wasm-direct' @('check','--workspace','--all-targets','--all-features','--target','wasm32-unknown-unknown','--locked','--offline') -DirectRustc
    Invoke-Step 'cargo-list' @('test','--workspace','--all-targets','--all-features','--locked','--offline','--','--list')
    Invoke-Step 'cargo-ignored-list' @('test','--workspace','--all-targets','--all-features','--locked','--offline','--','--list','--ignored')
    Invoke-Step 'nextest-list' @('nextest','list','--workspace','--all-targets','--all-features','--locked','--offline','--message-format','json')
    Invoke-Step 'nextest-run' @('nextest','run','--workspace','--all-targets','--all-features','--locked','--offline','--test-threads','1')
    Invoke-Step 'manifest' @('run','-p','lab-fixture-mesh','--example','fixture_mesh_manifest','--locked','--offline')
    Invoke-Step 'cpu-budget-release' @('run','-p','lab-fixture-mesh','--example','fixture_mesh_cpu_budget','--release','--locked','--offline')
} catch {
    @{assessment='INCOMPLETE';error=$_.Exception.Message;completed_steps=@($steps.ToArray())} | ConvertTo-Json -Depth 6 | Set-Content (Join-Path $RunDirectory 'campaign.json')
    throw
} finally {
    if (Test-Path -LiteralPath $state) { & (Join-Path $agentRoot 'stop-owned-sccache.ps1') -ToolState $state -ReportDirectory $RunDirectory }
}
@{assessment='PASS';completed_steps=@($steps.ToArray());source=(& git -C $checkout rev-parse HEAD).Trim()} | ConvertTo-Json -Depth 6 | Set-Content (Join-Path $RunDirectory 'campaign.json')
