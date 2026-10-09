function Get-EditorPackageName([string]$Version) {
    if ($Version -notmatch '^\d+\.\d+\.\d+$') { throw 'Invalid portable package version.' }
    return "SimpleVideoEditor-$Version-win-x64.zip"
}

function Assert-EditorPackage([string]$Path, [string]$Version) {
    $packageName = Get-EditorPackageName $Version
    if ([IO.Path]::GetFileName($Path) -cne $packageName) { throw 'Portable package filename does not match the app version.' }
    $hash = (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
    if ((Get-Content -LiteralPath ($Path + '.sha256') -Raw).Trim() -cne "$hash  $packageName") { throw 'Portable checksum mismatch.' }
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $archive = [IO.Compression.ZipFile]::OpenRead([IO.Path]::GetFullPath($Path))
    $versionProbe = Join-Path ([IO.Path]::GetDirectoryName([IO.Path]::GetFullPath($Path))) ("package-version-$([Guid]::NewGuid().ToString('N')).exe")
    try {
        $entries = @($archive.Entries | ForEach-Object { $_.FullName.Replace('\','/') })
        $roots = @($entries | ForEach-Object { ($_ -split '/')[0] } | Sort-Object -Unique)
        if (@(Compare-Object @('Simple Video Editor.exe','README.txt','bin','docs') $roots).Count) { throw 'Portable package has unexpected top-level entries.' }
        $executables = @($entries | Where-Object { $_ -match '(?i)(\.exe$|\.zip$|\.lnk$)' })
        if (@(Compare-Object @('Simple Video Editor.exe','bin/ffmpeg.exe','bin/ffprobe.exe') $executables).Count) { throw 'Portable package contains an extra app, shortcut or archive.' }
        if ($entries | Where-Object { $_ -match '(^/|^[A-Za-z]:|(^|/)\.\.(/|$))' }) { throw 'Portable package contains an unsafe entry path.' }
        [IO.Compression.ZipFileExtensions]::ExtractToFile($archive.GetEntry('Simple Video Editor.exe'), $versionProbe)
        if ([Diagnostics.FileVersionInfo]::GetVersionInfo($versionProbe).FileVersion -ne "$Version.0") { throw 'Packaged executable version mismatch.' }
    } finally {
        $archive.Dispose()
        if (Test-Path -LiteralPath $versionProbe) { Remove-Item -LiteralPath $versionProbe -Force }
    }
}

function Publish-EditorLocalPackage([string]$Path, [string]$Directory, [string]$Version) {
    # Never clear the previous package until the replacement is fully written and checked.
    Assert-EditorPackage $Path $Version
    $directoryPath = [IO.Path]::GetFullPath($Directory)
    New-Item -ItemType Directory -Path $directoryPath -Force | Out-Null
    $destination = Join-Path $directoryPath (Get-EditorPackageName $Version)
    if ([IO.Path]::GetFullPath($Path) -ne $destination) {
        Copy-Item -LiteralPath $Path -Destination $destination -Force
        Copy-Item -LiteralPath ($Path + '.sha256') -Destination ($destination + '.sha256') -Force
    }
    Assert-EditorPackage $destination $Version
    foreach ($file in Get-ChildItem -LiteralPath $directoryPath -File) {
        if ($file.Name -match '^SimpleVideoEditor(?:-\d+\.\d+\.\d+)?-win-x64\.zip(?:\.sha256)?$' -and $file.FullName -notin @($destination, ($destination + '.sha256'))) {
            if ([IO.Path]::GetDirectoryName([IO.Path]::GetFullPath($file.FullName)) -ne $directoryPath) { throw 'Package cleanup escapes its output directory.' }
            Remove-Item -LiteralPath $file.FullName -Force
        }
    }
}

function Assert-EditorCurrentPackage([string]$Directory, [string]$Version) {
    $name = Get-EditorPackageName $Version
    $packages = @(Get-ChildItem -LiteralPath $Directory -File | Where-Object { $_.Name -match '^SimpleVideoEditor(?:-\d+\.\d+\.\d+)?-win-x64\.zip(?:\.sha256)?$' } | ForEach-Object Name)
    if (@(Compare-Object @($name, ($name + '.sha256')) $packages).Count) { throw 'The package directory must contain only the current version ZIP and checksum.' }
}
