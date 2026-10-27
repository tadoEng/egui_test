# Derived guarded canonical-check runner; source-relative target + owned cache, pwsh check.ps1, fail-closed sampling. Verification only, not a benchmark.
#Requires -Version 7.0
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$Checkout,
    [Parameter(Mandatory)][string]$TargetDirectory,
    [Parameter(Mandatory)][string]$ReportDirectory,
    [Parameter(Mandatory)][ValidatePattern('^[a-z0-9-]+$')][string]$Label,
    [Parameter(Mandatory)][string[]]$CommandArguments,
    [switch]$RequireEmptyTarget,
    [int]$MinimumLaunchMemoryMiB = 6144,
    [int]$MinimumFreeGiB = 6,
    [ValidateRange(0, 1048576)][int]$MinimumAvailableMemoryMiB = 2048
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

if ($targetPath -ne [IO.Path]::GetFullPath((Join-Path $checkoutPath '.codex-target'))) { throw 'This variant requires its source worktree-local .codex-target.' }
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
function Get-InputStatus {
    $status = @(& git -C $checkoutPath status --porcelain --untracked-files=all)
    if ($LASTEXITCODE -ne 0) { throw 'Cannot inspect source checkout.' }
    $ignored = @(& git -C $checkoutPath ls-files --others --ignored --exclude-standard -- src crates examples tests benches assets .cargo Cargo.toml Cargo.lock build.rs check.ps1 rust-toolchain.toml)
    if ($LASTEXITCODE -ne 0) { throw 'Cannot inspect ignored source inputs.' }
    return [pscustomobject]@{ Dirty = [bool]$status.Count; IgnoredSource = [bool]$ignored.Count }
}

