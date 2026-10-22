# Rust application workspace

This workspace owns the production Simple Video Editor 1.1.0 candidate. The application's Cargo.toml is the version authority. See the [user guide](../../docs/user-guide.md), [development commands](../../docs/development.md) and [contract/cutover ledger](../../docs/rust-rewrite-status.md).

Rust owns all production editor, persistence, media, playback, export and UI responsibilities. No C#/.NET bridge or runtime is packaged. The legacy C# project is restored from an exact Git commit only for isolated differential tests. Current-display HDR preview and local game capture checks pass; the user confirmed the separate game-switch recording issue is external. Broader display/DPI/accessibility and startup follow-up remain tracked in the ledger; the PR stays open for user testing.

```powershell
./Development/scripts/setup.ps1
./Development/scripts/test.ps1 -Ui -Gpu -Extended
./Development/scripts/build.ps1 -Zip -OutputName App-Updated
./Development/scripts/verify-package.ps1 -OutputName App-Updated -Desktop -Extended
```

Omit -Gpu without compatible NVIDIA hardware. Native pins are unchanged. Rust 1.90.0 and checksum-pinned LLVM-MinGW supply workspace-local build tools. The portable package includes locked crate/font/Rust notices and needs no installed runtime. Core/private tool crates retain workspace version 0.1.0.

## Historical foundation and backend notes — 2026-10-20

The notes below describe the original inspector/lab stage. Their missing-editor
limitations are superseded by the candidate described above.

Run from the repository root on Windows x64:

```powershell
./Development/scripts/setup-rust.ps1
./Development/scripts/test-rust.ps1

# After setup.ps1 restores the C# baseline tools:
./Development/scripts/test-rust.ps1 -LegacyParity

# CPU rendering/lifecycle checks with generated SDR/PQ/HLG fixtures:
./Development/scripts/test-rust.ps1 -LegacyParity -Playback

# Local D3D11 tests open temporary native video windows:
./Development/scripts/test-rust-playback.ps1 -Gpu

# Inspect an existing project without writing it or opening its recordings:
./Development/rust/target/release/sve-inspect.exe 'C:/projects/example.sveproject'
```

`editor-core` owns pure timeline rules. `project-store` imports schema-1 data
without touching media. `sve-inspect` prints a path-free JSON timeline summary.
The test-only C# oracle compares committed generated-data fixtures against the
unchanged production reader. It is never a runtime dependency of Rust.

Legacy floating-point trim values remain intact. Rational frame rates describe
new split constraints; this is not a finished rational/VFR timing engine.
The project importer's syntax checks are not filesystem authorization. The
playback lab uses `media-path` to validate drives and link targets before access;
the future editor must apply the same boundary to imported source references.
No save/upgrade/export/recovery path is present.

Rust 1.90.0, bootstrap checksum and Cargo.lock are pinned. Tools, downloaded
crates and build output stay ignored inside the workspace. GNU is sufficient
for this pure foundation; the rendering spike will settle production tooling.
The workspace version 0.1.0 identifies private tools; the C# project still owns
shipping version 1.0.4 until verified cutover to application version 1.1.0.

Workspace source uses the repository MIT license. Locked crates retain their own
license terms; inspect `cargo metadata --locked --offline` for exact versions and
license expressions. Runtime dependencies include Serde/serde_json (MIT or
Apache-2.0), memchr (Unlicense or MIT), itoa (MIT or Apache-2.0), and ryu
(Apache-2.0 or BSL-1.0). Build-time unicode-ident additionally requires Unicode-3.0.
The future Rust portable package must include the matching notices and runtime
toolchain licenses before distribution; the current App bundle contains no Rust.

## Playback experiment

`media-path` checks local drives and immediate link targets before recording
access. `native-mpv` verifies the pinned DLL hash before loading it with a
restricted Windows DLL search. Its control handle stays on its owning thread;
the software render API has a separate worker, bounded one-frame mailbox and
an atomic-only update callback. Shutdown unregisters callbacks and joins that
worker before destroying the player and unloading the library.

`sve-playback-lab` exercises repeated load/play/pause/exact-seek/shutdown using
one of three modes:

| Mode | Purpose |
| --- | --- |
| `software` | CPU decode and in-memory 8-bit SDR frames; explicitly rejects PQ/HLG. |
| `native-sdr` | mpv-owned D3D11 window, forced sRGB/BT.709 target; native renderer converts HDR sources. |
| `native-auto` | mpv-owned D3D11 window with automatic color-space hints; reports the negotiated target without claiming physical HDR correctness. |

After the test build, invoke the lab with a mode, an absolute path to the pinned
`libmpv-2.dll`, and an absolute local recording path. The executable prints a JSON
report and exits; native modes briefly open their own windows. Audio output is
disabled. This is a backend diagnostic, not an interactive editor or an embedded
Slint window. Neither UI toolkit selection nor the M1 exit gate is complete.

The PowerShell test runner generates actual SDR-to-PQ/HLG sample conversions
with the pinned FFmpeg, rather than changing metadata alone. Reports/media stay
ignored under `Development/artifacts/rust-playback/`. The default CI runs the
software checks; native D3D11 remains an explicit local check.

Seeks/loads are serialized and bounded in the lab. Generation-safe pointer
scrubbing, source-switch preview presentation, correct HDR/SDR screenshots,
audio synchronization, HDR display changes, and toolkit embedding remain future
work. Do not run this synchronous lab control API on a future UI thread.
The same filesystem race limitations as the existing path validator apply;
this is not a guarantee against another process replacing links during access.

The native ABI declarations were checked against the official
[mpv 0.41 client header](https://github.com/mpv-player/mpv/blob/v0.41.0/include/mpv/client.h)
and [render header](https://github.com/mpv-player/mpv/blob/v0.41.0/include/mpv/render.h).
Runtime client API major version and the exact repository DLL hash are checked
independently. Hashing uses the locked sha2 dependency (MIT or Apache-2.0);
production packaging must include its transitive notices too.
