# Development

## Setup and build

Use Windows x64 and PowerShell. Run commands from the repository root. No globally installed .NET SDK or video tools are required.

The canonical repository is [Gimbuhh/simple-video-editor](https://github.com/Gimbuhh/simple-video-editor). This local project is its Git checkout; use commits and that remote for future work rather than source-backup ZIPs. For a new checkout:

```powershell
git clone https://github.com/Gimbuhh/simple-video-editor.git
cd simple-video-editor
git config --local core.hooksPath .githooks
```

```powershell
./Development/scripts/setup.ps1
./Development/scripts/check-repository.ps1
./Development/scripts/test.ps1
./Development/scripts/build.ps1 -Zip
./Development/scripts/verify-package.ps1
```

`global.json` and `Development/scripts/dependencies.json` pin the same SDK version. Setup downloads it into `Development/.tools/`, verifies the archive, and removes the download afterward. On a fresh checkout, setup downloads verified libmpv and FFmpeg binaries into `Development/vendor/`. When an existing `App/` has those exact files, setup reuses them.

Build produces the self-contained app under `App/`, a version-named local shortcut for the chosen output folder, and optionally `Releases/SimpleVideoEditor-<version>-win-x64.zip` plus its SHA-256 checksum. The version comes from the app project. The portable ZIP has four top-level entries: `Simple Video Editor.exe`, `README.txt`, `bin/`, and `docs/`. The executable bundles the .NET runtime; `bin` contains libmpv, FFmpeg, FFprobe, and their libraries, while `docs` contains guides and licenses. Keep the extracted folder together. Builds stage a complete bundle before replacing a generated output and refuse to overwrite a running editor. ZIP builds verify the replacement's embedded app version, contents and checksum before removing older generated package pairs from the local package directory. Unrelated files and published GitHub assets stay intact.

Upstream rolling media releases can remove old assets. Setup never silently changes versions: if a pinned download fails, CI tries the portable ZIP on this repository's pinned `MediaMirrorTag` release and verifies every native binary before restoring it. It accepts versioned assets and the unversioned names on older releases. Locally, supply a matching ZIP with `./Development/scripts/setup.ps1 -MediaBundlePath ./Releases/SimpleVideoEditor-1.0.4-win-x64.zip`. To use a private mirror automatically, authenticate GitHub CLI and set `GH_REPO` to the repository's `owner/name`. The first repository upload should preserve the verified portable ZIP in a draft release; see the release guide.

Run `./Development/scripts/test-packaging.ps1` after building a ZIP. These checks cover stale-package cleanup, a failed replacement retaining the previous package, unrelated-file preservation, misleading filenames and extra bundled apps. Both Windows workflows run them. For isolated packaging experiments, pass the same `-PackageDirectory` to build and package verification; this keeps the published local download unchanged.

When the app is open, use `./Development/scripts/build.ps1 -Zip -OutputName App-Updated`. Save and close the existing editor before switching builds. This alternative output is also ignored by Git.

## Verification

```powershell
# Generated fixtures, CPU encoding, and software playback; no NVIDIA GPU required.
./Development/scripts/test.ps1

# Verify the NVIDIA AV1 path with a compatible GPU and driver.
./Development/scripts/test.ps1 -Gpu

# Actual WPF window and editing; run in a desktop session.
./Development/scripts/test.ps1 -Ui

# Optional private recordings, supplied explicitly.
./Development/scripts/test.ps1 -Gpu -Clips @('D:/recordings/first.mp4', 'D:/recordings/second.mp4')
./Development/scripts/test.ps1 -Ui -Clips @('D:/recordings/first.mp4', 'D:/recordings/second.mp4')
```

The core real-recording check needs at least 22 seconds in each recording. Optional UI gameplay/performance checks expect two SDR 1440p/60 fps AV1 recordings with audio: at least 40 seconds in the first and 20 seconds in the second. Omit `-Clips` for portable generated-fixture checks. Missing explicit paths fail instead of silently skipping verification.

Run core tests before UI tests; core tests generate the fixtures consumed by the UI harness. UI checks dispatch WPF control/pointer callbacks in-process; they do not automate external operating-system drag gestures or file pickers. Default CI runs core/media checks and builds the portable app. GPU and desktop UI checks remain local integration checks.

Reports, screenshots, media fixtures, recovery files, and benchmarks stay under ignored `Development/artifacts/verification/`. Reports may contain private paths or recording frames; do not commit them. The curated README screenshot under `docs/images/` shows Arc Raiders gameplay captured in the editor; the original recordings remain outside Git.

Core checks cover exact non-keyframe cuts, frame counts, joins, gap silence, source protection, cancellation, CPU encoding, projects, relinking, recovery, cache cleanup, and native software playback. UI checks cover linked-track editing, playback, undo/redo, search, relinking, cache dialogs, dark loading states, text alignment, and layout.

## Local files and cleanup

Git ignores packaged apps, release ZIPs, build tools, native dependencies, build output, local source backups, shortcuts, projects, and verification artifacts. Only source, tests, scripts, authored assets, and repository documentation belong in commits. Git history replaces source-backup ZIPs after the repository is in use; existing local backups are retained.

```powershell
./Development/scripts/clean.ps1 -BuildTools
```

Cleanup verifies the `App/` media bundle, stops build servers, and removes reproducible build output, vendor copies, and workspace build tools. It preserves the app, releases, source, local history, and verification reports. Run setup before building again.

## Rust 1.1 rewrite

The [rewrite plan](rust-rewrite-plan.md) and [status/contract ledger](rust-rewrite-status.md)
track the authorized migration. The current portable app remains 1.0.4 until
Rust cutover; transitions are follow-up work.

```powershell
./Development/scripts/setup-rust.ps1
./Development/scripts/test-rust.ps1 -LegacyParity
./Development/scripts/test-rust.ps1 -LegacyParity -Playback

# Explicit local GPU rendering checks; opens temporary mpv video windows.
./Development/scripts/test-rust-playback.ps1 -Gpu
```

Run the existing setup first for `-LegacyParity`, which builds a test-only C#
oracle. Omit that switch for Rust-only development. The same checks run in the
Windows PR workflow. Rust tools and crate caches install in ignored
`Development/.tools/`; build output is ignored under `Development/rust/target/`.
Setup uses the checksum-pinned bootstrap from the official
[rustup archive](https://rust-lang.github.io/rustup/installation/other.html).
It does not modify system PATH or replace the portable app.

After testing, `Development/rust/target/release/sve-inspect.exe <project.sveproject>`
prints a read-only, path-free summary without opening recordings. The Rust
[workspace guide](https://github.com/Gimbuhh/Simple-Video-Editor/tree/main/Development/rust) describes its current limits.
Schema-1 fixtures are committed under `Development/rust/crates/project-store/tests/fixtures/`;
personal projects, media, and all generated verification output remain ignored.

The playback lab tests the pinned libmpv from Rust, using software SDR rendering
and optional native D3D11 SDR/automatic targets. The default CI includes generated
SDR/PQ/HLG fixtures, repeated load/seek/shutdown, native-hash protection and path
checks. The lab's raw software mode rejects unconverted HDR. The desktop uses
a shared FFmpeg tone-mapping graph for its explicit SDR software fallback.
Native reports include source and target color properties; these do not certify
visible HDR output. Logs and generated fixtures are ignored under
`Development/artifacts/rust-playback/`. The desktop candidate embeds libmpv in
egui/eframe; physical HDR and the full cutover gate remain open.

```powershell
# Actual Rust window, pointer/keyboard input and native/software preview:
./Development/scripts/test-rust-desktop.ps1

# Mixed SDR/PQ/HLG exports, decoded gaps/audio and source protections:
./Development/scripts/test-rust-media.ps1
./Development/scripts/test-rust-media.ps1 -Gpu

# Separate 1.1 test bundle; keeps the shipping App and Releases pair intact:
./Development/scripts/build-rust.ps1 -Zip
./Development/scripts/verify-rust-package.ps1 -Desktop
```

Run `test-rust-playback.ps1` first to create the generated SDR/PQ/HLG fixtures.
The Rust desktop harness injects pointer and keyboard events into the actual egui
input pipeline and captures its own framebuffer. It does not certify external
OS dialogs, physical HDR presentation or a game capture session. The portable
check relocates the ZIP, checks PE/version/native pins and absence of a managed
runtime, then exercises persistence, CPU export and libmpv from the packaged
executable. Desktop checks require a Windows desktop session with OpenGL support.

## Icon and dependencies

The approved icon source is `Development/src/SimpleVideoEditor/Assets/app-icon.png`. It was generated with imagegen for this project. Run `./Development/scripts/make-icon.ps1` after changing it, then rebuild.

Dependency versions, origins, and checksums live in `Development/scripts/dependencies.json`. Update them together and verify fresh setup, build, core tests, and local UI/GPU checks when changing the pinned bundle. Preserve the [third-party notices](../THIRD-PARTY-NOTICES.md) and packaged licenses.
