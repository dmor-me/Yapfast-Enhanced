$ErrorActionPreference='Stop'
try {
    $taskRepo=(Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '../..')).Path
    $taskLoader=Join-Path $taskRepo '.local/loader-prototype-target/release/yapfast-enhanced-loader.exe'
    $taskModule=Join-Path (Split-Path -Parent $taskLoader) 'yapfast_enhanced_module.dll'
    $taskApp=Join-Path $taskRepo '.local/installer-flow-qa/app/yapfast.exe'
    $taskHelper=Join-Path $taskRepo '.release/enhanced-v0.4.1.1/bundle/payload/yapfast-plugins.exe'
    foreach ($taskFile in @($taskLoader,$taskModule,$taskApp,$taskHelper)) {
        if (-not (Test-Path -LiteralPath $taskFile -PathType Leaf)) {
            throw "This local demo needs its prepared test files. Missing: $taskFile"
        }
    }
    if ((Get-FileHash -LiteralPath $taskApp -Algorithm SHA256).Hash.ToLowerInvariant() -ne '2d8363fad4dbd20632248107da67f74c45bb698e4fdcc416023e001fee3e7419') {
        throw 'The isolated official test application changed. The demo will not use your normal app instead.'
    }
    $taskSession=Join-Path $taskRepo ('.local/loader-feasibility/manual-demo-'+[guid]::NewGuid().ToString('N'))
    Write-Host 'Opening an isolated sample Yapfast window. Your normal account is not used.'
    Write-Host 'Look near the lower-right corner for Enhanced loader prototype.'
    Write-Host 'Ping runs once automatically. Click its button again to repeat it.'
    Write-Host 'A successful run displays Ping: pong. This demo does not support FGF yet.'
    Write-Host 'To finish, use Quit on the test app tray icon if closing its window leaves it in the tray.'
    Write-Host ''
    & $taskLoader --app $taskApp --runtime-helper $taskHelper --plugin-dir (Join-Path $taskRepo 'plugins/ping') --state-dir $taskSession --self-test
    if ($LASTEXITCODE -ne 0) { throw "Demo failed. Reports are in $taskSession" }
    Write-Host ''
    Write-Host "Demo finished. Reports: $taskSession"
} catch {
    Write-Host $_.Exception.Message -ForegroundColor Red
    exit 1
}
