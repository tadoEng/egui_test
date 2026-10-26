#Requires -Version 7.0
<#
Validates an already completed, owned hidden-dashboard run. This does not launch
or terminate processes, establish profile isolation, or certify manual input.
The caller must refuse a pre-existing report path BEFORE launching, then supply
the actual process start/finish timestamps and exit code. Timestamp checks alone
are not proof that a report was produced by that process.
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$ReportPath,
    [Parameter(Mandatory)][int]$ProcessExitCode,
    [Parameter(Mandatory)][DateTimeOffset]$RunStartedUtc,
    [Parameter(Mandatory)][DateTimeOffset]$RunFinishedUtc
)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
if ($ProcessExitCode -ne 0) { throw "Dashboard exited with code $ProcessExitCode" }
if ($RunStartedUtc -gt $RunFinishedUtc -or $RunFinishedUtc -gt [DateTimeOffset]::UtcNow) {
    throw 'Invalid run time interval'
}
$file = Get-Item -LiteralPath $ReportPath -ErrorAction Stop
if ($file.PSIsContainer -or $file.Length -eq 0 -or $file.Length -gt 1MB) {
    throw 'Report must be a nonempty JSON file of at most 1 MiB'
}
if ($file.LastWriteTimeUtc -lt $RunStartedUtc.UtcDateTime -or
    $file.LastWriteTimeUtc -gt $RunFinishedUtc.UtcDateTime) {
    throw 'Report modification time lies outside the supplied run interval'
}
$beforeHash = (Get-FileHash -LiteralPath $file.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
$report = Get-Content -LiteralPath $file.FullName -Raw | ConvertFrom-Json -AsHashtable
if ($report -isnot [System.Collections.IDictionary]) { throw 'Report must be a JSON object' }
function Require-Equal($Actual, $Expected, [string]$Field) {
    if ($null -eq $Actual -or $Actual.GetType() -ne $Expected.GetType()) {
        throw "Wrong type or missing field: $Field"
    }
    if ($Actual -cne $Expected) { throw "Unexpected value: $Field" }
}
function Require-Number($Actual, [double]$Expected, [double]$Tolerance, [string]$Field) {
    if (($Actual -isnot [long] -and $Actual -isnot [int] -and
         $Actual -isnot [double] -and $Actual -isnot [decimal]) -or
        -not [double]::IsFinite([double]$Actual) -or
        [Math]::Abs([double]$Actual - $Expected) -gt $Tolerance) {
        throw "Invalid numeric value: $Field"
    }
}
Require-Equal $report.assessment 'PASS' 'assessment'
if (-not $report.Contains('error') -or $null -ne $report.error) { throw 'Report contains or omits error' }
foreach ($field in 'manual_input_tested', 'network_capture_performed', 'presented_frames_measured') {
    Require-Equal $report[$field] $false $field
}
Require-Number $report.phase 4 0 'phase'
Require-Number $report.selected_story 11 0 'selected_story'
Require-Number $report.native_core_vertices 196 0 'native_core_vertices'
Require-Number $report.native_core_triangles 98 0 'native_core_triangles'
foreach ($field in 'rejected', 'native_callback_paints') {
    Require-Number $report[$field] ([double]$report[$field]) 0 $field
    if ($report[$field] -lt 1 -or [Math]::Truncate($report[$field]) -ne $report[$field]) {
        throw "Expected positive integer: $field"
    }
}
if ($report.webview2 -isnot [string] -or $report.webview2 -notmatch '^\d+\.\d+\.\d+\.\d+$') {
    throw 'Missing WebView2 runtime version'
}
if ($report.snapshots -isnot [array] -or $report.snapshots.Count -ne 3) { throw 'Expected three snapshots' }
for ($i = 0; $i -lt 3; $i++) {
    $snapshot = $report.snapshots[$i]
    if ($snapshot -isnot [System.Collections.IDictionary]) { throw 'Invalid snapshot object' }
    $first = if ($i -eq 0) { 1 } else { 10 }
    $last = if ($i -eq 0) { 69 } else { 12 }
    $generation = if ($i -eq 0) { 1 } else { 2 }
    $selected = @(59, 10, 11)[$i]
    Require-Equal $snapshot.dataset 'LAB-DASH-01' "snapshot[$i].dataset"
    foreach ($pair in @(@('first', $first), @('last', $last), @('generation', $generation),
                        @('selected_story', $selected), @('plots', 2), @('rows', ($last - $first + 1)))) {
        Require-Number $snapshot[$pair[0]] $pair[1] 0 "snapshot[$i].$($pair[0])"
    }
    foreach ($kind in 'drift', 'shear') {
        $points = $snapshot[$kind]
        if ($points -isnot [array] -or $points.Count -ne $last - $first + 1) { throw "Wrong $kind point count" }
        for ($j = 0; $j -lt $points.Count; $j++) {
            $point = $points[$j]
            if ($point -isnot [array] -or $point.Count -ne 2) { throw 'Expected [value, story] point' }
            $story = $first + $j
            $expected = if ($kind -eq 'drift') {
                if ($i -eq 0) { $story * 0.1 } else { $story * 0.0001 }
            } else {
                if ($i -eq 0) { (70 - $story) * 10 } else { (70 - $story) * 10000 }
            }
            # Authored signal/display contract, never an ext-calc parity test.
            $tolerance = if ($kind -eq 'drift') { 1e-12 } else { 1e-6 }
            Require-Number $point[0] $expected $tolerance "snapshot[$i].$kind[$j].value"
            Require-Number $point[1] $story 0 "snapshot[$i].$kind[$j].story"
        }
    }
}
# Detect a changed report while it was being validated. Callers must retain the
# report, launcher receipt and exact executable/source identities together.
$hash = (Get-FileHash -LiteralPath $file.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
$after = Get-Item -LiteralPath $file.FullName
if ($hash -cne $beforeHash -or $after.LastWriteTimeUtc -ne $file.LastWriteTimeUtc -or $after.Length -ne $file.Length) {
    throw 'Report changed during validation'
}
[pscustomobject]@{
    assessment = 'PASS'
    scope = 'Hidden synthetic report consistency only; not a new execution, manual input, network or presented-frame proof'
    report_sha256 = $hash
    process_exit_code = $ProcessExitCode
    supplied_run_started_utc = $RunStartedUtc.ToUniversalTime().ToString('O')
    supplied_run_finished_utc = $RunFinishedUtc.ToUniversalTime().ToString('O')
    manual_input_tested = $false
    network_capture_performed = $false
    presented_frames_measured = $false
}
