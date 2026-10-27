#Requires -Version 7.0
param([Parameter(Mandatory)][string]$RunDirectory, [Parameter(Mandatory)][string]$OwnedRootPath)
$ErrorActionPreference = 'Stop'
$runPath = [IO.Path]::GetFullPath($RunDirectory)
$ownedRoot = [IO.Path]::GetFullPath($OwnedRootPath).TrimEnd([IO.Path]::DirectorySeparatorChar) + [IO.Path]::DirectorySeparatorChar
if (-not $runPath.StartsWith($ownedRoot, [StringComparison]::OrdinalIgnoreCase)) { throw 'Run must stay in this agent owned directory' }
if (Test-Path (Join-Path $runPath 'tool-state.json')) { throw 'Fresh run directory required' }
if ([long](Get-CimInstance Win32_OperatingSystem -Property FreePhysicalMemory).FreePhysicalMemory * 1KB -lt 6GB) { throw 'Launch requires 6 GiB available RAM' }
if ([IO.DriveInfo]::new('D:\').AvailableFreeSpace -lt 6GB) { throw 'Launch requires 6 GiB disk' }
New-Item -ItemType Directory -Force -Path $runPath, (Join-Path $runPath 'cache'), (Join-Path $runPath 'temp') | Out-Null
$env:SCCACHE_DIR = Join-Path $runPath 'cache'
$env:TEMP = Join-Path $runPath 'temp'
$env:TMP = $env:TEMP
$env:RUSTC_WRAPPER = (Get-Command sccache).Source
$listener = [Net.Sockets.TcpListener]::new([Net.IPAddress]::Loopback, 0)
$listener.Start()
$port = $listener.LocalEndpoint.Port
$listener.Stop()
$env:SCCACHE_SERVER_PORT = "$port"
$start = [datetime]::UtcNow
& sccache --start-server *> (Join-Path $runPath 'cache-start.log')
if ($LASTEXITCODE -ne 0) { throw 'Owned cache failed to start' }
$connection = @(Get-NetTCPConnection -LocalAddress 127.0.0.1 -LocalPort $port -State Listen)
if ($connection.Count -ne 1) { throw 'Ambiguous owned listener' }
$process = Get-Process -Id $connection[0].OwningProcess
if ($process.ProcessName -ne 'sccache' -or $process.Path -ne $env:RUSTC_WRAPPER -or $process.StartTime.ToUniversalTime() -lt $start) { throw 'Owned cache process identity mismatch' }
$statsRaw = (& sccache --show-stats --stats-format json | Out-String)
if ($LASTEXITCODE -ne 0) { throw 'Cache stats failed' }
$stats = $statsRaw | ConvertFrom-Json
$actual = $stats.cache_location.Replace('\', '/') -replace '/+', '/'
$expected = $env:SCCACHE_DIR.Replace('\', '/') -replace '/+', '/'
if ($actual.IndexOf($expected, [StringComparison]::OrdinalIgnoreCase) -lt 0) { throw 'Cache directory mismatch' }
$statsRaw | Set-Content (Join-Path $runPath 'cache-before.json')
[ordered]@{ port = $port; pid = $process.Id; start_utc = $start.ToString('o'); process_start_utc = $process.StartTime.ToUniversalTime().ToString('o'); cache_directory = $env:SCCACHE_DIR; temp_directory = $env:TEMP; reported_owned_cache_verified = $true; shared_resources_touched = $false } | ConvertTo-Json | Set-Content (Join-Path $runPath 'tool-state.json')
$process.Dispose()
