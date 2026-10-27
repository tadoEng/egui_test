param([Parameter(Mandatory)][string]$RunDirectory)
$ErrorActionPreference='Stop'
$nextest=Get-Content (Join-Path $RunDirectory 'nextest-list.stdout.log') -Raw | ConvertFrom-Json
$binaries=@($nextest.'rust-suites'.PSObject.Properties | ForEach-Object {
    $suite=$_.Value
    [ordered]@{binary_id=$suite.'binary-id'; count=@($suite.testcases.PSObject.Properties).Count; ignored=@($suite.testcases.PSObject.Properties | Where-Object {$_.Value.ignored}).Count; tests=@($suite.testcases.PSObject.Properties.Name | Sort-Object)}
})
$nextNames=@($binaries | ForEach-Object {$_.tests} | Sort-Object)
$nextIgnored=@($nextest.'rust-suites'.PSObject.Properties | ForEach-Object {$_.Value.testcases.PSObject.Properties | Where-Object {$_.Value.ignored} | ForEach-Object {$_.Name}} | Sort-Object)
function Get-CargoNames([string]$File) { @(Get-Content (Join-Path $RunDirectory $File) | ForEach-Object { if($_ -match '^(.+): test$') {$Matches[1]} } | Sort-Object) }
$cargoNames=Get-CargoNames 'cargo-list.stdout.log'
$cargoIgnored=Get-CargoNames 'cargo-ignored-list.stdout.log'
$namesEqual=($cargoNames.Count -eq $nextNames.Count) -and -not (Compare-Object $cargoNames $nextNames)
$ignoredEqual=($cargoIgnored.Count -eq $nextIgnored.Count) -and -not (Compare-Object $cargoIgnored $nextIgnored)
$summary=@(Get-Content (Join-Path $RunDirectory 'nextest-run.stderr.log') | Where-Object {$_ -match '\btests run:.*passed'} | Select-Object -Last 1)
if(-not $namesEqual -or -not $ignoredEqual -or -not $summary.Count){throw 'Cargo/nextest test parity or run summary missing'}
[ordered]@{source=(Get-Content (Join-Path $RunDirectory 'nextest-list.json') -Raw | ConvertFrom-Json).source_commit; test_name_multiset_equal=$namesEqual; ignored_name_multiset_equal=$ignoredEqual; listed=$nextNames.Count; ignored=$nextIgnored.Count; executed=$nextNames.Count-$nextIgnored.Count; nextest_summary=$summary[0].Trim(); test_threads=1; replacement_for_plain_cargo_or_docs=$false; speedup_claim=$false; binaries=$binaries} | ConvertTo-Json -Depth 10 | Set-Content (Join-Path $RunDirectory 'nextest-parity.json')
