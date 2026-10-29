#requires -Version 7.4
param([Parameter(Mandatory)][string]$Source)
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'project-paths.ps1')
# The original playback classes run without WPF, with only DLL resolution and
# a playback-restart counter added. This does not time the full C# interface.
& (Join-Path $PSScriptRoot 'restore-legacy-oracle.ps1')
$legacyRoot=Join-Path $editorDevelopment ('artifacts/legacy-seek/'+[guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $legacyRoot -Force | Out-Null
$legacySource=[IO.Path]::GetFullPath($Source)
$legacySourceHash=(Get-FileHash -LiteralPath $legacySource -Algorithm SHA256).Hash
$legacyNativeRoot=Join-Path $editorWorkspace 'App/bin'
$legacyPins=Get-Content -LiteralPath (Join-Path $PSScriptRoot 'dependencies.json') -Raw | ConvertFrom-Json
if((Get-FileHash -LiteralPath (Join-Path $legacyNativeRoot 'libmpv-2.dll') -Algorithm SHA256).Hash -ine $legacyPins.MpvLibrarySha256){throw 'Legacy benchmark native pin mismatch.'}
$legacyClasses=Join-Path $editorDevelopment 'artifacts/rewrite-baseline/source/Development/src/SimpleVideoEditor/Services'
$legacyOriginal=Join-Path $legacyClasses 'NativePlayer.cs'
$legacyCode=[IO.File]::ReadAllText($legacyOriginal)
$legacyCode=$legacyCode.Replace('Path.Combine(AppContext.BaseDirectory, "bin", name)',('Path.Combine(@"'+$legacyNativeRoot.Replace('"','""')+'", name)'))
$legacyCode=$legacyCode.Replace('public event Action? PreviewFrameAvailable;', 'public event Action? PreviewFrameAvailable; public long Restarts;')
$legacyCode=$legacyCode.Replace('if (ev.Id == 8)', "if (ev.Id == 21) Interlocked.Increment(ref Restarts);`n            if (ev.Id == 8)")
[IO.File]::WriteAllText((Join-Path $legacyRoot 'NativePlayer.cs'),$legacyCode)
Copy-Item -LiteralPath (Join-Path $legacyClasses 'SoftwarePreviewRenderer.cs') -Destination $legacyRoot
[IO.File]::WriteAllText((Join-Path $legacyRoot 'Usings.cs'),'global using System; global using System.Threading; global using System.Linq;')
$legacyHarness=@'
using System.Diagnostics;
using System.Collections.Generic;
using SimpleVideoEditor.Services;
public static class SeekReference {
 public static double[] Run(string path) {
  using var p=new NativePlayer(); p.Initialize(); p.ResizePreview(960,540);
  long frames=0; p.PreviewFrameAvailable+=()=>Interlocked.Increment(ref frames);
  p.Load(path); Wait(()=>p.Get("video-params/w")!=null && p.Get("seeking")=="no" && Volatile.Read(ref frames)>0);
  var result=new List<double>();
  foreach(var t in new double[]{0.75,8.4,2.2,9.1,4.6,7.3,1.2,6.4,0.75,8.4,2.2,9.1,4.6,7.3,1.2,6.4,0.75,8.4,2.2,9.1,4.6,7.3,1.2,6.4}) {
   p.TakePreviewFrame()?.Dispose(); var before=p.Restarts; var f=Volatile.Read(ref frames); var clock=Stopwatch.StartNew();
   p.Seek(t); Wait(()=>Volatile.Read(ref p.Restarts)>before && p.Get("seeking")=="no" && Math.Abs(p.Position-t)<0.036 && Volatile.Read(ref frames)>f);
   p.TakePreviewFrame()?.Dispose(); result.Add(clock.Elapsed.TotalMilliseconds);
  } return result.ToArray();
 }
 static void Wait(Func<bool> ready) { var clock=Stopwatch.StartNew(); while(!ready()){ if(clock.Elapsed.TotalSeconds>12)throw new Exception("Reference timed out");Thread.Sleep(1);}}
}

'@
[IO.File]::WriteAllText((Join-Path $legacyRoot 'Reference.cs'),$legacyHarness)
Add-Type -Path @(Get-ChildItem -LiteralPath $legacyRoot -Filter '*.cs' | ForEach-Object FullName) -CompilerOptions '/nullable:enable'
$legacySamples=@([SeekReference]::Run($legacySource))
if($legacySamples.Count -ne 24){throw 'Expected 24 exact seeks.'}
if((Get-FileHash -LiteralPath $legacySource -Algorithm SHA256).Hash -ne $legacySourceHash){throw 'Legacy benchmark changed its source.'}
$legacySorted=@($legacySamples | Sort-Object)
$legacySummary=@{
    sourceCommit='dd65df13a41490e8168a624b65fcff6a27956940'
    nativePlayerSha256=(Get-FileHash -LiteralPath $legacyOriginal -Algorithm SHA256).Hash
    rendererSha256=(Get-FileHash -LiteralPath (Join-Path $legacyClasses 'SoftwarePreviewRenderer.cs') -Algorithm SHA256).Hash
    mpvSha256=$legacyPins.MpvLibrarySha256;sourceSha256=$legacySourceHash
    width=960;height=540;decoder='CPU';samples=$legacySamples
    medianMs=($legacySorted[11]+$legacySorted[12])/2;p95Ms=$legacySorted[22]
    scope='Pinned C# playback classes; exact seek restart plus new software frame; excludes WPF input/presentation and startup; requires SDR recording longer than 9.2 seconds; system load and filesystem caching uncontrolled'
}
$legacySummary | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $legacyRoot 'summary.json') -Encoding utf8
Write-Output "C# playback reference: median=$($legacySummary.medianMs) ms; p95=$($legacySummary.p95Ms) ms"
Write-Output "Legacy seek report: $legacyRoot"
