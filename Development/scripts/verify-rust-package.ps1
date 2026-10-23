param([string]$PackagePath,[switch]$Desktop,[switch]$Extended)
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'project-paths.ps1')
. (Join-Path $PSScriptRoot 'rust-environment.ps1')
. (Join-Path $PSScriptRoot 'portable-package.ps1')
$editorRustVersionFile=Join-Path $editorRustWorkspace 'crates/desktop-ui/Cargo.toml'
if ((Get-Content $editorRustVersionFile -Raw) -notmatch '(?m)^version = "(\d+\.\d+\.\d+)"$') {throw 'Missing Rust candidate version.'}
$editorRustVersion=$Matches[1]
if (!$PackagePath) {$PackagePath=Join-Path $editorRustDevelopment ('artifacts/rust-packages/'+(Get-EditorPackageName $editorRustVersion))}
$PackagePath=[IO.Path]::GetFullPath($PackagePath)
Assert-EditorPackage $PackagePath $editorRustVersion
$editorPortableRoot=Join-Path $editorRustDevelopment ('artifacts/rust-package-verification/'+[guid]::NewGuid().ToString('N'))
$editorPortableApp=Join-Path $editorPortableRoot "relocated café's editor/app"
Expand-Archive -LiteralPath $PackagePath -DestinationPath $editorPortableApp
$editorPortableExe=Join-Path $editorPortableApp 'Simple Video Editor.exe'
$editorPortablePins=Get-Content (Join-Path $PSScriptRoot 'dependencies.json') -Raw | ConvertFrom-Json
foreach($editorPortableFile in $editorPortablePins.FfmpegFiles.psobject.Properties){if((Get-FileHash -LiteralPath (Join-Path $editorPortableApp "bin/$($editorPortableFile.Name)") -Algorithm SHA256).Hash -ine $editorPortableFile.Value){throw 'Relocated native checksum mismatch.'}}
if((Get-FileHash -LiteralPath (Join-Path $editorPortableApp 'bin/libmpv-2.dll') -Algorithm SHA256).Hash -ine $editorPortablePins.MpvLibrarySha256){throw 'Relocated playback checksum mismatch.'}
$editorPortableExpected=@('libmpv-2.dll')+@($editorPortablePins.FfmpegFiles.psobject.Properties.Name)
$editorPortableActual=@(Get-ChildItem -LiteralPath (Join-Path $editorPortableApp 'bin') | Select-Object -ExpandProperty Name)
if(@(Compare-Object $editorPortableExpected $editorPortableActual).Count){throw 'bin must contain only the pinned media binaries.'}
if(Get-ChildItem -LiteralPath $editorPortableApp -Recurse -File | Where-Object Name -Match '^(hostfxr|hostpolicy|coreclr|clrjit|PresentationFramework|SimpleVideoEditor\.dll)'){throw 'Rust portable package contains a .NET runtime or legacy app.'}
$editorPortableBytes=[IO.File]::ReadAllBytes($editorPortableExe);$editorPortablePe=[BitConverter]::ToInt32($editorPortableBytes,60)
if([BitConverter]::ToUInt16($editorPortableBytes,$editorPortablePe+24) -ne 0x20b){throw 'Expected a Windows x64 executable.'}
if([BitConverter]::ToUInt32($editorPortableBytes,$editorPortablePe+24+112+14*8) -ne 0){throw 'Rust executable contains a managed CLR header.'}
if(!(Test-Path -LiteralPath (Join-Path $editorPortableApp 'docs/licenses/rust-dependencies.md'))){throw 'Rust dependency notices are missing.'}
foreach($editorPortableDocument in Get-ChildItem -LiteralPath (Join-Path $editorPortableApp 'docs') -Filter '*.md' -File -Recurse){
    foreach($editorPortableLinkMatch in [regex]::Matches([IO.File]::ReadAllText($editorPortableDocument.FullName),'\]\(([^)]+)\)')){
        $editorPortableLink=$editorPortableLinkMatch.Groups[1].Value
        if($editorPortableLink -match '^[a-zA-Z][a-zA-Z0-9+.-]*:' -or $editorPortableLink.StartsWith('#')){continue}
        $editorPortableLink=($editorPortableLink -split '#',2)[0]
        if(!(Test-Path -LiteralPath (Join-Path $editorPortableDocument.DirectoryName $editorPortableLink))){throw "Broken packaged documentation link in $($editorPortableDocument.Name): $editorPortableLink"}
    }
}
$editorPortableFixtures=Join-Path $editorRustDevelopment 'artifacts/rust-playback'
$editorPortableReports=Join-Path $editorPortableRoot 'reports'
$editorPortableProcess=Start-Process -FilePath $editorPortableExe -ArgumentList @('--verify-media',('"'+$editorPortableFixtures+'"'),('"'+$editorPortableReports+'"')) -WorkingDirectory $editorPortableApp -WindowStyle Hidden -PassThru
$editorPortableProcess.WaitForExit()
if($editorPortableProcess.ExitCode -ne 0){throw "Rust portable media verification failed ($($editorPortableProcess.ExitCode))."}
$editorPortableReport=Get-Content -LiteralPath (Join-Path $editorPortableReports 'portable.json') -Raw | ConvertFrom-Json
if(!$editorPortableReport.passed){throw "Portable check failed: $($editorPortableReport.failure)"}
$editorPortableMissing=Join-Path $editorPortableApp 'bin/ffprobe.exe'
$editorPortableHeld=Join-Path $editorPortableApp 'bin/ffprobe.verification-held'
try{
    # Work only in this newly extracted disposable bundle. The checkout's vendor
    # tools must not mask a damaged release package, even from that working dir.
    Move-Item -LiteralPath $editorPortableMissing -Destination $editorPortableHeld
    $editorPortableNegative=Join-Path $editorPortableRoot 'missing-native-report'
    $editorPortableProcess=Start-Process -FilePath $editorPortableExe -ArgumentList @('--verify-media',('"'+$editorPortableFixtures+'"'),('"'+$editorPortableNegative+'"')) -WorkingDirectory $editorWorkspace -WindowStyle Hidden -PassThru
    if(!$editorPortableProcess.WaitForExit(30000)){Stop-Process -Id $editorPortableProcess.Id;throw 'Damaged package verification did not terminate.'}
    if($editorPortableProcess.ExitCode -eq 0){throw 'A missing packaged native tool was silently borrowed from outside the portable app.'}
}finally{if(Test-Path -LiteralPath $editorPortableHeld){Move-Item -LiteralPath $editorPortableHeld -Destination $editorPortableMissing}}
if($Desktop){
    & (Join-Path $PSScriptRoot 'test-rust-launch.ps1') -AppPath $editorPortableExe
    & (Join-Path $PSScriptRoot 'test-rust-desktop.ps1') -AppPath $editorPortableExe -Extended:$Extended
}
Write-Output "Rust ZIP layout, checksum, embedded version, native pins, no-.NET startup, relocation, persistence, export and playback passed: $editorPortableRoot"
