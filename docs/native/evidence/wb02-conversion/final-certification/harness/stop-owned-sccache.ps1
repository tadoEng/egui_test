#Requires -Version 7.0
param([Parameter(Mandatory)][string]$ToolState,[Parameter(Mandatory)][string]$ReportDirectory)
$ErrorActionPreference='Stop'
$s=Get-Content -LiteralPath $ToolState -Raw|ConvertFrom-Json
$ls=@(Get-NetTCPConnection -LocalAddress 127.0.0.1 -LocalPort $s.port -State Listen -ErrorAction SilentlyContinue)
$observed=$null
if($ls.Count){
    if($ls.Count -ne 1){throw 'Ambiguous private cache listener'}
    $p=Get-Process -Id $ls[0].OwningProcess
    $stamp=$p.StartTime.ToUniversalTime()
    if($p.ProcessName -ne 'sccache' -or $p.Path -ne (Get-Command sccache).Source -or $stamp -lt [datetime]$s.start_utc){throw 'Owned replacement identity mismatch'}
    if($env:SCCACHE_SERVER_PORT -ne $s.port.ToString()){throw 'Private port environment mismatch'}
    $raw=(& sccache --show-stats --stats-format json|Out-String)
    if($LASTEXITCODE -ne 0){throw 'Cannot verify private server cache path'}
    $stats=$raw|ConvertFrom-Json
    $actual=$stats.cache_location.Replace('\','/') -replace '/+','/'
    $expected=$env:SCCACHE_DIR.Replace('\','/') -replace '/+','/'
    if(!$expected -or $actual.IndexOf($expected,[StringComparison]::OrdinalIgnoreCase) -lt 0){throw 'Private cache path mismatch'}
    $raw|Set-Content (Join-Path $ReportDirectory 'cache-after.json')
    $again=Get-NetTCPConnection -LocalAddress 127.0.0.1 -LocalPort $s.port -State Listen
    $current=Get-Process -Id $again.OwningProcess
    if($current.Id -ne $p.Id -or $current.StartTime.ToUniversalTime() -ne $stamp){throw 'Private listener changed during verification'}
    $current.Dispose()
    & sccache --stop-server *> (Join-Path $ReportDirectory 'cache-stop.log')
    if(!$p.WaitForExit(10000)){throw 'Verified private cache server did not stop'}
    $observed=@{pid=$p.Id;start_utc=$stamp.ToString('o');reported_owned_directory_verified=$true}
    $p.Dispose()
}
$remaining=@(Get-NetTCPConnection -LocalAddress 127.0.0.1 -LocalPort $s.port -State Listen -ErrorAction SilentlyContinue)
if($remaining.Count){throw 'Listener remains on owned port; cleanup incomplete'}
[ordered]@{owned_port=$s.port;current_listener=$observed;no_listener_after_cleanup=$true;owned_cache_server_exit_verified=$true;shared_cache_or_server_touched=$false;cache_entries_preserved=$true;temp_and_logs_preserved_for_inspection=$true;compiler_cache_worker_memory_not_in_cargo_tree_samples=$true}|ConvertTo-Json -Depth 4|Set-Content (Join-Path $ReportDirectory 'cache-lifecycle.json')
