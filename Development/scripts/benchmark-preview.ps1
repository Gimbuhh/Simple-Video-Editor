param([Parameter(Mandatory)][string]$AppPath,[Parameter(Mandatory)][string]$Source,[switch]$Software)
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'project-paths.ps1')
$previewExe=[IO.Path]::GetFullPath($AppPath)
$previewSource=[IO.Path]::GetFullPath($Source)
$previewSourceHash=(Get-FileHash -LiteralPath $previewSource -Algorithm SHA256).Hash
$previewExeHash=(Get-FileHash -LiteralPath $previewExe -Algorithm SHA256).Hash
$previewRoot=Join-Path $editorDevelopment ('artifacts/preview-benchmark/'+[guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $previewRoot -Force | Out-Null
$previewArguments=@('--verify-desktop',('"'+$previewSource+'"'),('"'+$previewRoot+'"'),'--benchmark-preview')
if($Software){$previewArguments+='--software-preview'}
$previewProcess=Start-Process -FilePath $previewExe -ArgumentList $previewArguments -PassThru -WindowStyle Hidden
$previewProcess.WaitForExit()
if($previewProcess.ExitCode -ne 0){throw 'Preview benchmark process failed.'}
$previewReport=Get-Content -LiteralPath (Join-Path $previewRoot 'desktop.json') -Raw | ConvertFrom-Json
if(!$previewReport.passed){throw $previewReport.failure}
$previewLatencies=Get-Content -LiteralPath (Join-Path $previewRoot 'preview-latency.json') -Raw | ConvertFrom-Json
$previewSeeks=@($previewLatencies.samples | Where-Object kind -EQ 'seek' | ForEach-Object {[double]$_.ms} | Sort-Object)
if($previewSeeks.Count -ne 24){throw 'Expected 24 completed production preview seeks.'}
if((Get-FileHash -LiteralPath $previewSource -Algorithm SHA256).Hash -ne $previewSourceHash){throw 'Benchmark source changed.'}
if((Get-FileHash -LiteralPath $previewExe -Algorithm SHA256).Hash -ne $previewExeHash){throw 'Benchmark executable changed.'}
$previewSummary=@{
    executableSha256=$previewExeHash; sourceSha256=$previewSourceHash
    software=[bool]$Software; samples=$previewSeeks.Count
    loadMs=$previewLatencies.samples[0].ms
    medianMs=($previewSeeks[11]+$previewSeeks[12])/2
    p95Ms=$previewSeeks[22];maximumMs=$previewSeeks[23]
    decoder=@($previewLatencies.samples | ForEach-Object {$_.decoder} | Where-Object {$_} | Sort-Object -Unique)
    retainedFrameChecks=$previewLatencies.retainedFrameChecks
    continuousScrubFrames=$previewLatencies.continuousScrubFrames
    finalScrubReleaseMs=$previewLatencies.finalScrubReleaseMs
    scope=$previewLatencies.scope
}
$previewSummary | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $previewRoot 'summary.json') -Encoding utf8
Write-Output "Production preview: median=$($previewSummary.medianMs) ms; p95=$($previewSummary.p95Ms) ms; software=$Software"
Write-Output "Preview benchmark reports: $previewRoot"
