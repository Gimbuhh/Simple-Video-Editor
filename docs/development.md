# Development

Run scripts from the repository root in PowerShell. Source is in `Development/rust`; the application package is `crates/desktop-ui`. Its Cargo version is authoritative. Tool crates use their separate private workspace version. Native pins and their exact recovery mirror remain in `Development/scripts/dependencies.json`.

## Setup and verification

```powershell
./Development/scripts/setup.ps1
./Development/scripts/check-repository.ps1
./Development/scripts/test.ps1
./Development/scripts/test.ps1 -Ui -Gpu -Extended
```

Setup installs pinned Rust 1.90.0 and checksum-pinned LLVM-MinGW 20260616 inside `Development/.tools`. GNU x64 Rust uses the pinned LLD linker and self-contained CRT. LLVM supplies dlltool and Windows resource tools; it is excluded from the portable app. Rust dependencies are locked in Cargo.lock. FFmpeg/FFprobe/libmpv are restored with their existing hashes; upstream rolling downloads may fall back to the release named by MediaMirrorTag. Authenticate GitHub CLI and set GH_REPO for the private mirror, or pass `-MediaBundlePath` to setup with an exact matching portable ZIP.

Default tests check formatting, strict Clippy, Rust unit/executable tests, native playback lifecycle, generated SDR/PQ/HLG media exports, pixel geometry, VFR stepping, audio/gaps/cancellation and persistence. `-Gpu` exercises available NVIDIA encoders. `-Ui` runs actual egui input and captures app framebuffers. `-Extended` adds portrait, anamorphic, RGB and multiple-video recordings to the desktop matrix. Generated fixtures and screenshots stay ignored. No user editor is stopped and no private recording is required.

The AV1/PQ integration fixture checks full and limited range with known primary colors and a neutral gray ramp through thumbnails, post-seek software preview, SDR export and HDR10 AV1 export. CPU runs generate a small fixture with libaom; `-Gpu` uses NVIDIA encoding at 2560×1440 and 60000/1001 fps. A recognizable/nonblack frame alone is not treated as color correctness.

Use `. ./Development/scripts/rust-environment.ps1` before direct Cargo commands. Build from the pinned workspace environment rather than a system toolchain.

`test-rust-desktop.ps1 -AppPath <candidate-exe> -Fixtures sdr.mkv -ScaleLayouts` checks four window sizes at 100/125/150/175/200% egui UI scale in native and software preview modes. It verifies actual Windows child-window bounds against scaled preview coordinates and saves each rendered layout. This changes only the isolated editor's UI zoom; it does not change Windows display settings or prove per-monitor DPI transitions. Injected input targets the isolated egui context without depending on another app surrendering OS focus. Native user input is excluded from that verification sequence; renderer screenshots are retained. Capture-hold and ordinary sessions retain native input. The default desktop matrix includes an audible two-track fixture and requires a working Windows audio output. It checks negotiated output and selection of the first probed track even when another track is marked default; this is separate from export-audio tests and null-output lab playback. Normal desktop runs also check Tab/Enter timeline selection, Shift+F10/Escape menus, keyboard edits/undo and Space transport while preserving text-entry ownership.

For desktop startup failures, `test-rust-startup.ps1 -AppPath <candidate-exe> -Cycles 100` repeats generated PQ import, exact seek, split, undo/redo and shutdown. Add `-GraphicsOnly` to isolate initial rendered frames without importing media. These isolated `--verify-desktop` processes write their phase and native exception/module addresses on failure; ordinary app sessions do not install the diagnostic handler. Reports remain in ignored artifacts, and the runner closes or times out only its own helper process.

`test-rust-launch.ps1 -AppPath <candidate-exe>` exercises ordinary filename startup with a Unicode/space project, multiple recording filenames and relative filenames. It opts into isolated recovery data and reporting through `SVE_VERIFY_LAUNCH_DIRECTORY`; normal startup parsing and dispatch still run. This mode relies on production event wakeups rather than the desktop verifier's frame timer. The desktop matrix also injects library and external-file timeline drops and verifies placement and undo. `verify-package.ps1 -Desktop` runs these against the relocated portable executable.

