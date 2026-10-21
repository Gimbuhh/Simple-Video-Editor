# Third-party components

## mpv / libmpv

The portable folder includes `libmpv-2.dll` from the official mpv project's Windows LGPL build, version `v0.41.0-dev-g06ee185d3`, workflow run `37938244617`.

- Release: https://github.com/mpv-player/mpv/releases/tag/git-release
- Source: https://github.com/mpv-player/mpv/tree/06ee185d3
- Build workflow: https://github.com/mpv-player/mpv/actions/runs/37938244617
- License: bundled as `licenses/mpv-LGPL.txt` (LGPL 2.1).
- The library includes its own third-party dependencies; consult the upstream source/build workflow for their notices and corresponding sources.

The DLL is dynamically loaded. Changing a native build requires updating its recorded checksum and retaining the corresponding license/source notices when rebuilding the app.

## FFmpeg and FFprobe

The bundle uses BtbN's `n8.1.3-14-g330caae0c1-win64-gpl-shared-8.1` build from `autobuild-2026-10-08-13-05`. FFmpeg and FFprobe run as separate processes and share the bundled FFmpeg DLLs. The GPL license is bundled as `licenses/ffmpeg-GPL.txt`.

- Pinned binary distribution: https://github.com/BtbN/FFmpeg-Builds/releases/tag/autobuild-2026-10-08-13-05
- Build scripts, configuration, and dependency sources: https://github.com/BtbN/FFmpeg-Builds
- FFmpeg source revision: https://github.com/FFmpeg/FFmpeg/tree/330caae0c1
- License information: https://ffmpeg.org/legal.html

The archive hash, installed executable/DLL hashes, and origins are recorded in `licenses/dependencies.json` beside this document. Keep the complete `bin` folder together; the shared executables require its DLLs.

## Rust desktop application

The Rust application uses egui/eframe 0.32.3 with Glow, AccessKit and
locked Cargo dependencies. Its package includes the resolved dependency versions
and SPDX expressions in `licenses/rust-dependencies.md`, the matching crate
license/notice files under `licenses/rust/`, and Rust toolchain library copyright
notices. Embedded fonts have their own licenses: Hack's font notice, Noto Emoji's
SIL Open Font License, Ubuntu's Font License, and the emoji icon font's MIT notice
are included under the `epaint_default_fonts` package's `fonts` folder.

The Rust application does not contain .NET/WPF. LLVM-MinGW is a checksum-pinned
build-time dependency and is not included in the portable app.

## Isolated legacy test oracle

Optional differential tests restore the exact 1.0.4 implementation from Git and compile it with the pinned .NET 10 SDK. This test-only implementation and its runtime are excluded from the Rust portable package. Previously published 1.0.x bundles retain their own .NET/WPF notices.

- Source and licenses: https://github.com/dotnet/runtime and https://github.com/dotnet/wpf

Application source licensing does not replace third-party license terms.
