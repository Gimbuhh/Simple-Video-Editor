$ErrorActionPreference = 'Stop'
$editorRustDevelopment = Split-Path -Parent $PSScriptRoot
$editorRustWorkspace = Join-Path $editorRustDevelopment 'rust'
$editorRustTools = Get-Content -LiteralPath (Join-Path $PSScriptRoot 'rust-tools.json') -Raw | ConvertFrom-Json
$editorRustToolchain = Get-Content -LiteralPath (Join-Path $editorRustWorkspace 'rust-toolchain.toml') -Raw
if ($editorRustToolchain -notmatch '(?m)^channel = "(\d+\.\d+\.\d+)"$') { throw 'Rust must have a pinned three-part toolchain version.' }
$editorRustVersion = $Matches[1]
$env:CARGO_HOME = Join-Path $editorRustDevelopment '.tools/cargo'
$env:RUSTUP_HOME = Join-Path $editorRustDevelopment '.tools/rustup'
$env:RUSTUP_TOOLCHAIN = "$editorRustVersion-$($editorRustTools.Host)"
$env:CARGO_TARGET_DIR = Join-Path $editorRustWorkspace 'target'
$editorCargo = Join-Path $env:CARGO_HOME 'bin/cargo.exe'
$editorRustc = Join-Path $env:CARGO_HOME 'bin/rustc.exe'
$env:PATH = (Join-Path $env:CARGO_HOME 'bin') + [IO.Path]::PathSeparator + $env:PATH
$env:PATH = (Join-Path $editorRustDevelopment '.tools/llvm/bin') + [IO.Path]::PathSeparator + $env:PATH
# Keep GNU and raw-dylib imports under one linker. GNU ld can leave duplicate
# import groups unresolved when the GUI graph combines windows-sys generations.
$env:CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER = Join-Path $editorRustDevelopment '.tools/llvm/bin/ld.lld.exe'
$env:CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUSTFLAGS = '-C linker-flavor=ld.lld -C link-self-contained=yes'
