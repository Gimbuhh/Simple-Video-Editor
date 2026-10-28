param([string]$AppPath='App/Simple Video Editor.exe')
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'rust-environment.ps1')
$AppPath=[IO.Path]::GetFullPath($AppPath)
Add-Type -TypeDefinition @"
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
public static class EditorDisplayChecks {
 [StructLayout(LayoutKind.Sequential)] public struct Rect {public int L,T,R,B;}
 [StructLayout(LayoutKind.Sequential,CharSet=CharSet.Unicode)] public struct Info {public int Size;public Rect Bounds,Work;public uint Flags;[MarshalAs(UnmanagedType.ByValTStr,SizeConst=32)]public string Name;}
 delegate bool MonitorCallback(IntPtr h,IntPtr dc,IntPtr r,IntPtr data);
 delegate bool WindowCallback(IntPtr h,IntPtr data);
 [DllImport("user32.dll")] static extern bool EnumDisplayMonitors(IntPtr dc,IntPtr clip,MonitorCallback cb,IntPtr data);
 [DllImport("user32.dll",CharSet=CharSet.Unicode)] static extern bool GetMonitorInfo(IntPtr h,ref Info info);
 [DllImport("user32.dll")] static extern bool EnumWindows(WindowCallback cb,IntPtr data);
 [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr h,out uint pid);
 [DllImport("user32.dll")] static extern bool SetWindowPos(IntPtr h,IntPtr after,int x,int y,int w,int height,uint flags);
 public static Info[] Monitors(){var rows=new List<Info>();EnumDisplayMonitors(IntPtr.Zero,IntPtr.Zero,(h,dc,r,d)=>{var i=new Info();i.Size=Marshal.SizeOf<Info>();if(GetMonitorInfo(h,ref i))rows.Add(i);return true;},IntPtr.Zero);return rows.ToArray();}
 public static IntPtr Window(uint pid){IntPtr result=IntPtr.Zero;EnumWindows((h,d)=>{uint owner;GetWindowThreadProcessId(h,out owner);if(owner==pid){result=h;return false;}return true;},IntPtr.Zero);return result;}
 public static bool Move(IntPtr h,int x,int y){return SetWindowPos(h,IntPtr.Zero,x,y,0,0,0x15);}
}
"@
$editorDisplayMonitors=@([EditorDisplayChecks]::Monitors())
if($editorDisplayMonitors.Count -lt 2){throw 'Cross-monitor verification requires at least two connected monitors.'}
$editorDisplaySource=Join-Path $editorRustDevelopment 'artifacts/rust-playback/pq.mkv'
$editorDisplaySourceHash=(Get-FileHash -LiteralPath $editorDisplaySource -Algorithm SHA256).Hash
$editorDisplayExeHash=(Get-FileHash -LiteralPath $AppPath -Algorithm SHA256).Hash
$editorDisplayRoot=Join-Path $editorRustDevelopment ('artifacts/rust-displays/'+[guid]::NewGuid().ToString('N'))
$editorDisplayReports=@()
foreach($editorDisplayMode in @('native','software')){
    $editorDisplayOutput=Join-Path $editorDisplayRoot $editorDisplayMode
    New-Item -ItemType Directory -Path $editorDisplayOutput -Force | Out-Null
    $editorDisplayArguments=@('--verify-desktop',('"'+$editorDisplaySource+'"'),('"'+$editorDisplayOutput+'"'),'--capture-hold','--capture-hdr','--track-display')
    if($editorDisplayMode -eq 'software'){$editorDisplayArguments+='--software-preview'}
    $editorDisplayProcess=Start-Process -FilePath $AppPath -ArgumentList $editorDisplayArguments -WindowStyle Hidden -PassThru
    try{
        $editorDisplayDeadline=[DateTime]::UtcNow.AddSeconds(30)
        while(!(Test-Path -LiteralPath (Join-Path $editorDisplayOutput 'capture-ready.json'))){
            if($editorDisplayProcess.HasExited -or [DateTime]::UtcNow -gt $editorDisplayDeadline){throw 'Held display preview did not start.'}
            Start-Sleep -Milliseconds 250
        }
        $editorDisplayWindow=[EditorDisplayChecks]::Window([uint32]$editorDisplayProcess.Id)
        if($editorDisplayWindow -eq [IntPtr]::Zero){throw 'Missing isolated editor window.'}
        foreach($editorDisplayMonitor in @($editorDisplayMonitors[0],$editorDisplayMonitors[1],$editorDisplayMonitors[0])){
            $editorDisplayMoved=[DateTime]::UtcNow
            if(![EditorDisplayChecks]::Move($editorDisplayWindow,$editorDisplayMonitor.Work.L+40,$editorDisplayMonitor.Work.T+40)){throw 'Could not move the isolated editor.'}
            $editorDisplayDeadline=[DateTime]::UtcNow.AddSeconds(20)
            $editorDisplayStatePath=Join-Path $editorDisplayOutput 'display-state.json'
            $editorDisplayState=$null
            do{
                Start-Sleep -Milliseconds 250
                if($editorDisplayProcess.HasExited){throw 'Display helper exited during a monitor transition.'}
                # A state file can be observed during its write; retry only JSON parsing,
                # within this deadline, then require a fresh settled observation.
                $editorDisplayState=$null
                if(Test-Path -LiteralPath $editorDisplayStatePath){
                    try{$editorDisplayState=Get-Content -LiteralPath $editorDisplayStatePath -Raw | ConvertFrom-Json}catch{ $editorDisplayState=$null }
                }
                $editorDisplaySettled=$editorDisplayState -and $editorDisplayState.ready -and $editorDisplayState.stateKnown -and $editorDisplayState.display -eq $editorDisplayMonitor.Name -and (Get-Item -LiteralPath $editorDisplayStatePath).LastWriteTimeUtc -gt $editorDisplayMoved
                if($editorDisplaySettled){
                    $editorDisplaySettled=@(Compare-Object @($editorDisplayState.bounds) @($editorDisplayState.expectedBounds) -SyncWindow 0).Count -eq 0
                    $editorDisplayHdr=$editorDisplayMode -eq 'native' -and $editorDisplayState.hdrActive
                    $editorDisplaySettled=$editorDisplaySettled -and (($editorDisplayState.gamma -eq 'pq') -eq $editorDisplayHdr) -and ($editorDisplayState.software -eq ($editorDisplayMode -eq 'software'))
                }
                if([DateTime]::UtcNow -gt $editorDisplayDeadline){throw "Preview did not settle on $($editorDisplayMonitor.Name): $($editorDisplayState | ConvertTo-Json -Compress)"}
            }while(!$editorDisplaySettled)
            $editorDisplayReports+=@{mode=$editorDisplayMode;state=$editorDisplayState}
        }
    }finally{
        [IO.File]::WriteAllText((Join-Path $editorDisplayOutput 'capture-stop'),'stop')
        if(!$editorDisplayProcess.WaitForExit(10000)){Stop-Process -Id $editorDisplayProcess.Id -Force;throw 'Isolated display helper failed to close.'}
    }
    if($editorDisplayProcess.ExitCode -ne 0){throw 'Display helper crashed.'}
}
if((Get-FileHash -LiteralPath $editorDisplaySource -Algorithm SHA256).Hash -ne $editorDisplaySourceHash){throw 'Display test changed its source.'}
if((Get-FileHash -LiteralPath $AppPath -Algorithm SHA256).Hash -ne $editorDisplayExeHash){throw 'Display test executable changed.'}
@{passed=$true;executableSha256=$editorDisplayExeHash;observations=$editorDisplayReports;scope='Physical monitor round trips and actual native bounds/transfer selection. No Windows settings are changed. This does not measure HDR luminance, toggle HDR, or certify mixed-DPI hardware.'} | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $editorDisplayRoot 'results.json') -Encoding utf8
Write-Output "Native/software monitor round trips passed: $editorDisplayRoot"