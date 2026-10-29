# Installs migration tools inside the workspace; does not modify system PATH.
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'rust-environment.ps1')
if (!(Test-Path -LiteralPath $editorCargo)) {
    $editorRustInstaller = Join-Path $editorRustDevelopment '.tools/rustup-init.exe'
    New-Item -ItemType Directory -Path (Split-Path -Parent $editorRustInstaller) -Force | Out-Null
    if (!(Test-Path -LiteralPath $editorRustInstaller) -or (Get-FileHash -LiteralPath $editorRustInstaller -Algorithm SHA256).Hash -ine $editorRustTools.RustupSha256) {
        Invoke-WebRequest -Uri $editorRustTools.RustupUrl -OutFile $editorRustInstaller -UseBasicParsing
    }
    if ((Get-FileHash -LiteralPath $editorRustInstaller -Algorithm SHA256).Hash -ine $editorRustTools.RustupSha256) { throw 'Rust bootstrap checksum mismatch.' }
    & $editorRustInstaller -y --no-modify-path --profile minimal --default-host $editorRustTools.Host --default-toolchain $editorRustVersion -c rustfmt -c clippy
    if ($LASTEXITCODE -ne 0) { throw 'Workspace Rust installation failed.' }
} else {
    & (Join-Path $env:CARGO_HOME 'bin/rustup.exe') toolchain install $env:RUSTUP_TOOLCHAIN --profile minimal --no-self-update --no-update -c rustfmt -c clippy
    if ($LASTEXITCODE -ne 0) { throw 'Pinned Rust toolchain restoration failed.' }
}
& (Join-Path $env:CARGO_HOME 'bin/rustup.exe') set auto-self-update disable
if ($LASTEXITCODE -ne 0) { throw 'Could not disable unpinned Rust bootstrap self-updates.' }
$editorRustActual = & $editorRustc --version
if ($LASTEXITCODE -ne 0 -or !$editorRustActual.StartsWith("rustc $editorRustVersion ")) { throw 'Unexpected Rust compiler version.' }
Write-Output "Workspace Rust ready: $editorRustActual"
$editorLlvmBin = Join-Path $editorRustDevelopment '.tools/llvm/bin'
if (!(Test-Path -LiteralPath (Join-Path $editorLlvmBin 'dlltool.exe')) -or !(Test-Path -LiteralPath (Join-Path $editorLlvmBin 'ld.lld.exe')) -or !(Test-Path -LiteralPath (Join-Path $editorLlvmBin 'llvm-rc.exe')) -or !(Test-Path -LiteralPath (Join-Path $editorLlvmBin 'llvm-cvtres.exe')) -or !(Test-Path -LiteralPath (Join-Path $editorRustDevelopment '.tools/llvm/lib/libshlwapi.a'))) {
    $editorLlvmZip = Join-Path $editorRustDevelopment '.tools/llvm-mingw.zip'
    if (!(Test-Path -LiteralPath $editorLlvmZip) -or (Get-FileHash -LiteralPath $editorLlvmZip -Algorithm SHA256).Hash -ine $editorRustTools.LlvmSha256) {
        Invoke-WebRequest -Uri $editorRustTools.LlvmUrl -OutFile $editorLlvmZip
    }
    if ((Get-FileHash -LiteralPath $editorLlvmZip -Algorithm SHA256).Hash -ine $editorRustTools.LlvmSha256) { throw 'LLVM archive checksum mismatch.' }
    New-Item -ItemType Directory -Path $editorLlvmBin -Force | Out-Null
    $editorLlvmArchive = [IO.Compression.ZipFile]::OpenRead($editorLlvmZip)
    try {
        foreach ($editorLlvmEntry in $editorLlvmArchive.Entries) {
            if ($editorLlvmEntry.FullName -match ('^' + [regex]::Escape($editorRustTools.LlvmRoot) + '/bin/(llvm-dlltool\.exe|ld\.lld\.exe|lld\.exe|llvm-rc\.exe|llvm-cvtres\.exe|[^/]+\.dll)$')) {
                [IO.Compression.ZipFileExtensions]::ExtractToFile($editorLlvmEntry, (Join-Path $editorLlvmBin $editorLlvmEntry.Name), $true)
            }
            if ($editorLlvmEntry.FullName -eq "$($editorRustTools.LlvmRoot)/x86_64-w64-mingw32/lib/libshlwapi.a") {
                $editorLlvmLib = Join-Path $editorRustDevelopment '.tools/llvm/lib'
                New-Item -ItemType Directory -Path $editorLlvmLib -Force | Out-Null
                [IO.Compression.ZipFileExtensions]::ExtractToFile($editorLlvmEntry, (Join-Path $editorLlvmLib 'libshlwapi.a'), $true)
            }
        }
    } finally { $editorLlvmArchive.Dispose() }
    Copy-Item -LiteralPath (Join-Path $editorLlvmBin 'llvm-dlltool.exe') -Destination (Join-Path $editorLlvmBin 'dlltool.exe') -Force
}
