param([Parameter(Mandatory)][string]$AppPath,[ValidateRange(1,1000)][int]$Cycles=100,[switch]$GraphicsOnly)
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'project-paths.ps1')
$editorStressExe=[IO.Path]::GetFullPath($AppPath)
$editorStressRoot=Join-Path $editorDevelopment ('artifacts/startup-stress/'+[guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $editorStressRoot -Force | Out-Null
$editorStressSource=Join-Path $editorDevelopment 'artifacts/rust-playback/pq.mkv'
$editorStressSecondary=Join-Path $editorDevelopment 'artifacts/rust-playback/sdr.mkv'
if(!$GraphicsOnly -and (!(Test-Path -LiteralPath $editorStressSource) -or !(Test-Path -LiteralPath $editorStressSecondary))){throw 'Generate playback fixtures first.'}
foreach($editorStressIndex in 1..$Cycles){
    $editorStressOutput=Join-Path $editorStressRoot $editorStressIndex
    $editorStressArguments=@('--verify-desktop',('"'+$editorStressSource+'"'),('"'+$editorStressOutput+'"'),'--software-preview')
    if($GraphicsOnly){$editorStressArguments+='--graphics-startup-only'}else{$editorStressArguments+=@('--startup-only','--second-recording',('"'+$editorStressSecondary+'"'))}
    $editorStressProcess=Start-Process -FilePath $editorStressExe -ArgumentList $editorStressArguments -PassThru -WindowStyle Hidden
    if(!$editorStressProcess.WaitForExit(45000)){
        $editorStressProcess.Kill()
        throw "Owned verification process timed out: $editorStressOutput"
    }
    if($editorStressProcess.ExitCode -ne 0){
        foreach($editorStressDiagnostic in @('phase.txt','native-crash.txt')){
            $editorStressFile=Join-Path $editorStressOutput $editorStressDiagnostic
            if(Test-Path -LiteralPath $editorStressFile){Get-Content -LiteralPath $editorStressFile}
        }
        throw "Desktop startup crashed ($($editorStressProcess.ExitCode)): $editorStressOutput"
    }
    $editorStressReport=Get-Content -LiteralPath (Join-Path $editorStressOutput 'desktop.json') -Raw | ConvertFrom-Json
    if(!$editorStressReport.passed){throw "Desktop startup failed: $($editorStressReport.failure) · $editorStressOutput"}
    if($editorStressIndex % 10 -eq 0){Write-Output "Desktop startup passed $editorStressIndex/$Cycles · $editorStressRoot"}
}
@{passed=$true;cycles=$Cycles;graphicsOnly=[bool]$GraphicsOnly;executableSha256=(Get-FileHash -LiteralPath $editorStressExe -Algorithm SHA256).Hash} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $editorStressRoot 'results.json') -Encoding utf8
Write-Output "Startup stress reports: $editorStressRoot"