The UI sleeps while idle. Worker events and playback changes wake it; transport/seek gestures keep a short frame timer, unsaved projects keep the recovery deadline, and the playback actor checks display/HDR state once per second while paused. Software rendering wakes from libmpv's callback, explicit refresh/resize and shutdown. The video player is created only when there is a recording to preview. DX12 uses wgpu's memory-oriented allocation hint. Preview transforms follow the available physical display area in size tiers; project settings and export resolution stay unchanged. Native failures retain the dequeued/latest queued request for software fallback. Add `-NativeFallback` to `test-rust-desktop.ps1` for the opt-in failure-after-dequeue case. These scheduling choices require lifecycle, seek, import/recovery and actual resource checks; they are not an encoding-speed claim.

`compare-implementations.ps1 -LegacyApp <extracted-CSharp-1.0.4-bundle> -RustApp App -Source <generated-SDR-recording> -Visible -OrdinaryStartup` compares process resource use in C# 1.0.4 and Rust 1.1.0. Extract the original release ZIP into an ignored temporary directory when needed; obsolete baseline app folders are no longer required. Run it only when visible test windows are wanted. It samples separate launches, checks window state, records immutable artifact/native hashes and closes only its own children. `-OrdinaryStartup` opens the same generated project through each app's filename path; omit it only for older candidates that need the preview-hold harness. Ordinary preview readiness is not separately instrumented by this runner: first run the launch/desktop checks. It writes raw samples and means to ignored comparison output. Hidden launches are diagnostic because the toolkits can choose different minimized states. Preview sizes/backends differ; these samples are not an export-speed or cold-start benchmark. See the [comparison and retirement decision](rust-csharp-comparison.md) before removing legacy evidence.

`benchmark-rust-playback.ps1 -Gpu` generates twelve-second 3840×2160 SDR/PQ HEVC fixtures, verifies actual keyframes at zero and ten seconds, then records 24 exact seeks per backend/profile through three native lifecycles. Omit `-Gpu` to generate fixtures with libx265, or use `-ReuseFixturesFrom <previous-report-directory>` to reuse those generated files without rewriting them. Reports record CPU decoding, hardware/driver inventory, hashes, startup/load time, seek targets and median/p95/max acknowledgement times. Filesystem caching is uncontrolled. This backend diagnostic excludes desktop input and the shared project transform graph; it is not a latency guarantee for real recordings. Run it after other automated desktop checks finish for an isolated comparison.

`benchmark-preview.ps1 -AppPath 'App/Simple Video Editor.exe' -Source <local-recording>` measures the actual editor pipeline from UI request to playback-ready acknowledgement over 24 exact seeks. Add `-Software` for the CPU fallback. Native acknowledgement is not a physical display measurement; software requires the post-seek rendered frame. The isolated helper also streams actual pointer motion for two seconds, requires new preview frames during the gesture, checks exact release settling and verifies that same-presentation seeks retain the last valid frame, records actual decoder/backend, executable/source hashes and raw timings, and preserves the original recording. Run sequentially after other desktop/media work finishes; caching and system load remain uncontrolled. The older playback-lab benchmark excludes the project filter graph and cannot substitute for this check. `pwsh -File Development/scripts/benchmark-legacy-preview.ps1 -Source <SDR-recording>` restores the pinned 1.0.4 playback classes and runs their software renderer through the same 24 target positions. It requires a recording longer than 9.2 seconds and the pinned native DLL in App/bin. Only DLL resolution and a restart counter are instrumented; this reference excludes WPF input/presentation and is not a full-app latency comparison. Both helpers preserve recordings and record source/native hashes. See the comparison document for the bounded results.

