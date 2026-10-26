[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$Checkout,
    [Parameter(Mandatory)][string]$TargetDirectory,
    [Parameter(Mandatory)][string]$ReportDirectory,
    [Parameter(Mandatory)][ValidatePattern('^[a-z0-9-]+$')][string]$Label,
    [Parameter(Mandatory)][string[]]$CargoArguments,
    [switch]$RequireEmptyTarget,
    [int]$MinimumFreeGiB = 6
)

$ErrorActionPreference = 'Stop'
if ($env:RUSTFLAGS -or $env:CARGO_ENCODED_RUSTFLAGS) {
    throw 'Unset inherited Rust flags before a controlled measurement; do not change CRT policy.'
}
$checkoutPath = (Resolve-Path -LiteralPath $Checkout).Path
$targetPath = [IO.Path]::GetFullPath($TargetDirectory)
$reportPath = [IO.Path]::GetFullPath($ReportDirectory)
if ($RequireEmptyTarget -and (Test-Path -LiteralPath $targetPath)) {
    if (Get-ChildItem -LiteralPath $targetPath -Force | Select-Object -First 1) {
        throw 'Cold run requires a new empty target. This script never deletes a cache.'
    }
}
New-Item -ItemType Directory -Force -Path $targetPath, $reportPath | Out-Null
$summaryPath = Join-Path $reportPath "$Label.json"
if (Test-Path -LiteralPath $summaryPath) { throw 'Use a fresh label; never overwrite a prior receipt.' }

function Get-TargetBytes {
    $options = [IO.EnumerationOptions]::new()
    $options.RecurseSubdirectories = $true
    $options.IgnoreInaccessible = $true
    $options.AttributesToSkip = [IO.FileAttributes]::ReparsePoint
    [long]$total = 0
    foreach ($file in [IO.Directory]::EnumerateFiles($targetPath, '*', $options)) {
        $total += [IO.FileInfo]::new($file).Length
    }
    return $total
}

function Get-FreeBytes {
    return ([IO.DriveInfo]::new([IO.Path]::GetPathRoot($targetPath))).AvailableFreeSpace
}

if ((Get-FreeBytes) -lt ($MinimumFreeGiB * 1GB)) { throw 'Insufficient free space for a safe measured run.' }
$head = (& git -C $checkoutPath rev-parse HEAD).Trim()
if ($LASTEXITCODE -ne 0) { throw 'Cannot identify measured source commit.' }
$dirty = [bool](& git -C $checkoutPath status --porcelain --untracked-files=no)
$rust = (& rustc --version).Trim()
$runnerHash = (Get-FileHash -LiteralPath $PSCommandPath -Algorithm SHA256).Hash.ToLowerInvariant()
$beforeBytes = Get-TargetBytes
$beforeFree = Get-FreeBytes
$info = [Diagnostics.ProcessStartInfo]::new()
$info.FileName = (Get-Command cargo).Source
$info.WorkingDirectory = $checkoutPath
$info.UseShellExecute = $false
$info.CreateNoWindow = $true
$info.RedirectStandardOutput = $true
$info.RedirectStandardError = $true
foreach ($argument in $CargoArguments) { $info.ArgumentList.Add($argument) }
$settings = @{
    CARGO_TARGET_DIR = $targetPath
    CARGO_BUILD_JOBS = '1'
    CARGO_PROFILE_DEV_DEBUG = '0'
    CARGO_PROFILE_TEST_DEBUG = '0'
    CARGO_PROFILE_RELEASE_DEBUG = '0'
    CARGO_PROFILE_RELEASE_OPT_LEVEL = '2'
    CARGO_PROFILE_RELEASE_LTO = 'false'
    CARGO_PROFILE_RELEASE_INCREMENTAL = 'false'
    RUST_MIN_STACK = '268435456'
}
foreach ($entry in $settings.GetEnumerator()) { $info.Environment[$entry.Key] = $entry.Value }
$started = [DateTimeOffset]::UtcNow
$timer = [Diagnostics.Stopwatch]::StartNew()
$process = [Diagnostics.Process]::new()
$process.StartInfo = $info
$stdoutFile = [IO.FileStream]::new((Join-Path $reportPath "$Label.stdout.log"),
    [IO.FileMode]::CreateNew, [IO.FileAccess]::Write, [IO.FileShare]::Read, 1, [IO.FileOptions]::Asynchronous)
