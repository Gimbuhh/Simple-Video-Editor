param([Parameter(Mandatory)][string]$AppPath)
$ErrorActionPreference='Stop'
$editorLaunchRoot=Join-Path (Split-Path -Parent $PSScriptRoot) ('artifacts/rust-launch/'+[guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $editorLaunchRoot -Force | Out-Null
$AppPath=[IO.Path]::GetFullPath($AppPath)
$editorLaunchHash=(Get-FileHash -LiteralPath $AppPath -Algorithm SHA256).Hash
$editorLaunchFixtureRoot=Join-Path (Split-Path -Parent $PSScriptRoot) 'artifacts/rust-playback'
$editorLaunchSources=@('sdr.mkv','pq.mkv') | ForEach-Object { Join-Path $editorLaunchFixtureRoot $_ }
$editorLaunchSourceHashes=@($editorLaunchSources | ForEach-Object { (Get-FileHash -LiteralPath $_ -Algorithm SHA256).Hash })
$editorLaunchProject=Join-Path $editorLaunchRoot '編集 project with spaces.SVEPROJECT'
@{Version=1;Sources=$editorLaunchSources;Clips=@(@{Path=$editorLaunchSources[0];Start=0.123456;End=1.987654;TimelineStart=5.0})} | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath $editorLaunchProject -Encoding utf8
$editorLaunchProjectHash=(Get-FileHash -LiteralPath $editorLaunchProject -Algorithm SHA256).Hash
foreach($editorLaunchCase in @('project','recordings','relative-recordings')) {
    $editorLaunchReport=Join-Path $editorLaunchRoot $editorLaunchCase
    $editorLaunchArguments=if($editorLaunchCase -eq 'project'){@($editorLaunchProject)}elseif($editorLaunchCase -eq 'relative-recordings'){@('--software-preview')+@($editorLaunchSources | ForEach-Object { [IO.Path]::GetRelativePath($PWD, $_) })}else{@('--software-preview')+$editorLaunchSources}
    $editorLaunchProcess=$null
    try {
        $editorLaunchProcess=Start-Process -FilePath $AppPath -ArgumentList @($editorLaunchArguments | ForEach-Object { '"'+$_+'"' }) -WorkingDirectory $PWD -Environment @{SVE_VERIFY_LAUNCH_DIRECTORY=$editorLaunchReport} -WindowStyle Hidden -PassThru
        if(!$editorLaunchProcess.WaitForExit(45000)){throw "Filename startup timed out: $editorLaunchCase"}
        if($editorLaunchProcess.ExitCode -ne 0){throw "Filename startup crashed: $editorLaunchCase"}
        $editorLaunchResult=Get-Content -LiteralPath (Join-Path $editorLaunchReport 'desktop.json') -Raw | ConvertFrom-Json
        if(!$editorLaunchResult.passed){throw "Filename startup failed: $($editorLaunchResult.failure)"}
        Write-Output "Filename startup passed: $editorLaunchCase"
    } finally {
        if($editorLaunchProcess -and !$editorLaunchProcess.HasExited){$editorLaunchProcess.Kill();$editorLaunchProcess.WaitForExit()}
    }
}
if((Get-FileHash -LiteralPath $AppPath -Algorithm SHA256).Hash -ne $editorLaunchHash -or (Get-FileHash -LiteralPath $editorLaunchProject -Algorithm SHA256).Hash -ne $editorLaunchProjectHash){throw 'Filename startup changed the app or project.'}
for($editorLaunchIndex=0;$editorLaunchIndex -lt $editorLaunchSources.Count;$editorLaunchIndex++) {
    if((Get-FileHash -LiteralPath $editorLaunchSources[$editorLaunchIndex] -Algorithm SHA256).Hash -ne $editorLaunchSourceHashes[$editorLaunchIndex]){throw 'Filename startup changed a recording.'}
}
Write-Output "Filename startup reports: $editorLaunchRoot"