Timeline filmstrips request only visible source-time samples, quantized to 100 ms. One cancellable worker processes the current bounded queue; the UI admits at most 128 visible samples, including cache hits, retains at most 128 sample textures and uses the library image while samples load. Counting cache hits in that budget prevents repeated decoding and eviction on crowded timelines. File cache entries include source identity, metadata and timestamp. Library images and waveform decoding use independent jobs. Background image decodes use two codec threads and one filter thread. Clearing the generated thumbnail cache includes filmstrip samples. Preview alone scales pixels before color conversion; exports retain the full-resolution graph. Ordinary Rec.709 YUV 4:2:0 SDR clips with square pixels, no rotation, Fit framing and matching output aspect use mpv's metadata-only `format` filter; the renderer handles scaling. Explicit tags preserve the shared graph's defaults for untagged video. Native direct preview requests `hwdec=auto`, allowing direct D3D11 surfaces with automatic CPU fallback. Processed AV1 still requests copied hardware frames; processed non-AV1 and Software preview retain CPU decoding. HDR, other color spaces, rotated/anamorphic sources, cropping/fill and aspect changes keep the shared transform. Export is unchanged. The direct-preview integration compares full/limited-range 8/10-bit primary colors and gray levels with the shared graph on CPU and direct GPU paths, including untagged metadata and exact seeks. Presentation signatures include source interpretation, section framing, output and HDR mode so pending seeks can retain a valid image and its HDR indicator without displaying a previous source or transform. During continuous pointer scrubbing, an in-flight frame finishes before taking the newest same-presentation target; pointer release, source/transform changes and frame steps interrupt immediately. The single pending-request slot stays bounded.

### Opt-in Shadowplay capture checks

With NVIDIA App's local MCP server already enabled, run `test-shadowplay.ps1 -LiveCapture -RecordingDirectory <game-recordings-folder> -ExpectedGameProcessId <pid> -Seconds 12 -RequireHdr`. It waits one minute for the API budget, refuses an existing recording or a different target, records briefly, stops its own recording and checks the new file's duration and HDR tags. It never enables API access, changes NVIDIA settings or deletes recordings. Reports, paths and media hashes stay in ignored artifacts. A passing report covers these checks only; inspect decoded frames to establish the actual recorded target and color correctness.

For a controlled editor comparison, launch an isolated `--verify-desktop <recording> <new-report-directory> --capture-hold` process. This uses the production import/preview paths and an isolated recovery directory, opens without taking focus from the game, and writes `capture-ready.json` when the preview settles. Add `--software-preview` for the SDR fallback. Add `--capture-hdr` to select HDR10/HEVC output; native verification then requires active display HDR and a PQ output transfer before signaling readiness. The report records the actual output gamma and display HDR state. Create `capture-stop` in that report directory to close the owned helper; it also times out after five minutes. It holds the imported clip without further timeline edits. Keep desktop screenshot/control sessions shut down during Shadowplay recording: NVIDIA reported Protected Content while one was active in the observed setup, and recording succeeded after ending it. Do not disable capture protection to run this check.

## Interface copy

Let layout and control names explain the editor. Avoid repeated headings, secondary descriptions and permanent shortcut legends; put optional technical detail and shortcuts in contextual tooltips. Keep action labels, source identity, active HDR/SDR state, meaningful metadata, warnings, errors and progress visible where they affect a decision. Preserve accessible names and keyboard behavior when simplifying the presentation.

Keep trim boundaries and quick trims visible in the inspector. Audio, framing and source color start collapsed; expanding a group must not create a project edit or hide color warnings. Keep expanded controls within the resizable sidebar.

Enabled buttons, dropdowns and toggles use the pointing hand cursor on hover. Set it on their responses with `on_hover_cursor`: the pinned egui version does not consume `Visuals::interact_cursor`. Keep text-entry and drag cursors specific to their interactions.

## Frozen legacy oracle

The old implementation is removed from production source. Optional differential tests restore only its source from Git commit `dd65df13a41490e8168a624b65fcff6a27956940` into ignored `Development/artifacts/rewrite-baseline/source`. The Rust application never loads it.

```powershell
./Development/scripts/setup.ps1 -LegacyOracle
./Development/scripts/test-rust.ps1 -LegacyParity
```

This optional path restores the pinned .NET SDK solely to compile the independent C# test oracle. Fetch full Git history if the baseline commit is unavailable in a shallow checkout. Ordinary Rust builds and packaged startup require no .NET SDK/runtime.

Graphite styling lives in `desktop-ui/src/ui/theme.rs`. It reads installed Segoe UI/Segoe UI Semibold/Consolas faces when available, retains egui fallback fonts, and never redistributes Windows font files. Body/button text is 14 logical pixels; metadata is 12.5 and timecode 13. The palette test requires at least 4.5:1 contrast for normal primary/secondary text and the enabled Export label. The real-input scale verifier checks Export stays within the top-right margin at a readable minimum size while transport/timecode and native preview bounds fit. These are app-scale checks, not full assistive-technology or physical DPI-transition certification.

