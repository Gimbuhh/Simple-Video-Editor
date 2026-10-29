# Simple Video Editor

A portable Windows editor for trimming and combining local recordings. Edits leave the original files untouched.

![Rust editor with linked video and audio lanes](docs/images/editor.png)

Version 1.1.0 is the Rust release candidate on this branch. The latest published 1.0.x release remains available for rollback. Current-display HDR preview is user-confirmed and local game/Shadowplay controls pass. The user confirmed the separate HDR-to-SDR game-switch recording issue is external to this project. The PR stays open for remaining testing and approval.

Current startup checks pass. The current candidate includes automatic audio-output recovery, accessible playback/trim labels and preview-cache cleanup when changing projects. Connected HDR/SDR monitor round trips pass. The [release checklist](docs/release-readiness.md) separates automated checks from the remaining physical DPI/HDR-toggle and screen-reader acceptance.

The Rust candidate opens project/media filenames and places library/File Explorer drops at the timeline pointer. Idle previews sleep between updates and use display-sized processing while exports retain their chosen resolution. The [C#/Rust comparison](docs/rust-csharp-comparison.md) records the verified benefits, resource measurements and remaining tradeoffs. Keep the C# release and original projects for rollback while testing continues.

## Run

Extract the complete portable ZIP and open **Simple Video Editor.exe**. Keep **bin** and **docs** beside it. The Rust app needs no installer, account, .NET runtime, or separately installed FFmpeg.

[Published Windows releases](https://github.com/Gimbuhh/simple-video-editor/releases) require access to this private repository. Build the candidate from this branch using the commands below.

## Editing

1. Import recordings, select one for preview, and use **Add to timeline**, **Add all**, or drag a recording onto the timeline.
2. Drag linked sections to move them. Drag edges to trim, split at the playhead, or remove sections. Right-click a section and choose **Separate audio** to edit its audio independently. Gaps and repeated sections are preserved.
3. Choose output size, rational frame rate, SDR/HDR10 profile and codec. Per-section audio stream, gain, mute, fades, fit/fill/crop and rotation are saved with the project.
4. Export an MP4 using NVIDIA encoding when supported, with CPU fallback.

PQ and HLG inputs are supported. SDR preview deliberately tone-maps HDR. Native HDR presentation is requested only when Windows reports active HDR on the preview display; software preview uses SDR. Physical HDR presentation is not certified by metadata or screenshots.

| Shortcut | Action |
| --- | --- |
| Space / Left / Right | Play or pause / previous or next decoded frame |
| Ctrl+B or S | Split at the playhead |
| Q / W | Trim before / after, retaining the current frame |
| I / O | Set selected section in / out |
| Delete | Delete the selected video/audio item |
| Ctrl+Z / Ctrl+Y | Undo / redo |
| Alt+Left / Alt+Right | Swap neighboring sections |
| Ctrl+I / Ctrl+F | Import / search recordings |
| Ctrl+N / Ctrl+O | New / open project |
| Ctrl+S / Ctrl+Shift+S | Save / save as |
| Shift+F10 | Selected section context menu |

See the [user guide](docs/user-guide.md) for persistence, recovery, source interpretation and export details. Transitions are follow-up work after the rewrite; hard cuts remain the default.

## Development

Run from the repository root in PowerShell:

```powershell
./Development/scripts/setup.ps1
./Development/scripts/check-repository.ps1
./Development/scripts/test.ps1 -Ui -Gpu -Extended
./Development/scripts/build.ps1 -Zip -OutputName App-Updated
./Development/scripts/verify-package.ps1 -OutputName App-Updated -Desktop -Extended
./Development/scripts/test-packaging.ps1
```

Omit `-Gpu` without compatible NVIDIA hardware. Setup restores checksum-pinned workspace-local Rust/LLVM and native media tools. Generated media tests need no private recordings. [Development details](docs/development.md) cover the isolated 1.0.4 oracle and verification limits.

| Folder | Purpose |
| --- | --- |
| `Development/rust/crates/` | Rust application, editor core, media, persistence and Windows boundaries |
| `Development/assets/` | Authored icon and Windows manifest |
| `Development/tests/RustParityOracle/` | Test-only reader of the exact old implementation restored from Git |
| `Development/scripts/` | Setup, checks, build, test and cleanup |
| `App*/`, `Releases/`, `Development/artifacts/` | Ignored generated bundles, packages and verification evidence |

The application version in `Development/rust/crates/desktop-ui/Cargo.toml` owns executable metadata and package names. [Release policy](docs/releasing.md) requires user testing, passing checks, and approval before merge, tagging or publication.

[Changelog](CHANGELOG.md) · [Contributing](CONTRIBUTING.md) · [Security](SECURITY.md) · [License](LICENSE) · [Dependency notices](THIRD-PARTY-NOTICES.md) · [Rewrite evidence](docs/rust-rewrite-status.md)
