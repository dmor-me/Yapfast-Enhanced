param(
    [Parameter(Mandatory=$true)][string]$Loader,
    [Parameter(Mandatory=$true)][string]$RuntimeHelper,
    [Parameter(Mandatory=$true)][string]$PluginDirectory,
    [Parameter(Mandatory=$true)][string[]]$OfficialExecutables,
    [Parameter(Mandatory=$true)][string]$OutputDirectory
)
$ErrorActionPreference='Stop'
$taskLoader=(Resolve-Path -LiteralPath $Loader).Path
$taskDll=Join-Path (Split-Path -Parent $taskLoader) 'yapfast_enhanced_module.dll'
$taskDllHash=(Get-FileHash -LiteralPath $taskDll -Algorithm SHA256).Hash
$taskReport=@()
foreach ($taskApp in $OfficialExecutables) {
    $taskHash=(Get-FileHash -LiteralPath $taskApp -Algorithm SHA256).Hash
    $taskSession=Join-Path $OutputDirectory ($taskHash.Substring(0,12)+'-'+[guid]::NewGuid().ToString('N'))
    & $taskLoader --app $taskApp --runtime-helper $RuntimeHelper --plugin-dir $PluginDirectory --state-dir $taskSession --self-test --close-after-test
    if ($LASTEXITCODE -ne 0) { throw "Loader compatibility test failed. See $taskSession" }
    $taskResult=Get-Content -LiteralPath (Join-Path $taskSession 'loader-report.json') -Raw | ConvertFrom-Json
    if (-not $taskResult.passed -or -not $taskResult.file_unchanged) { throw 'Compatibility report failed its integrity checks' }
    if ((Get-FileHash -LiteralPath $taskDll -Algorithm SHA256).Hash -ne $taskDllHash) { throw 'The DLL changed between app versions' }
    $taskReport+=$taskResult
}
$taskSummary=Join-Path $OutputDirectory 'compatibility-summary.json'
$taskReport | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath $taskSummary -Encoding utf8
Write-Output "Same module passed on $($taskReport.Count) explicitly tested official executables. This covers the overlay and Ping only."