The workspace uses progressive disclosure: empty recordings hide search/add controls, and an empty preview hides its transport. The timeline shows tracks/ruler when it has sections or an active recording drag; Undo stays available when history exists. File contains project actions; More options contains export settings, the software renderer toggle and cache maintenance. Preview mode changes retain their actor restart behavior, source protection and keyboard shortcuts. The desktop verifier captures the empty workspace and settled More menu, opens/closes File with actual input, and toggles the relocated software option twice without editing the project. Frame-arrow buttons retain accessible names and tooltips. Scale checks cover timecode and frame-control separation.

## Portable build

```powershell
./Development/scripts/build.ps1 -Zip -OutputName App-Updated
./Development/scripts/verify-package.ps1 -OutputName App-Updated -Desktop -Extended
./Development/scripts/test-packaging.ps1
```

The build stages a complete bundle, checks version/native pins and licenses, then replaces only its chosen stopped App/App-* output. It refuses to overwrite a running editor. ZIP names derive from Cargo's application version. The current local Releases pair is replaced/pruned only after embedded version, layout and checksum checks pass; unrelated files remain untouched. Published release assets are never replaced.

`build.ps1` is the single build entry point. `verify-rust-package.ps1` defaults to the current ZIP in `Releases` and tests the extracted executable from a relocated Unicode/space path, including absence of .NET, exact native pins, persistence/export/playback and optional desktop tests.

After verifying a complete bundle, run `clean.ps1 -BuildTools` (or pass `-OutputName App-Updated` for that output) to remove reproducible compiler/vendor files, Rust targets and C# build leftovers. It preserves app bundles, releases, projects and verification reports. Superseded local candidates and relocated test bundles were removed after the rewrite; the current executable is in `App`. Compact reports, final desktop screenshots, projects, the original C# release ZIP and the optional frozen source oracle remain available. The oracle restores its source from Git when needed; it is not part of the application.

The approved icon source is `Development/assets/app-icon.png`. Run `Development/scripts/make-icon.ps1` after editing it. Source assets and license notices are tracked; binaries, toolchains, caches, media, projects and verification results are ignored.

## Acceptance and repository policy

Physical HDR luminance/color accuracy, monitor movement/HDR toggles, 100–200% DPI, screen-reader navigation and concurrent game/Shadowplay capture require the corresponding hardware/session. Automated metadata and framebuffer checks do not prove these gates. Record unavailable combinations honestly in the [rewrite ledger](rust-rewrite-status.md).

Use feature branches and open PRs. Enable the tracked guard with `git config --local core.hooksPath .githooks`; never push directly to main. Keep PRs open until the user finishes testing and approves merge. Tags come from merged main, after checks, and workflows prepare draft releases only. See [release instructions](releasing.md).

The [2026-10-28 editor audit](editor-audit.md) records preview audio, action-label alignment, color-history refresh and focused cleanup findings.

## Release hardware checks

`test-rust-desktop.ps1 -AppPath 'App/Simple Video Editor.exe' -Fixtures audio.mkv -AudioRecovery` temporarily selects an unavailable endpoint in each isolated test player, then checks that production recovery reopens the default output while preserving the video clock. It never changes Windows audio settings. Ordinary desktop runs also check that New project releases the player, library previews and filmstrip worker/cache. An accessibility-tree unit test checks the preview-position slider, associated In/Out labels, exact values and named transport control.

`test-rust-displays.ps1 -AppPath 'App/Simple Video Editor.exe'` requires two connected monitors. It moves only its own native/software HDR-preview helpers out and back, checks fresh display state, the expected PQ/SDR transfer and actual child-window bounds, and closes those helpers. It preserves Windows settings and original recordings. With the current two 96-DPI monitors it establishes physical monitor movement, not a mixed-DPI transition. `--track-display` writes diagnostics only inside explicit held desktop verification.

Use the [release checklist](release-readiness.md) for the current gates and physical acceptance scope.
