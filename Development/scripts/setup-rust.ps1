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