$head = (& git -C $checkoutPath rev-parse HEAD).Trim()
if ($LASTEXITCODE -ne 0) { throw 'Cannot identify measured source commit.' }
$inputStatus = Get-InputStatus
if ($inputStatus.Dirty -or $inputStatus.IgnoredSource) {
    throw 'Controlled measurements require clean committed inputs, including untracked and ignored source files. Commit an explicit disposable edit probe first.'
}
$tree = (& git -C $checkoutPath rev-parse 'HEAD^{tree}').Trim()
$lockHash = (Get-FileHash -LiteralPath (Join-Path $checkoutPath 'Cargo.lock') -Algorithm SHA256).Hash.ToLowerInvariant()
Push-Location $checkoutPath
try {
    $rust = (& rustc --version).Trim()
    $cargoVersion = (& cargo --version).Trim()
} finally { Pop-Location }
$hostInfo = [ordered]@{
    os = [Environment]::OSVersion.VersionString
    architecture = [Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString()
    logical_processors = [Environment]::ProcessorCount
    cpu = $null
    physical_memory_bytes = $null
}
try {
    $hostInfo.cpu = (Get-CimInstance Win32_Processor -Property Name | Select-Object -First 1).Name.Trim()
    $hostInfo.physical_memory_bytes = [long](Get-CimInstance Win32_ComputerSystem -Property TotalPhysicalMemory).TotalPhysicalMemory
} catch { throw 'Cannot record controlled Windows host inventory; check CIM read access.' }
$initialAvailableMemory = [long](Get-CimInstance Win32_OperatingSystem -Property FreePhysicalMemory).FreePhysicalMemory * 1KB
if ($initialAvailableMemory -lt ($MinimumLaunchMemoryMiB * 1MB)) {
    throw 'Insufficient available system memory for a safe measured run.'
}
$runnerHash = (Get-FileHash -LiteralPath $PSCommandPath -Algorithm SHA256).Hash.ToLowerInvariant()
$beforeBytes = Get-TargetBytes
$beforeFree = Get-FreeBytes
$info = [Diagnostics.ProcessStartInfo]::new()
$info.FileName = (Get-Command pwsh).Source
$info.WorkingDirectory = $checkoutPath
$info.UseShellExecute = $false
$info.CreateNoWindow = $true
$info.RedirectStandardOutput = $true
$info.RedirectStandardError = $true
foreach ($argument in $CommandArguments) { $info.ArgumentList.Add($argument) }
$settings = @{
    CARGO_TARGET_DIR = '.codex-target'
    CARGO_BUILD_JOBS = '1'
    CARGO_PROFILE_DEV_DEBUG = '0'
    CARGO_PROFILE_TEST_DEBUG = '0'
    RUST_TEST_THREADS = '1'
    CARGO_PROFILE_RELEASE_DEBUG = '0'
    RUST_MIN_STACK = '268435456'
    RUSTC_WRAPPER = $env:RUSTC_WRAPPER
    SCCACHE_DIR = $env:SCCACHE_DIR
    SCCACHE_CONF = $env:SCCACHE_CONF
    SCCACHE_SERVER_PORT = $env:SCCACHE_SERVER_PORT
    TEMP = $env:TEMP
    TMP = $env:TMP
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
$stoppedForMemory = $false
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
            $availableMemory = [long]$os.FreePhysicalMemory * 1KB
            $minimumAvailableMemory = [Math]::Min($minimumAvailableMemory, $availableMemory)
            $samples++
            if ($availableMemory -lt ($MinimumAvailableMemoryMiB * 1MB) -and -not $process.HasExited) {
                $stoppedForMemory = $true
                $process.Kill($true) # Only the owned build tree, never user applications.
                break
            }
        } catch { $samplingError = $_.Exception.Message; if (-not $process.HasExited) { $process.Kill($true) }; break }
        Start-Sleep -Milliseconds 1000
    }
    $process.WaitForExit()
} finally {
    if (-not $process.HasExited) { $process.Kill($true); $process.WaitForExit() }
    $timer.Stop()
    [void]$stdout.GetAwaiter().GetResult()
    [void]$stderr.GetAwaiter().GetResult()
    $stdoutFile.Dispose()
    $stderrFile.Dispose()
    $afterHead = (& git -C $checkoutPath rev-parse HEAD).Trim()
    $afterStatus = Get-InputStatus
    $afterLockHash = (Get-FileHash -LiteralPath (Join-Path $checkoutPath 'Cargo.lock') -Algorithm SHA256).Hash.ToLowerInvariant()
    $inputsUnchanged = ($head -eq $afterHead) -and ($lockHash -eq $afterLockHash) -and
        -not $afterStatus.Dirty -and -not $afterStatus.IgnoredSource
    $receipt = [ordered]@{
        label = $Label
        measurement_script_sha256 = $runnerHash
        source_commit = $head
        source_tree = $tree
        source_commit_after = $afterHead
        source_commit_unchanged = ($head -eq $afterHead)
        controlled_inputs_clean_before = $true
        controlled_inputs_clean_after = (-not $afterStatus.Dirty -and -not $afterStatus.IgnoredSource)
        source_inputs_unchanged = $inputsUnchanged
        lockfile_sha256_before = $lockHash
        lockfile_sha256_after = $afterLockHash
        started_utc = $started.ToString('o')
        rust = $rust
        cargo = $cargoVersion
        powershell = $PSVersionTable.PSVersion.ToString()
        dotnet = [Runtime.InteropServices.RuntimeInformation]::FrameworkDescription
        host = $hostInfo
        arguments = $CommandArguments
        target_directory = $targetPath
        fresh_target_required = [bool]$RequireEmptyTarget
        settings = $settings
        elapsed_seconds = ($process.ExitTime - $process.StartTime).TotalSeconds
        runner_observed_seconds = $timer.Elapsed.TotalSeconds
        exit_code = $process.ExitCode
        stopped_for_disk = $stoppedForDisk
        stopped_for_memory = $stoppedForMemory
        resource_policy = @{
            minimum_free_disk_bytes = $MinimumFreeGiB * 1GB
            minimum_available_system_memory_bytes = $MinimumAvailableMemoryMiB * 1MB
        }
        process_tree_samples = $samples
        sampled_peak_sum_working_set_bytes = $(if ($peakSumWorkingSet) { $peakSumWorkingSet } else { $null })
        sampled_peak_sum_private_bytes = $(if ($peakSumPrivate) { $peakSumPrivate } else { $null })
        minimum_available_system_memory_bytes = $(if ($samples) { $minimumAvailableMemory } else { $null })
        sampling_error = $samplingError
        target_bytes_before = $beforeBytes
        target_bytes_after = Get-TargetBytes
        free_bytes_before = $beforeFree
        free_bytes_after = Get-FreeBytes
        stdout_sha256 = (Get-FileHash -LiteralPath (Join-Path $reportPath "$Label.stdout.log") -Algorithm SHA256).Hash.ToLowerInvariant()
        stderr_sha256 = (Get-FileHash -LiteralPath (Join-Path $reportPath "$Label.stderr.log") -Algorithm SHA256).Hash.ToLowerInvariant()
        verification_only = $true
        caveat = 'Approximate one-second process-tree samples; short-lived children may be missed. Working sets can double-count shared pages. Global registry/source and OS caches are not cleared. Raw local logs/paths require review before publication.'
    }
    $receipt | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath $summaryPath -Encoding utf8
    $process.Dispose()
}
Get-Content -LiteralPath $summaryPath -Raw
if ($receipt.exit_code -ne 0 -or -not $receipt.source_inputs_unchanged -or
    $receipt.stopped_for_disk -or $receipt.stopped_for_memory -or $receipt.sampling_error -or $receipt.process_tree_samples -eq 0) { exit 1 }


