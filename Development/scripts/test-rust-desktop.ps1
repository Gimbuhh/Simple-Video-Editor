param([string]$AppPath,[switch]$Extended,[string[]]$Fixtures,[switch]$ScaleLayouts,[switch]$NativeFallback)
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'rust-environment.ps1')
if (!$AppPath) {
    & $editorCargo build --manifest-path (Join-Path $editorRustWorkspace 'Cargo.toml') -p simple-video-editor --locked
    if ($LASTEXITCODE -ne 0) { throw 'Desktop build failed.' }
    $AppPath=Join-Path $env:CARGO_TARGET_DIR 'debug/simple-video-editor.exe'
}
$AppPath=[IO.Path]::GetFullPath($AppPath)
$editorDesktopExeHash=(Get-FileHash -LiteralPath $AppPath -Algorithm SHA256).Hash
$editorDesktopRoot=Join-Path $editorRustDevelopment ('artifacts/rust-desktop/'+[guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $editorDesktopRoot -Force | Out-Null
$editorDesktopReports=@()
foreach ($editorDesktopMode in @('native','software')) {
    $editorDesktopFixtures=if($Fixtures){$Fixtures}else{@('sdr.mkv','pq.mkv','hlg.mkv','audio.mkv')}
    if($Extended){$editorDesktopFixtures+=@('geometry/rotated.mp4','geometry/anamorphic.mp4','geometry/rgb.mkv','geometry/multiple.mkv','stepping/variable.mkv')}
    foreach ($editorDesktopFixture in $editorDesktopFixtures) {
        $editorDesktopSource=Join-Path $editorRustDevelopment "artifacts/rust-playback/$editorDesktopFixture"
        if (!(Test-Path -LiteralPath $editorDesktopSource)) { throw 'Generate the fixtures with test-rust-playback.ps1 first.' }
        $editorDesktopCase=[IO.Path]::GetFileNameWithoutExtension($editorDesktopFixture)
        $editorDesktopOutput=Join-Path $editorDesktopRoot "$editorDesktopMode-$editorDesktopCase"
        $editorDesktopArguments=@('--verify-desktop', ('"'+$editorDesktopSource+'"'), ('"'+$editorDesktopOutput+'"'))
        if ($editorDesktopMode -eq 'software') {$editorDesktopArguments+='--software-preview'}
        if ($ScaleLayouts) {$editorDesktopArguments+='--scale-layouts'}
        if ($NativeFallback) {$editorDesktopArguments+='--fail-native-preview'}
        $editorDesktopSecondary=Join-Path $editorRustDevelopment ('artifacts/rust-playback/'+$(if($editorDesktopCase -eq 'sdr'){'pq.mkv'}else{'sdr.mkv'}))
        $editorDesktopArguments+=@('--second-recording',('"'+$editorDesktopSecondary+'"'))
        $editorDesktopProcess=Start-Process -FilePath $AppPath -ArgumentList $editorDesktopArguments -PassThru -WindowStyle Hidden
        $editorDesktopProcess.WaitForExit()
        if ($editorDesktopProcess.ExitCode -ne 0) { throw "Desktop crashed: $editorDesktopMode $editorDesktopFixture ($($editorDesktopProcess.ExitCode))" }
        $editorDesktopReport=Get-Content -LiteralPath (Join-Path $editorDesktopOutput 'desktop.json') -Raw | ConvertFrom-Json
        if (!$editorDesktopReport.passed) { throw "Desktop check failed: $editorDesktopMode $editorDesktopFixture · $($editorDesktopReport.failure)" }
        $editorDesktopReports+=@{mode=$editorDesktopMode;fixture=$editorDesktopFixture;report=$editorDesktopReport;executableSha256=$editorDesktopExeHash;scaleLayouts=[bool]$ScaleLayouts}
        Write-Output "Desktop passed: $editorDesktopMode $editorDesktopFixture · $($editorDesktopReport.checks.Count) checks"
    }
}
$editorDesktopReports | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $editorDesktopRoot 'results.json') -Encoding utf8
Write-Output "Desktop reports and screenshots: $editorDesktopRoot"
