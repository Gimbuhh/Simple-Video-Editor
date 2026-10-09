# Third-party components

## mpv / libmpv

The portable folder includes `libmpv-2.dll` from the official mpv project's Windows LGPL build, version `v0.41.0-dev-g06ee185d3`, workflow run `37938244617`.

- Release: https://github.com/mpv-player/mpv/releases/tag/git-release
- Source: https://github.com/mpv-player/mpv/tree/06ee185d3
- Build workflow: https://github.com/mpv-player/mpv/actions/runs/37938244617
- License: bundled as `licenses/mpv-LGPL.txt` (LGPL 2.1).
- The library includes its own third-party dependencies; consult the upstream source/build workflow for their notices and corresponding sources.

The DLL is dynamically loaded and can be replaced by a compatible x64 libmpv build.

## FFmpeg and FFprobe

The bundle uses BtbN's `n8.1.3-14-g330caae0c1-win64-gpl-shared-8.1` build from `autobuild-2026-10-08-13-05`. FFmpeg and FFprobe run as separate processes and share the bundled FFmpeg DLLs. The GPL license is bundled as `licenses/ffmpeg-GPL.txt`.

- Pinned binary distribution: https://github.com/BtbN/FFmpeg-Builds/releases/tag/autobuild-2026-10-08-13-05
- Build scripts, configuration, and dependency sources: https://github.com/BtbN/FFmpeg-Builds
- FFmpeg source revision: https://github.com/FFmpeg/FFmpeg/tree/330caae0c1
- License information: https://ffmpeg.org/legal.html

The archive hash, installed executable/DLL hashes, and origins are recorded in `licenses/dependencies.json` beside this document. Keep the complete `bin` folder together; the shared executables require its DLLs.

## .NET / WPF

The portable app includes Microsoft's .NET 10 Windows Desktop runtime through self-contained publishing. Its license and third-party notices are copied into the `licenses` folder from the exact runtime packages and Windows Desktop SDK used by the build.

- Source and licenses: https://github.com/dotnet/runtime and https://github.com/dotnet/wpf

Application source licensing does not replace third-party license terms.