$stderrFile = [IO.FileStream]::new((Join-Path $reportPath "$Label.stderr.log"),
    [IO.FileMode]::CreateNew, [IO.FileAccess]::Write, [IO.FileShare]::Read, 1, [IO.FileOptions]::Asynchronous)
[void]$process.Start()
$stdout = $process.StandardOutput.BaseStream.CopyToAsync($stdoutFile)
$stderr = $process.StandardError.BaseStream.CopyToAsync($stderrFile)
[long]$peakSumWorkingSet = 0
[long]$peakSumPrivate = 0
[long]$minimumAvailableMemory = [long]::MaxValue
[int]$samples = 0
$samplingError = $null
$stoppedForDisk = $false
try {
    while (-not $process.HasExited) {
        if ((Get-FreeBytes) -lt ($MinimumFreeGiB * 1GB)) {
            $stoppedForDisk = $true
            $process.Kill($true) # Only this script's cargo process and its descendants.
            break
        }
        try {
            $snapshot = @(Get-CimInstance Win32_Process -Property ProcessId, ParentProcessId)
            $ids = [Collections.Generic.HashSet[int]]::new()
            [void]$ids.Add($process.Id)
            do {
                $added = $false
                foreach ($row in $snapshot) {
                    if ($ids.Contains([int]$row.ParentProcessId)) {
                        if ($ids.Add([int]$row.ProcessId)) { $added = $true }
                    }
                }
            } while ($added)
            [long]$working = 0
            [long]$private = 0
            foreach ($processId in $ids) {
                $child = Get-Process -Id $processId -ErrorAction SilentlyContinue
                if ($child) {
                    $working += $child.WorkingSet64
                    $private += $child.PrivateMemorySize64
                }
            }
            $peakSumWorkingSet = [Math]::Max($peakSumWorkingSet, $working)
            $peakSumPrivate = [Math]::Max($peakSumPrivate, $private)
            $os = Get-CimInstance Win32_OperatingSystem -Property FreePhysicalMemory
            $minimumAvailableMemory = [Math]::Min($minimumAvailableMemory, [long]$os.FreePhysicalMemory * 1KB)
            $samples++
        } catch { $samplingError = $_.Exception.Message }
        Start-Sleep -Milliseconds 1000
    }
    $process.WaitForExit()
} finally {
    if (-not $process.HasExited) { $process.Kill($true); $process.WaitForExit() }
    $timer.Stop()
    $stdout.GetAwaiter().GetResult()
    $stderr.GetAwaiter().GetResult()
    $stdoutFile.Dispose()
    $stderrFile.Dispose()
    $afterHead = (& git -C $checkoutPath rev-parse HEAD).Trim()
    $receipt = [ordered]@{
        label = $Label
        measurement_script_sha256 = $runnerHash
        source_commit = $head
        source_commit_unchanged = ($head -eq $afterHead)
        tracked_source_dirty = $dirty
        started_utc = $started.ToString('o')
        rust = $rust
        arguments = $CargoArguments
        target_directory = $targetPath
        fresh_target_required = [bool]$RequireEmptyTarget
        settings = $settings
        elapsed_seconds = ($process.ExitTime - $process.StartTime).TotalSeconds
        runner_observed_seconds = $timer.Elapsed.TotalSeconds
        exit_code = $process.ExitCode
        stopped_for_disk = $stoppedForDisk
        process_tree_samples = $samples
        sampled_peak_sum_working_set_bytes = $(if ($peakSumWorkingSet) { $peakSumWorkingSet } else { $null })
        sampled_peak_sum_private_bytes = $(if ($peakSumPrivate) { $peakSumPrivate } else { $null })
        minimum_available_system_memory_bytes = $(if ($samples) { $minimumAvailableMemory } else { $null })
        sampling_error = $samplingError
        target_bytes_before = $beforeBytes
        target_bytes_after = Get-TargetBytes
        free_bytes_before = $beforeFree
        free_bytes_after = Get-FreeBytes
        caveat = 'Approximate one-second process-tree samples; short-lived children may be missed. Working sets can double-count shared pages. Global registry/source and OS caches are not cleared. Raw local logs/paths require review before publication.'
    }
    $receipt | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath $summaryPath -Encoding utf8
    $process.Dispose()
}
Get-Content -LiteralPath $summaryPath -Raw
if ($receipt.exit_code -ne 0 -or -not $receipt.source_commit_unchanged) { exit 1 }
