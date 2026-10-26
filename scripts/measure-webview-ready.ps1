#Requires -Version 7.0
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$Checkout,
    [Parameter(Mandatory)][string]$TargetDirectory,
    [Parameter(Mandatory)][string]$BuildManifest,
    [Parameter(Mandatory)][string]$ReportDirectory,
    [Parameter(Mandatory)][ValidatePattern('^[a-z0-9-]+$')][string]$Label,
    [ValidateRange(3,20)][int]$Runs=3,
    [ValidateRange(10,120)][int]$TimeoutSeconds=30
)

$ErrorActionPreference='Stop'
$checkoutPath=(Resolve-Path -LiteralPath $Checkout).Path
$head=(& git -C $checkoutPath rev-parse HEAD).Trim()
if($LASTEXITCODE -ne 0) { throw 'Cannot identify source' }
function Assert-CleanInputs {
    $dirty=@(& git -C $checkoutPath status --porcelain --untracked-files=all)
    if($LASTEXITCODE -ne 0) { throw 'Cannot inspect input status' }
    $ignored=@(& git -C $checkoutPath ls-files --others --ignored --exclude-standard -- src examples tests benches assets .cargo Cargo.toml Cargo.lock build.rs rust-toolchain.toml)
    if($LASTEXITCODE -ne 0 -or $dirty.Count -or $ignored.Count) { throw 'Startup probes require clean committed source inputs' }
}
Assert-CleanInputs
$manifest=Get-Content -LiteralPath $BuildManifest -Raw | ConvertFrom-Json
$receiptPath=$BuildManifest -replace '\.artifacts\.json$','.json'
if($receiptPath -eq $BuildManifest) { throw 'Require an artifacts manifest and its controlled build receipt' }
$receipt=Get-Content -LiteralPath $receiptPath -Raw | ConvertFrom-Json
if($manifest.source_commit -ne $head -or $receipt.source_commit -ne $head -or $receipt.exit_code -ne 0 -or !$receipt.source_inputs_unchanged -or !$receipt.controlled_inputs_clean_before -or !$receipt.controlled_inputs_clean_after -or !$receipt.source_commit_unchanged) {
    throw 'Stale or uncontrolled build source: compile the exact telemetry commit first'
}
$artifact=@($manifest.artifacts | Where-Object artifact -eq 'release/examples/webview_stage_a.exe')
if($artifact.Count -ne 1) { throw 'Require exactly the synthetic WebView example; never launch the structural GUI' }
$binary=(Resolve-Path -LiteralPath (Join-Path $TargetDirectory 'release/examples/webview_stage_a.exe')).Path
$hash=(Get-FileHash -LiteralPath $binary -Algorithm SHA256).Hash.ToLowerInvariant()
if($hash -ne $artifact[0].sha256 -or (Get-Item -LiteralPath $binary).Length -ne $artifact[0].bytes) { throw 'Stale binary hash/size' }
$reportPath=Join-Path ([IO.Path]::GetFullPath($ReportDirectory)) $Label
if(Test-Path -LiteralPath $reportPath) { throw 'Fresh report label required; never reuse readiness markers' }
New-Item -ItemType Directory -Path $reportPath | Out-Null
$samples=[Collections.Generic.List[object]]::new()
for($run=0;$run -le $Runs;$run++) {
    $readyPath=Join-Path $reportPath "ready-$run.json"
    $smokePath=Join-Path $reportPath "smoke-$run.json"
    $start=[Diagnostics.ProcessStartInfo]::new($binary)
    $start.UseShellExecute=$false
    $start.CreateNoWindow=$true
    $start.WorkingDirectory=$reportPath
    $start.Environment['WEBVIEW_READY_REPORT']=$readyPath
    $start.Environment['WEBVIEW_SMOKE_REPORT']=$smokePath
    $process=[Diagnostics.Process]::new()
    $process.StartInfo=$start
    $startedOk=$false
    try {
        if(!$process.Start()) { throw 'Cannot start synthetic example' }
        $startedOk=$true
        $started=$process.StartTime.ToUniversalTime()
        if(!$process.WaitForExit($TimeoutSeconds*1000)) {
            $process.Kill($true)
            $process.WaitForExit()
            throw 'Owned synthetic smoke timed out; only its process tree was stopped'
        }
        if($process.ExitCode -ne 0) { throw 'Synthetic smoke failed' }
        $ready=Get-Content -LiteralPath $readyPath -Raw | ConvertFrom-Json
        $smoke=Get-Content -LiteralPath $smokePath -Raw | ConvertFrom-Json
        if($ready.version -ne 1 -or $ready.fixture -ne 'stage-a-synthetic' -or $ready.event -ne 'rust_observed_chart_ready' -or $smoke.assessment -ne 'PASS') {
            throw 'Missing/invalid validated-ready marker or failed programmatic roundtrip'
        }
        $readyTime=[DateTimeOffset]::FromUnixTimeMilliseconds([long]$ready.ready_at_unix_ms)
        $latency=($readyTime.UtcDateTime-$started).TotalMilliseconds
        $duration=($process.ExitTime.ToUniversalTime()-$started).TotalMilliseconds
        if($latency -lt 0 -or $latency -gt $duration -or $duration -gt $TimeoutSeconds*1000+1000) { throw 'Inconsistent readiness/process timestamps' }
        $samples.Add([ordered]@{run=$run;warmup=($run -eq 0);process_to_rust_chart_ready_ms=$latency;whole_smoke_ms=$duration;webview2=$smoke.webview2;manual_input_tested=$smoke.manual_input_tested;network_capture_performed=$smoke.network_capture_performed;ready_sha256=(Get-FileHash $readyPath -Algorithm SHA256).Hash.ToLowerInvariant();smoke_sha256=(Get-FileHash $smokePath -Algorithm SHA256).Hash.ToLowerInvariant()})
    } finally {
        if($startedOk -and !$process.HasExited) { $process.Kill($true); $process.WaitForExit() }
        $process.Dispose()
    }
}
Assert-CleanInputs
if((& git -C $checkoutPath rev-parse HEAD).Trim() -ne $head) { throw 'Source changed during run' }
if((Get-FileHash -LiteralPath $binary -Algorithm SHA256).Hash.ToLowerInvariant() -ne $hash) { throw 'Binary changed during run' }
$result=[ordered]@{
    label=$Label;source_commit=$head;binary_sha256=$hash;measurement_script_sha256=(Get-FileHash $PSCommandPath -Algorithm SHA256).Hash.ToLowerInvariant()
    powershell=$PSVersionTable.PSVersion.ToString();dotnet=[Runtime.InteropServices.RuntimeInformation]::FrameworkDescription;os=[Environment]::OSVersion.VersionString
    definition='OS process start to first Rust-validated chart_ready IPC; not first presented frame, API project readiness or whole smoke duration'
    cache_policy='One warmup then fresh processes; incognito WebView configuration, machine/OS/runtime caches not cleared; not a cold browser-profile claim'
    memory='Not measured by this readiness helper; do not infer runtime RSS from build working sets'
    samples=$samples.ToArray()
}
$result | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath (Join-Path $reportPath 'summary.json')
$result
