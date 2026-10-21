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

For desktop startup failures, `test-rust-startup.ps1 -AppPath <candidate-exe> -Cycles 100` repeats generated PQ import, exact seek, split, undo/redo and shutdown. Add `-GraphicsOnly` to isolate initial rendered frames without importing media. These isolated `--verify-desktop` processes write their phase and native exception/module addresses on failure; ordinary app sessions do not install the diagnostic handler. Reports remain in ignored artifacts, and the runner closes or times out only its own helper process.

`benchmark-rust-playback.ps1 -Gpu` generates twelve-second 3840×2160 SDR/PQ HEVC fixtures, verifies actual keyframes at zero and ten seconds, then records 24 exact seeks per backend/profile through three native lifecycles. Omit `-Gpu` to generate fixtures with libx265, or use `-ReuseFixturesFrom <previous-report-directory>` to reuse those generated files without rewriting them. Reports record CPU decoding, hardware/driver inventory, hashes, startup/load time, seek targets and median/p95/max acknowledgement times. Filesystem caching is uncontrolled. This backend diagnostic excludes desktop input and the shared project transform graph; it is not a latency guarantee for real recordings. Run it after other automated desktop checks finish for an isolated comparison.

### Opt-in Shadowplay capture checks

With NVIDIA App's local MCP server already enabled, run `test-shadowplay.ps1 -LiveCapture -RecordingDirectory <game-recordings-folder> -ExpectedGameProcessId <pid> -Seconds 12 -RequireHdr`. It waits one minute for the API budget, refuses an existing recording or a different target, records briefly, stops its own recording and checks the new file's duration and HDR tags. It never enables API access, changes NVIDIA settings or deletes recordings. Reports, paths and media hashes stay in ignored artifacts. A passing report covers these checks only; inspect decoded frames to establish the actual recorded target and color correctness.

For a controlled editor comparison, launch an isolated `--verify-desktop <recording> <new-report-directory> --capture-hold` process. This uses the production import/preview paths and an isolated recovery directory, opens without taking focus from the game, and writes `capture-ready.json` when the preview settles. Add `--software-preview` for the SDR fallback. Add `--capture-hdr` to select HDR10/HEVC output; native verification then requires active display HDR and a PQ output transfer before signaling readiness. The report records the actual output gamma and display HDR state. Create `capture-stop` in that report directory to close the owned helper; it also times out after five minutes. It holds the imported clip without further timeline edits. Keep desktop screenshot/control sessions shut down during Shadowplay recording: NVIDIA reported Protected Content while one was active in the observed setup, and recording succeeded after ending it. Do not disable capture protection to run this check.

## Frozen legacy oracle

The old implementation is removed from production source. Optional differential tests restore only its source from Git commit `dd65df13a41490e8168a624b65fcff6a27956940` into ignored `Development/artifacts/rewrite-baseline/source`. The Rust application never loads it.

```powershell
./Development/scripts/setup.ps1 -LegacyOracle
./Development/scripts/test-rust.ps1 -LegacyParity
```

This optional path restores the pinned .NET SDK solely to compile the independent C# test oracle. Fetch full Git history if the baseline commit is unavailable in a shallow checkout. Ordinary Rust builds and packaged startup require no .NET SDK/runtime.

## Portable build

```powershell
./Development/scripts/build.ps1 -Zip -OutputName App-Updated
./Development/scripts/verify-package.ps1 -OutputName App-Updated -Desktop -Extended
./Development/scripts/test-packaging.ps1
```

The build stages a complete bundle, checks version/native pins and licenses, then replaces only its chosen stopped App/App-* output. It refuses to overwrite a running editor. ZIP names derive from Cargo's application version. The current local Releases pair is replaced/pruned only after embedded version, layout and checksum checks pass; unrelated files remain untouched. Published release assets are never replaced.

`build-rust.ps1` is a compatibility entry point that invokes this same build with a separate candidate output/package directory. `verify-rust-package.ps1` tests the extracted executable from a relocated Unicode/space path, including absence of .NET, exact native pins, persistence/export/playback and optional desktop tests.

The approved icon source is `Development/assets/app-icon.png`. Run `Development/scripts/make-icon.ps1` after editing it. Source assets and license notices are tracked; binaries, toolchains, caches, media, projects and verification results are ignored.

## Acceptance and repository policy

Physical HDR luminance/color accuracy, monitor movement/HDR toggles, 100–200% DPI, screen-reader navigation and concurrent game/Shadowplay capture require the corresponding hardware/session. Automated metadata and framebuffer checks do not prove these gates. Record unavailable combinations honestly in the [rewrite ledger](rust-rewrite-status.md).

Use feature branches and open PRs. Enable the tracked guard with `git config --local core.hooksPath .githooks`; never push directly to main. Keep PRs open until the user finishes testing and approves merge. Tags come from merged main, after checks, and workflows prepare draft releases only. See [release instructions](releasing.md).
