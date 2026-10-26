#Requires -Version 7.0
[CmdletBinding()]
param([Parameter(Mandatory)][string]$ScratchDirectory)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
if (Test-Path -LiteralPath $ScratchDirectory) { throw 'Use a fresh test scratch directory' }
$scratch = New-Item -ItemType Directory -Path $ScratchDirectory
$fixture = Join-Path $PSScriptRoot '../docs/native/evidence/dashboard/release-dashboard.json'
$gate = Join-Path $PSScriptRoot 'assert-dashboard-smoke.ps1'
$results = [System.Collections.Generic.List[object]]::new()
function Test-Case([string]$Name, [scriptblock]$Mutate, [bool]$ShouldPass = $false) {
    $report = Get-Content -LiteralPath $fixture -Raw | ConvertFrom-Json -AsHashtable
    $path = Join-Path $scratch.FullName "$Name.json"
    $started = [DateTimeOffset]::UtcNow
    $exitCode = 0
    [void](& $Mutate $report)
    $report | ConvertTo-Json -Depth 20 | Set-Content -LiteralPath $path
    if ($Name -eq 'missing') { Remove-Item -LiteralPath $path }
    if ($Name -eq 'malformed') { Set-Content -LiteralPath $path -Value '{broken' }
    if ($Name -eq 'empty') { Set-Content -LiteralPath $path -Value '' }
    if ($Name -eq 'oversized') { Set-Content -LiteralPath $path -Value ('x' * (1MB + 1)) }
    if ($Name -eq 'stale') { (Get-Item -LiteralPath $path).LastWriteTimeUtc = $started.UtcDateTime.AddMinutes(-1) }
    if ($Name -eq 'exit-failure') { $exitCode = 3 }
    $finished = [DateTimeOffset]::UtcNow
    if ($Name -eq 'reversed-interval') { $started = $finished.AddMinutes(1) }
    if ($Name -eq 'future-interval') { $finished = $finished.AddMinutes(1) }
    $passed = $false
    $failure = $null
    try {
        $value = & $gate -ReportPath $path -ProcessExitCode $exitCode -RunStartedUtc $started -RunFinishedUtc $finished
        $passed = $value.assessment -ceq 'PASS'
    } catch { $failure = $_.Exception.Message }
    if ($passed -ne $ShouldPass) { throw "Unexpected outcome for ${Name}: $failure" }
    $results.Add([pscustomobject]@{ case=$Name; expected_accept=$ShouldPass; accepted=$passed; assessment='PASS' })
}
Test-Case 'valid-copy' {} $true
foreach ($name in 'missing', 'malformed', 'empty', 'oversized', 'stale', 'exit-failure', 'reversed-interval', 'future-interval') { Test-Case $name {} }
Test-Case 'explicit-fail' { param($r) $r.assessment = 'FAIL' }
Test-Case 'error' { param($r) $r.error = 'fixture failure' }
Test-Case 'missing-error' { param($r) $r.Remove('error') }
Test-Case 'manual-overclaim' { param($r) $r.manual_input_tested = $true }
Test-Case 'network-overclaim' { param($r) $r.network_capture_performed = $true }
Test-Case 'present-overclaim' { param($r) $r.presented_frames_measured = $true }
Test-Case 'wrong-phase' { param($r) $r.phase = 3 }
Test-Case 'wrong-selected' { param($r) $r.selected_story = 10 }
Test-Case 'zero-callbacks' { param($r) $r.native_callback_paints = 0 }
Test-Case 'no-stale-refusal' { param($r) $r.rejected = 0 }
Test-Case 'numeric-string' { param($r) $r.native_callback_paints = '46' }
Test-Case 'numeric-boolean' { param($r) $r.rejected = $true }
Test-Case 'flag-string' { param($r) $r.manual_input_tested = 'false' }
Test-Case 'missing-runtime' { param($r) $r.webview2 = $null }
Test-Case 'missing-snapshot' { param($r) $r.snapshots = @($r.snapshots[0], $r.snapshots[1]) }
Test-Case 'wrong-dataset' { param($r) $r.snapshots[1].dataset = 'Other' }
Test-Case 'stale-generation' { param($r) $r.snapshots[2].generation = 1 }
Test-Case 'one-chart' { param($r) $r.snapshots[0].plots = 1 }
Test-Case 'wrong-count' { param($r) $r.snapshots[0].rows = 68 }
Test-Case 'wrong-unit-value' { param($r) $r.snapshots[1].drift[0][0] = 1.0 }
Test-Case 'wrong-story-order' { param($r) $r.snapshots[1].shear[0][1] = 12 }
Test-Case 'missing-points' { param($r) $r.snapshots[2].shear = @() }
Test-Case 'wrong-geometry' { param($r) $r.native_core_triangles = 99 }
[pscustomobject]@{
    assessment='PASS'; cases=$results.ToArray()
    scope='Offline copies/mutations of public synthetic report; no app launched and no new Windows interaction evidence'
} | ConvertTo-Json -Depth 6
