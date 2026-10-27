#Requires -Version 7.0
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$Checkout,
    [Parameter(Mandatory)][ValidatePattern('^[0-9a-f]{40}$')][string]$ExpectedSource,
    [Parameter(Mandatory)][string]$WorkspaceRoot,
    [Parameter(Mandatory)][string]$BuildDirectory,
    [Parameter(Mandatory)][string]$ReportDirectory
)
$ErrorActionPreference='Stop'
$root=(Resolve-Path -LiteralPath $WorkspaceRoot).Path
$source=(Resolve-Path -LiteralPath $Checkout).Path
$expected=$ExpectedSource
$script:controlRuns=@()
$build=Get-Content (Join-Path $BuildDirectory 'build-native-1.json') -Raw|ConvertFrom-Json
if($build.source_commit -ne $expected -or $build.exit_code -ne 0 -or !$build.source_inputs_unchanged -or $build.stopped_for_disk -or $build.stopped_for_memory){throw 'Accepted exact-source build required'}
if((& git -C $source rev-parse HEAD).Trim() -ne $expected -or @(& git -C $source status --porcelain).Count){throw 'Exact clean source required'}
$artifact=Get-Content (Join-Path $BuildDirectory 'artifact.json') -Raw|ConvertFrom-Json
$exe=Join-Path $source '.codex-target/debug/examples/native_e2k.exe'
if($artifact.source -ne $expected -or (Get-FileHash -LiteralPath $exe).Hash.ToLowerInvariant() -ne $artifact.sha256){throw 'Current-source artifact required; never use the old binary'}
if(([long](Get-CimInstance Win32_OperatingSystem -Property FreePhysicalMemory).FreePhysicalMemory*1024) -lt 6GB -or ([IO.DriveInfo]::new([IO.Path]::GetPathRoot($source))).AvailableFreeSpace -lt 6GB){throw 'Public control launch headroom unavailable; no child started'}
$evidence=[IO.Path]::GetFullPath($ReportDirectory)
if(Test-Path -LiteralPath $evidence){throw 'Fresh public report directory required'}
New-Item -ItemType Directory -Path $evidence|Out-Null
function Invoke-Owned([string[]]$Arguments,[string]$Label){
    $outPath=Join-Path $evidence "$Label.stdout.log";$errPath=Join-Path $evidence "$Label.stderr.log"
    if((Test-Path $outPath)-or(Test-Path $errPath)){throw 'Fresh log label required before launch'}
    $info=[Diagnostics.ProcessStartInfo]::new($exe)
    $info.WorkingDirectory=[IO.Path]::GetDirectoryName($exe)
    $runtimeTemp=Join-Path $root ('target/owned-pr41-runtime-'+[guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Path $runtimeTemp|Out-Null
    $info.Environment['TEMP']=$runtimeTemp;$info.Environment['TMP']=$runtimeTemp
    $info.UseShellExecute=$false;$info.CreateNoWindow=$true
    $info.RedirectStandardOutput=$true;$info.RedirectStandardError=$true
    foreach($arg in $Arguments){$info.ArgumentList.Add($arg)}
    $p=[Diagnostics.Process]::new();$p.StartInfo=$info;$started=$false
    try{
        $started=$p.Start();if(!$started){throw 'Owned process failed to start'}
        $ownedStart=$p.StartTime.ToUniversalTime()
        $stdout=$p.StandardOutput.ReadToEndAsync();$stderr=$p.StandardError.ReadToEndAsync()
        $timer=[Diagnostics.Stopwatch]::StartNew()
        while(!$p.WaitForExit(1000)){
            if($timer.Elapsed.TotalSeconds -ge 45){throw 'Native control timeout; no acceptance'}
            if(([long](Get-CimInstance Win32_OperatingSystem -Property FreePhysicalMemory).FreePhysicalMemory*1024) -lt 2GB -or ([IO.DriveInfo]::new([IO.Path]::GetPathRoot($source))).AvailableFreeSpace -lt 6GB){throw 'Public control resource guard; no acceptance'}
        }
        $output=$stdout.GetAwaiter().GetResult();$errors=$stderr.GetAwaiter().GetResult()
        if((Test-Path $outPath)-or(Test-Path $errPath)){throw 'Fresh log label required'}
        [IO.File]::WriteAllText($outPath,$output);[IO.File]::WriteAllText($errPath,$errors)
        $script:controlRuns += [ordered]@{label=$Label;exit=$p.ExitCode;self_exit=$true;stdout_sha256=(Get-FileHash $outPath).Hash.ToLowerInvariant();stderr_sha256=(Get-FileHash $errPath).Hash.ToLowerInvariant()}
        return [pscustomobject]@{exit=$p.ExitCode;stdout=$output;stderr=$errors;self_exit=$true;stdout_sha256=(Get-FileHash $outPath).Hash.ToLowerInvariant();stderr_sha256=(Get-FileHash $errPath).Hash.ToLowerInvariant()}
    }finally{
        if($started -and !$p.HasExited -and $p.StartTime.ToUniversalTime() -eq $ownedStart){$p.Kill();[void]$p.WaitForExit(10000)}
        $p.Dispose()
        $resolvedTemp=[IO.Path]::GetFullPath($runtimeTemp);$targetBase=[IO.Path]::GetFullPath((Join-Path $root 'target'))+[IO.Path]::DirectorySeparatorChar
        if(!$resolvedTemp.StartsWith($targetBase,[StringComparison]::OrdinalIgnoreCase) -or (Split-Path $resolvedTemp -Leaf) -notmatch '^owned-pr41-runtime-[0-9a-f]{32}$' -or ((Get-Item $resolvedTemp).Attributes -band [IO.FileAttributes]::ReparsePoint)){throw 'Owned runtime-temp cleanup boundary mismatch'}
        Remove-Item -LiteralPath $resolvedTemp -Recurse -Force
    }
}
function Assert-PublicCounters($report){
    if($report.fixture -ne 'LAB-REAL02-GPU-01-r1' -or $report.private_input_used -ne $false -or $report.source_entities -ne 3 -or $report.vertices -ne 52 -or $report.triangles -ne 26 -or $report.mesh_uploads -ne 1 -or $report.resident_mesh_buffers -ne 2 -or $report.vertex_bytes -ne 1872 -or $report.index_bytes -ne 312 -or $report.draw_indexed_records -lt 3 -or $report.timed_out -ne $false){throw 'Public geometry/counter contract failed'}
}
$smoke=Invoke-Owned @('--public-smoke') 'pass'
$pass=$smoke.stdout|ConvertFrom-Json
Assert-PublicCounters $pass
if($smoke.exit -ne 0 -or $pass.assessment -ne 'PASS' -or $pass.controlled_failure -ne $false){throw 'Positive smoke must self-exit0 with JSON PASS'}
$pass|ConvertTo-Json -Depth 8|Set-Content (Join-Path $evidence 'public-host-pass.json')
$forced=Invoke-Owned @('--public-smoke-force-fail') 'forced-fail'
$fail=$forced.stdout|ConvertFrom-Json
Assert-PublicCounters $fail
if($forced.exit -eq 0 -or $fail.assessment -ne 'FAIL' -or $fail.controlled_failure -ne $true){throw 'Forced smoke must self-exit nonzero with actual forced FAIL JSON'}
$missing=Invoke-Owned @('--public-smoke-missing-receipt') 'missing-receipt'
if($missing.exit -eq 0 -or $missing.stdout.Trim().Length -ne 0 -or !$missing.stderr.Contains('Public GPU smoke ended without a completed receipt')){throw 'Missing receipt must self-exit nonzero for the correct reason'}
$fixtureDir=Join-Path $root ('target/owned-pr41-public-'+[guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $fixtureDir|Out-Null
$fixturePath=Join-Path $fixtureDir 'authored.e2k'
try{
    $text=Get-Content (Join-Path $source 'crates/lab-e2k-mesh/src/public_fixture.rs') -Raw
    $literal=[regex]::Match($text,'(?s)r#"(.*)"#;')
    if(!$literal.Success){throw 'Authored public fixture extraction failed'}
    [IO.File]::WriteAllText($fixturePath,$literal.Groups[1].Value)
    $before=(Get-FileHash $fixturePath).Hash;$mtime=(Get-Item $fixturePath).LastWriteTimeUtc
    $valid=Invoke-Owned @('--check-only',$fixturePath) 'check-only'
    $noArgs=Invoke-Owned @() 'no-args'
    $edb=Invoke-Owned @('--check-only',(Join-Path $fixtureDir 'authored.edb')) 'edb'
    $extra=Invoke-Owned @('--public-smoke',$fixturePath) 'public-plus-path'
    $forceExtra=Invoke-Owned @('--public-smoke-force-fail',$fixturePath) 'forced-plus-path'
    $missingExtra=Invoke-Owned @('--public-smoke-missing-receipt',$fixturePath) 'missing-plus-path'
    if($valid.exit -ne 0 -or $noArgs.exit -eq 0 -or $edb.exit -eq 0 -or $extra.exit -eq 0 -or $forceExtra.exit -eq 0 -or $missingExtra.exit -eq 0){throw 'Public CLI refusals failed'}
    if((Get-FileHash $fixturePath).Hash -ne $before -or (Get-Item $fixturePath).LastWriteTimeUtc -ne $mtime){throw 'Owned authored fixture changed'}
}finally{
    $resolved=[IO.Path]::GetFullPath($fixtureDir);$base=[IO.Path]::GetFullPath((Join-Path $root 'target'))+[IO.Path]::DirectorySeparatorChar
    if(!$resolved.StartsWith($base,[StringComparison]::OrdinalIgnoreCase) -or (Split-Path $resolved -Leaf) -notmatch '^owned-pr41-public-[0-9a-f]{32}$'){throw 'Owned cleanup boundary mismatch'}
    if(Test-Path $fixturePath){Remove-Item -LiteralPath $fixturePath}
    if(Test-Path $fixtureDir){Remove-Item -LiteralPath $fixtureDir}
}
if((& git -C $source rev-parse HEAD).Trim() -ne $expected -or @(& git -C $source status --porcelain).Count -or (Get-FileHash $exe).Hash.ToLowerInvariant() -ne $artifact.sha256){throw 'Source or executable changed during controls'}
$receipt=[ordered]@{assessment='PASS';harness_sha256=(Get-FileHash $PSCommandPath).Hash.ToLowerInvariant();controls=$script:controlRuns;source=$expected;artifact_sha256=$artifact.sha256;artifact_bytes=$artifact.bytes;profile='Debug with debug0; not a performance comparison';smoke_exit=$smoke.exit;forced_fail_exit=$forced.exit;missing_receipt_exit=$missing.exit;forced_fail_json_verified=$true;missing_receipt_reason_verified=$true;public_counters_verified=$true;all_owned_root_self_exits_verified=$true;cli=@{check_only=$valid.exit;no_args=$noArgs.exit;edb=$edb.exit;public_plus_path=$extra.exit;forced_plus_path=$forceExtra.exit;missing_plus_path=$missingExtra.exit};public_fixture_bytes_mtime_unchanged=$true;owned_fixture_removed=(!(Test-Path $fixtureDir));private_input_used=$false;physical_input_verified=$false;presented_frames_verified=$false;scope='Current-source public native smoke exit semantics and readonly CLI controls only'}
$receipt|ConvertTo-Json -Depth 8|Set-Content (Join-Path $evidence 'validation.json')
$receipt|ConvertTo-Json -Depth 8
