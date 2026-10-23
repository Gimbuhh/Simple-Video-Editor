param(
    [string]$LegacyApp='App-Rewrite-Release',
    [string]$RustApp='App-Rust-11-Candidate',
    [Parameter(Mandatory)][string]$Source,
    [ValidateRange(1,5)][int]$Samples=3,
    [ValidateRange(5,30)][int]$SettleSeconds=10,
    [ValidateRange(3,20)][int]$MeasureSeconds=6,
    [switch]$Visible
)
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'project-paths.ps1')
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class EditorComparisonWindow {
    [StructLayout(LayoutKind.Sequential)] public struct Bounds { public int Left, Top, Right, Bottom; }
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr window);
    [DllImport("user32.dll")] public static extern bool IsIconic(IntPtr window);
    [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr window, out Bounds bounds);
    [DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr window);
}
'@
$editorComparisonRoot=Join-Path $editorDevelopment ('artifacts/version-comparison/'+[guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $editorComparisonRoot -Force | Out-Null
$Source=[IO.Path]::GetFullPath($Source)
$editorComparisonSourceHash=(Get-FileHash -LiteralPath $Source -Algorithm SHA256).Hash
$editorComparisonApps=@{}
foreach($editorComparisonName in @('csharp','rust')){
    $editorComparisonDirectory=[IO.Path]::GetFullPath($(if($editorComparisonName -eq 'csharp'){$LegacyApp}else{$RustApp}))
    $editorComparisonExe=Join-Path $editorComparisonDirectory 'Simple Video Editor.exe'
    $editorComparisonApps[$editorComparisonName]=@{
        directory=$editorComparisonDirectory;exe=$editorComparisonExe
        version=[Diagnostics.FileVersionInfo]::GetVersionInfo($editorComparisonExe).FileVersion
        executableSha256=(Get-FileHash -LiteralPath $editorComparisonExe -Algorithm SHA256).Hash
        executableBytes=(Get-Item -LiteralPath $editorComparisonExe).Length
        bundleBytes=(Get-ChildItem -LiteralPath $editorComparisonDirectory -File -Recurse | Measure-Object Length -Sum).Sum
    }
}
if($editorComparisonApps.csharp.version -ne '1.0.4.0' -or $editorComparisonApps.rust.version -ne '1.1.0.0'){throw 'Expected the preserved C# 1.0.4 and Rust 1.1.0 artifacts.'}
$editorComparisonNative=@()
foreach($editorComparisonFile in Get-ChildItem -LiteralPath (Join-Path $editorComparisonApps.rust.directory 'bin') -File){
    $editorComparisonOld=Join-Path $editorComparisonApps.csharp.directory ('bin/'+$editorComparisonFile.Name)
    if(!(Test-Path -LiteralPath $editorComparisonOld)){throw 'Native dependency sets differ.'}
    $editorComparisonHash=(Get-FileHash -LiteralPath $editorComparisonFile.FullName -Algorithm SHA256).Hash
    if((Get-FileHash -LiteralPath $editorComparisonOld -Algorithm SHA256).Hash -ne $editorComparisonHash){throw 'Native dependency versions differ; do not compare as the same media engine.'}
    $editorComparisonNative+=@{name=$editorComparisonFile.Name;sha256=$editorComparisonHash}
}
@{apps=$editorComparisonApps;native=$editorComparisonNative;source=$Source;sourceSha256=$editorComparisonSourceHash;visibleRequested=[bool]$Visible;cpu=@(Get-CimInstance Win32_Processor | Select-Object Name,NumberOfLogicalProcessors);gpu=@(Get-CimInstance Win32_VideoController | Select-Object Name,DriverVersion);scope='Process-only CPU/private bytes/working set after settling; excludes child-process and GPU memory. App default window/preview sizes, uncontrolled OS cache/system load, not a cold-start or export-speed benchmark. Hidden runs can produce unmatched minimized states and are diagnostic only.'} | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $editorComparisonRoot 'environment.json') -Encoding utf8
$editorComparisonRows=@()
$editorComparisonCases=@(
    @{name='csharp-empty';app='csharp';preview=$false},
    @{name='rust-empty';app='rust';preview=$false},
    @{name='csharp-preview';app='csharp';preview=$true},
    @{name='rust-software-preview';app='rust';preview=$true;software=$true},
    @{name='rust-native-preview';app='rust';preview=$true;software=$false}
)
foreach($editorComparisonSample in 1..$Samples){
    # Alternate order to reduce systematic first/last bias without claiming a
    # controlled cold cache or equivalent decoder/compositor internals.
    $editorComparisonOrder=if($editorComparisonSample % 2){$editorComparisonCases}else{@($editorComparisonCases[4..0])}
    foreach($editorComparisonCase in $editorComparisonOrder){
        $editorComparisonRun=Join-Path $editorComparisonRoot "$($editorComparisonCase.name)-$editorComparisonSample"
        New-Item -ItemType Directory -Path $editorComparisonRun -Force | Out-Null
        $editorComparisonProject=Join-Path $editorComparisonRun 'comparison-project.sveproject'
        $editorComparisonDocument=@{Version=1;Sources=@();Clips=@()}
        if($editorComparisonCase.preview){
            $editorComparisonDocument.Sources=@($Source)
            $editorComparisonDocument.Clips=@(@{Path=$Source;Start=0.5;End=2.5;TimelineStart=0.0})
        }
        $editorComparisonDocument | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath $editorComparisonProject -Encoding utf8
        $editorComparisonArguments=@(('"'+$editorComparisonProject+'"'))
        $editorComparisonHold=$editorComparisonCase.app -eq 'rust' -and $editorComparisonCase.preview
        if($editorComparisonHold){
            $editorComparisonArguments=@('--verify-desktop',('"'+$Source+'"'),('"'+$editorComparisonRun+'"'),'--capture-hold')
            if($editorComparisonCase.software){$editorComparisonArguments+='--software-preview'}
        }
        $editorComparisonApp=$editorComparisonApps[$editorComparisonCase.app]
        $editorComparisonProcess=$null
        $editorComparisonClosed=$false
        try{
            $editorComparisonWindowStyle=if($Visible){'Normal'}else{'Hidden'}
            $editorComparisonProcess=Start-Process -FilePath $editorComparisonApp.exe -ArgumentList $editorComparisonArguments -WorkingDirectory $editorComparisonApp.directory -WindowStyle $editorComparisonWindowStyle -Environment @{LOCALAPPDATA=(Join-Path $editorComparisonRun 'local-data')} -PassThru
            Start-Sleep -Seconds $SettleSeconds
            $editorComparisonProcess.Refresh()
            if($editorComparisonProcess.HasExited){throw 'Comparison process exited before sampling.'}
            if($editorComparisonHold -and !(Test-Path -LiteralPath (Join-Path $editorComparisonRun 'capture-ready.json'))){throw 'Rust preview did not become ready within the settling interval.'}
            if($Visible -and (![EditorComparisonWindow]::IsWindowVisible($editorComparisonProcess.MainWindowHandle) -or [EditorComparisonWindow]::IsIconic($editorComparisonProcess.MainWindowHandle))){throw 'Visible comparison helper was hidden or minimized; sample excluded.'}
            $editorComparisonCpuStart=$editorComparisonProcess.TotalProcessorTime.TotalMilliseconds
            $editorComparisonClock=[Diagnostics.Stopwatch]::StartNew()
            $editorComparisonMemory=@()
            foreach($editorComparisonTick in 1..$MeasureSeconds){
                Start-Sleep -Seconds 1
                $editorComparisonProcess.Refresh()
                if($editorComparisonProcess.HasExited){throw 'Comparison process exited during sampling.'}
                $editorComparisonMemory+=@{privateBytes=$editorComparisonProcess.PrivateMemorySize64;workingSet=$editorComparisonProcess.WorkingSet64}
            }
            $editorComparisonClock.Stop()
            $editorComparisonCpu=$editorComparisonProcess.TotalProcessorTime.TotalMilliseconds-$editorComparisonCpuStart
            $editorComparisonWindow=$editorComparisonProcess.MainWindowHandle
            $editorComparisonBounds=[EditorComparisonWindow+Bounds]::new()
            $editorComparisonHasBounds=[EditorComparisonWindow]::GetClientRect($editorComparisonWindow,[ref]$editorComparisonBounds)
            if($Visible -and (![EditorComparisonWindow]::IsWindowVisible($editorComparisonWindow) -or [EditorComparisonWindow]::IsIconic($editorComparisonWindow) -or !$editorComparisonHasBounds -or $editorComparisonBounds.Right -le 0 -or $editorComparisonBounds.Bottom -le 0)){throw 'Visible comparison helper changed window state; sample excluded.'}
            $editorComparisonRow=@{
                case=$editorComparisonCase.name;sample=$editorComparisonSample
                elapsedMs=$editorComparisonClock.Elapsed.TotalMilliseconds;cpuMs=$editorComparisonCpu
                cpuPercentOfOneCore=100*$editorComparisonCpu/$editorComparisonClock.Elapsed.TotalMilliseconds
                privateBytes=($editorComparisonMemory.privateBytes | Measure-Object -Average).Average
                workingSet=($editorComparisonMemory.workingSet | Measure-Object -Average).Average
                windowTitle=$editorComparisonProcess.MainWindowTitle
                windowVisible=[EditorComparisonWindow]::IsWindowVisible($editorComparisonWindow)
                windowMinimized=[EditorComparisonWindow]::IsIconic($editorComparisonWindow)
                windowDpi=[EditorComparisonWindow]::GetDpiForWindow($editorComparisonWindow)
                clientSize=$(if($editorComparisonHasBounds){@(($editorComparisonBounds.Right-$editorComparisonBounds.Left),($editorComparisonBounds.Bottom-$editorComparisonBounds.Top))}else{@()})
                rustPreviewReady=$editorComparisonHold
                desktopControlUsed=$false
            }
            if($editorComparisonHold){
                [IO.File]::WriteAllText((Join-Path $editorComparisonRun 'capture-stop'),'stop')
            }else{
                if(!$editorComparisonProcess.CloseMainWindow()){throw 'Owned comparison window could not be closed normally.'}
            }
            if(!$editorComparisonProcess.WaitForExit(15000)){throw 'Owned comparison process did not close.'}
            if($editorComparisonProcess.ExitCode -ne 0){throw 'Owned comparison process reported a failure.'}
            $editorComparisonClosed=$true
            $editorComparisonRow.normalClose=$true
            $editorComparisonRows+=$editorComparisonRow
            $editorComparisonRows | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $editorComparisonRoot 'samples.json') -Encoding utf8
            Write-Output "Compared $($editorComparisonCase.name) $editorComparisonSample/$Samples"
        }finally{
            # Only the exact child created above may be stopped on failure.
            if(!$editorComparisonClosed -and $editorComparisonProcess -and !$editorComparisonProcess.HasExited){$editorComparisonProcess.Kill();$editorComparisonProcess.WaitForExit()}
        }
    }
}
foreach($editorComparisonApp in $editorComparisonApps.Values){if((Get-FileHash -LiteralPath $editorComparisonApp.exe -Algorithm SHA256).Hash -ne $editorComparisonApp.executableSha256){throw 'Comparison changed an executable.'}}
if((Get-FileHash -LiteralPath $Source -Algorithm SHA256).Hash -ne $editorComparisonSourceHash){throw 'Comparison changed its source.'}
Write-Output "Comparison evidence: $editorComparisonRoot"
