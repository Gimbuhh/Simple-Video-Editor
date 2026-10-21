# Rust rewrite status

Target **1.1.0**. Started 2026-10-20. Production source/build cutover is implemented on the PR branch. This is an unreleased candidate: an intermittent desktop startup crash is under investigation, and physical HDR, display/DPI/accessibility and game-capture acceptance still prevent declaring the full rewrite/release verified. No merge, tag or publication is authorized or performed.

## Current implementation — 2026-10-21

Rust owns the desktop, timeline commands/history, persistence/recovery/relinking, source authorization, native playback coordination, shared color/framing transforms, cache and export. The normal build now packages the Rust executable. `Development/rust/crates/desktop-ui/Cargo.toml` is the sole application-version authority; project metadata and Windows resources derive from it. Private core/tool crates retain version 0.1.0.

The superseded C#/WPF production tree and old production UI harness are removed. The optional C# reader oracle restores v1.0.4 source from Git commit `dd65df13a41490e8168a624b65fcff6a27956940` into ignored verification output, compiles it with the pinned SDK, and compares legacy import semantics. It is excluded from production and portable packages. Git and the exact previous release retain rollback. The authored icon source moved to Development/assets.

The selected UI is egui/eframe 0.32.3 with Glow and AccessKit. Pinned Rust 1.90.0 uses checksum-pinned LLVM-MinGW 20260616 tools and LLD with the self-contained CRT. A real GNU-ld startup failure from mixed Windows import tables was reproduced and corrected. Native FFmpeg/FFprobe/libmpv pins and MediaMirrorTag are unchanged. Dependency/font/runtime notices accompany the package.

## Current contract and ownership evidence

| Contracts | Authoritative owner | Verification |
| --- | --- | --- |
| Exact legacy trims, gaps, ordered/unused library and repeated recordings | project-store + editor-core | Frozen C# differential oracle, encoding/parser fixtures and exact schema-2 roundtrips |
| Linked split/delete/move/trim/swap/duplicate and bounded history | editor-core | Pure command tests, actual desktop keys/pointer drags, one-step undo/redo and Escape |
| Trim pointer anchoring and retained preview frame | editor-core + desktop-ui | Left edge preview holds right edge/followers; commit ripples once; removed preview frames clamp; Escape restores bounds/frame |
| Search focus, hidden selection, Add all library order | desktop-ui | Actual text/pointer input; hidden selection disabled; Add all includes filtered sources as one undo step |
| Scrub ownership, pause intent, endpoint and native clock | desktop-ui playback actor | Held cross-recording ruler, interleaved rapid seeks, final-generation settling, playing/paused cases and last retained frame |
| Source timestamps and video selection | native-mpv + media-engine | Known VFR PTS forward/reverse steps; multiple-video/default-track fixture; explicit stream mapping in preview, thumbnails and export |
| Source safety and local-only paths | media-path + platform-windows | Namespace/reparse checks, source hashes, save/export identity rejection, cancel preservation and hard-link-aware cache protection |
| Atomic persistence, recovery, relinking | project-store + media-engine | UTF-8/16/32, schema rejection, exclusive live-session leases, revision cancellation, recovery claim ownership and immutable relink/load |
| SDR/PQ/HLG, source interpretation, geometry | shared media-engine graph | CPU/NVIDIA exports, tagged 10-bit HDR10 decode, rotation/SAR/RGB pixel comparisons, deliberate SDR software transform |
| Preview delivery and lifecycle | native-mpv + playback actor | Bounded latest-request/frame mailboxes, post-seek software render serials, narrow FFI, render-thread join before core/window destruction |
| Display-aware HDR fallback | platform-windows + playback actor | Read-only active-monitor queries and SDK-layout tests; HDR requires active HDR mode, not WCG; unsupported/unknown state chooses SDR |
| Portable artifact and rollback | build.ps1 + portable verifier | Exact native hashes, PE/version/no-CLR checks, Unicode/space relocation, actual packaged persistence/export/playback; previous release SHA retained |

The packaged desktop matrix passed **310 checks across 16 runs**: native/software SDR, PQ, HLG, rotated, anamorphic, RGB, multiple-video and VFR fixtures. The main cases exercise actual input and four window sizes each. VFR cases additionally prove seek completion between timestamps, actual next-frame boundaries for trim-after, trim-before and undo. Native/software preview mode switches preserve the project. The production review executable's SHA-256 at that run was `9ec19c66fdefc7e9645c3634c7129ce3573dd09bce5a9710736c8419ebeddb55`. Reports/screenshots: ignored rust-desktop/c5c4634cf1a64d5a903fddd3a6e16704; portable proof: rust-package-verification/6bef32a1d6af4b9da7e73eb0df06ed0e.

Strict formatting/Clippy and **32 workspace/executable tests** pass, together with the frozen C# import comparisons and **48 native lifecycle/backend checks**. Four media integration tests pass through CPU and available NVIDIA paths, covering pixel geometry, VFR, phase/delay-aware waveforms, exact frame counts, black/silent gaps, gain, atomic cancellation, relinking and source preservation. Nine packaging safeguard checks pass. Physical HDR presentation remains unproven.

The production ZIP passed embedded version, exact native pins, no CLR/.NET, Unicode/space relocation, packaged persistence/export/playback, documentation links and missing-native-tool rejection without borrowing checkout tools. The shipping App folder and any running user editor are preserved by using App-Rust-11-Review. Local package replacement/pruning follows verified version/layout/checksum checks; published assets remain unchanged.

Current Rust inventory is 29 source/test/build modules, approximately 9,700 nonblank lines and 394 KB of text. The desktop UI is the largest production module (2,670 nonblank lines); the actual input verifier is separate (931). Growth versus the 1.x baseline includes HDR/color/audio/framing behavior, explicit worker/native/OS boundaries and integration tests. There are zero tracked files under the superseded production source/test directories, and the Rust crates contain no TODO/unimplemented stubs. The release executable is 16,440,832 bytes and the ZIP about 146 MB; size is not evidence of faster encoding or seeking.

The library-row virtualization follow-up passed 137 targeted desktop checks (executable fc88ed12498b8162f99679b633884cd79b5de3ee0525b4f2dd68858b50d806b4). After recovery, timestamp-precision and quick-trim fixes, the current executable **7fdcbe804f03e444d9b56deb5da75abfc7ea29940f3d8b007830f3154a09af93** passed **316 checks across 16 relocated packaged desktop runs**, plus **86 checks across four generated 4K SDR/PQ runs**. Reports: rust-desktop/9b1097eaf1eb410182e855a9bf548a14 and rust-desktop/80ec8096d05a4f339e4561aa979a6294; portable proof: rust-package-verification/e64fcc85c0fb4418b5f7cf92ee857272. ZIP/native-pin/no-.NET/relocation checks and all nine packaging safeguards passed. Strict formatting/Clippy, workspace tests and frozen C# comparisons passed again. Windows CI passed at quick-trim commit 1bbdfefa62edc1079370237d699800964213d7b6 (run 39611269237), as well as earlier cutover/follow-up commits. Documentation and the independent benchmark tool are checked separately before handoff.

## Outstanding acceptance gates

- Resolve the intermittent graphics/startup access violation observed in a debug desktop verification process. Windows' report lists graphics/capture/overlay modules but no libmpv; it provides no usable stack. Loaded overlays alone do not establish causation. Isolated startup/early-edit stress and opt-in native diagnostics are being used without changing or stopping other apps.
- Physical HDR display luminance/color accuracy, real display movement and Windows HDR toggles. An OS HDR flag, PQ target or ordinary screenshot is insufficient proof.
- Game/Shadowplay coexistence with a running game. No user game or capture session was opened or stopped.
- Full 100–200% DPI and assistive-technology navigation matrix. Automated window sizing and AccessKit integration are partial evidence.
- Representative game-footage/cold-cache performance acceptance and unavailable AMD/Intel encoder combinations. Generated 4K/long-GOP diagnostics below do not establish a universal latency guarantee.
- User testing and explicit merge/release approval. Keep PR #8 open; do not tag or publish.

Hard cuts remain the default. Transitions/audio crossfades follow after rewrite/HDR verification. Proxy editing, markers, export queues, independent music tracks and other platforms are outside the accepted parity scope.

### Follow-up correctness and startup investigation

Actual native/software VFR input checks now cover retrying a locked recovery write while export is busy, clearing obsolete recovery after a successful asynchronous save, and accepting unchanged in/out fields without rounding stored timestamps. Selecting a different section for a move anchors its preview to that section's retained start. These checks pass in both preview modes.

A later packaged native HLG run exposed a quick-trim race: a seek clears its request key before the following UI tick advances the generation, so a shortcut could read the previous generation's ready timestamp. Both quick trims now wait for the requested decoded frame. The playback acknowledgement carries its actual pre-step timestamp separately from the next frame used as an exclusive out boundary. Native/software VFR checks include trim-before while a seek is pending, and the normal desktop matrix presses trim-after before its seek completes. The legacy loader also derives application-version metadata from the same authoritative Cargo manifest.

The access violation remains a separate unresolved investigation; the quick-trim failure was a test assertion with a normal exit, not that native crash. The original crash's Windows report identifies execute access violation `0xc0000005`, no usable stack, graphics/capture/overlay modules and no libmpv in its loaded-module list. Module presence/absence alone does not establish the cause or exclude an earlier native teardown. One hundred startup/load/seek/shutdown cycles, one hundred early split/undo/redo cycles, two hundred isolated release graphics-startup cycles and two hundred packaged graphics-startup cycles passed without recurrence. Debug graphics startup produced 126 passing reports before executor interruption, followed by a completed 74-cycle run; the interrupted runner's final exit was not retained. Reports: startup-stress/c481b03e9180439eb502e47cc679d9c7, edit-stress/ec6ec1608efb47fe981421eab4e5affd, startup-stress/e1726fc94d93406d8691c7772a4a4d0c, startup-stress/fcb0a1c8ad044f65ade532ef2f41c004, startup-stress/08df50ad452e4e9ea800003d616a866c and startup-stress/fcf3d781bc9b40b1a22ba239fa860dc8. Instrumentation is confined to explicit verification processes and never changes another application's hooks, capture or HDR settings. Passing repetitions do not establish a fix. The user has agreed to test HDR and game/Shadowplay; results and merge approval remain pending.

### Generated 4K/long-GOP diagnostic

Twelve-second 3840×2160/30 fps HEVC fixtures have actual keyframes at zero and ten seconds. The PQ fixture applies a real floating-point color transform. Each backend/profile has 24 completed exact seeks across three player lifecycles, at eight nonsequential positions spanning the recording. Host: Ryzen 7 9800X3D, with an RTX 5070 Ti reported by Windows (driver 32.0.16.1714). Decoding is deliberately CPU; native rendering reports D3D11. The isolated run began with zero other automated desktop verifications. Filesystem caching and unrelated system load are uncontrolled. These timings exclude desktop input and the application's shared project filter graph; they are diagnostic evidence, not an improvement comparison or hardware acceptance result.

| Profile/backend | Median seek acknowledgement | p95 | Maximum |
| --- | --- | --- | --- |
| SDR / software | 607 ms | 1,463 ms | 1,705 ms |
| SDR / native SDR | 540 ms | 1,333 ms | 1,387 ms |
| PQ / native SDR | 360 ms | 874 ms | 1,088 ms |
| PQ / native automatic | 379 ms | 931 ms | 1,200 ms |

Reproduce with benchmark-rust-playback.ps1. Raw targets, samples, library pins, hardware inventory and source hashes are in ignored playback-benchmark/fbe6453b83964028a3a2dc100e36396a. A preceding run overlapped other desktop checks and is not the isolated comparison. Source files remained unchanged. Separately, the actual portable editor passed its input/geometry/quick-trim workflow on three-second 4K SDR/PQ sections (86 checks above); that does not substitute for representative recording benchmarks or physical HDR observation.

### Completion audit

| Gate | Evidence / remaining work |
| --- | --- |
| One new authoritative owner per responsibility | Current ownership table above; Rust desktop, core, persistence, media and Windows boundaries are production reachable. |
| Superseded owner and temporary bridges | C#/WPF production files are removed. The pinned C# reader remains test-only; build-rust.ps1 delegates to the one authoritative builder as a compatibility entry point. No C# runtime bridge is packaged. |
| Retained contracts and differential differences | Exact legacy imports, editing/history, source protection, persistence, geometry and actual input checks pass. Explicit parser/resource hardening and schema-2 additions are documented; physical interaction/HDR gates remain open. |
| Rebuild and artifact identity | Locked Cargo graph, pinned Rust/LLVM/native binaries, normal build.ps1 and passing Windows CI. Byte-identical clean builds/ZIP timestamps are not promised. Exact tested executable hash is recorded above. |
| Actual artifact and application harness | Relocated portable persistence/export/playback, 316 desktop checks, 86 generated 4K checks and nine packaging safeguards pass. The intermittent startup failure remains unresolved despite stress passes. |
| Live-target scope | Generated local fixtures and isolated helper processes only. User HDR/game-capture observations are pending; no remote production data was modified. |
| Rollback/restoration | Exact 1.0.4 release ZIP/hash retained and its relocated restoration previously verified; original schema-1 projects are preserved and upgrade saves use a new path. |
| Dead-code and metrics audit | Zero tracked superseded production files; no Rust TODO/unimplemented stubs. Current inventory, largest modules and artifact sizes are recorded above. The permanent native/software render alternatives serve distinct presentation/fallback purposes. |
| Metadata and publication | Cargo 1.1.0 owns version resources and package naming. README/changelog/guides/release notes describe an unreleased candidate and known blockers. PR #8 stays open; no merge, tag or publication occurred. |

Implementation cutover is present, but full rewrite acceptance is **not complete**: the original startup access violation still needs diagnosis, and physical HDR, display/DPI/accessibility, capture and representative-footage acceptance require the outstanding evidence listed above.

The dated foundation record below preserves original measurements and migration decisions. Its future-owner language and missing-feature statements describe the old foundation, not the current Rust production implementation.

## Historical foundation record — 2026-10-20

## Scope and version ownership

The [implementation plan](rust-rewrite-plan.md) tracks parity, HDR and release
gates. Hard cuts remain the default. Cross dissolve and optional audio crossfade
are follow-up work after the rewrite. No transition settings or schema are added
in this slice. Recommended additions and stretch work remain subject to capacity;
they must not delay parity and HDR verification.

`Development/src/SimpleVideoEditor/SimpleVideoEditor.csproj` still owns the
shipping version, **1.0.4**. Cargo's **0.1.0** identifies private migration tools,
not an application release or a second source of application version authority.
The release metadata changes to 1.1.0 only at verified cutover. Project schema
versions are independent of both values.

## Frozen baseline

- Published reference: [v1.0.4](https://github.com/Gimbuhh/Simple-Video-Editor/releases/tag/v1.0.4),
  source commit `dd65df13a41490e8168a624b65fcff6a27956940`.
- Starting checkout: `fb1fbf7641146d7a1d9211c56ad77bb44f1ec46b`; app source,
  existing tests, scripts and SDK pin have no changes relative to v1.0.4.
- Exact release artifact: `SimpleVideoEditor-1.0.4-win-x64.zip`, 199,337,788 bytes.
  SHA-256: `5503f27981238c9d38e9e0b31fce8075b0c26511727fdfd16f6c0ef772e86493`.
  Downloaded ZIP matches both the published digest and companion checksum.
- Baseline inventory: 34 tracked source/test files, 1,380,616 bytes including
  authored icon assets, 4,369 text lines in C#/XAML/project/manifest files.
- .NET SDK: 10.0.401, pinned in `global.json` and `dependencies.json`.
  Native tools and exact hashes remain pinned in `Development/scripts/dependencies.json`.
- Rust tools: rustc 1.90.0 (`1159e78c4`), Cargo 1.90.0 (`840b83a10`), GNU x64
  host for this foundation. Workspace-local installation avoids system changes
  and does not settle the desktop UI/FFI toolchain choice. Bootstrap origin and
  SHA-256 are pinned in `Development/scripts/rust-tools.json`.
- Rebuild: `./Development/scripts/build.ps1 -Zip -OutputName App-Rewrite-Baseline -PackageDirectory Development/artifacts/rewrite-baseline/rebuilt`.
  Rebuilt executable SHA-256: `235f1561aa4401aa1b3d77bf38a0ac72ea02003bc4d93ee5b8ee46ff06aed58f`.
  Rebuilt ZIP SHA-256: `a8e10756e3198f742d1bc3eb227ba0dcaa25c4ab92dc3778355bc2b1196ab9ec`.
  ZIP byte identity with the published archive is not claimed; generated archive
  metadata and current documentation can differ. The downloaded release remains
  the exact rollback artifact, separate from the reproduced build.

Local archives, reports, fixtures generated from the media tools, and test app
folders are ignored. No private recordings were imported or committed.

## Verification recorded on 2026-10-20

| Check | Result |
| --- | --- |
| Repository metadata/scripts | Passed before and after migration edits. |
| Existing core/media harness | 103 passed using generated fixtures, CPU encoding and software playback. |
| Existing desktop UI harness | 233 passed; callback-driven WPF harness, not external OS drag automation. |
| Reproduced portable artifact | Layout, embedded version, checksum, native pins, documentation, relocated project preview and clean close passed. |
| Packaging safeguards | 9 passed using the isolated baseline package directory. |
| Rust foundation and native boundary | 23 tests passed, formatting and Clippy passed, optimized inspector smoke passed. |
| C#/Rust differential import | Both committed fixtures passed exact numeric field comparisons against unchanged C# ProjectStore/TimelineLayout. |
| Exact release rollback | Downloaded original ZIP extracted and passed relocated saved-project preview/native pin/clean-close verification. |
| Rust software playback | 9 checks passed: generated SDR frames, serialized reload/seek/pause intent, missing-input failure, active-playback shutdown, and explicit PQ/HLG rejection. |
| Native D3D11 experiment | 48 checks passed across generated SDR/PQ/HLG, native SDR and automatic targets, including the software checks. Local automatic-mode reports show 10-bit PQ/BT.2020. |
| Physical HDR/toolkit/display changes/capture | Not verified. No UI framework selected and no full HDR presentation claim. |

Commands and fixture locations are in the [development guide](development.md).
No application source changed, so the baseline UI/media evidence remains valid
for this foundation PR. Rust behavior has its own separate evidence.

## Contracts and ownership

All C# production owners remain active in the shipping application. The new
owners below currently serve the Rust inspector or pure tests, not the desktop
entry point. Rust has no dependency on the C# runtime; `RustParityOracle` is
test-only and must remain outside app packaging.

| ID | Contract and reference owner | Intended Rust owner | Evidence / state |
| --- | --- | --- | --- |
| C01 | Saved trims keep their exact values; Models/ProjectStore | editor-core + project-store | Bit-preserving import test and direct C# comparison; verified in inspector only. |
| C02 | Legacy projects without Sources/TimelineStart reconstruct library and sequential placement | project-store | sequential.json, mixed-case test and C# oracle; verified in inspector only. |
| C03 | Leading/intermediate gaps, unused sources and repeated sections survive | project-store + editor-core | gaps-library.json, C# oracle, distinct clip IDs and exclusive-end mapping tests. |
| C04 | Split keeps both linked parts, surrounding placement and undo identity; MainWindow.SplitAt | editor-core | Pure split/undo/redo test; not connected to desktop UI. |
| C05 | Delete closes only the joined local group; MainWindow.Remove_Click / TimelineLayout.JoinedFollowing | editor-core | Tests cover joined group, separated group and undo; not connected to desktop UI. |
| C06 | Add-all is one edit independent of search filtering; MainWindow.AppendSources | editor-core command; future library owner | Batch append/undo test; library UI/filter behavior still belongs to C#. |
| C07 | Reject invalid/overlapping/extreme timeline values before media work; ProjectStore.ReadAsync | project-store + editor-core | Invalid input, bounded stream/count, future-schema and failed-command tests. |
| C08 | Original recordings remain untouched; LocalRecordingPath/ExportService | media-path + future export | Rust lab validates local drives/link targets and hashes generated sources before/after. Inspector still has no media access. Export source-as-output protection remains unported. |
| C09 | Pointer owns scrubbing; stale decoder events cannot change play/pause intent; MainWindow/NativePlayer | future playback + desktop-ui | Baseline harness only; not ported. |
| C10 | Save/recovery/relink/cache and concurrent-session isolation | future project-store/cache | Baseline harness only; Rust importer cannot save or upgrade projects. |
| C11 | Exact exclusive cuts, silent/black gaps, linked audio and cancellation | future export/playback | Rust lab verifies decode and serialized seeking only. Timeline playback/export, frame-exact presentation, audio sync and cancellation remain unported. |
| C12 | Portable version/layout/native pins and safe replacement | future Rust packaging | Existing packaging retained; inspector is not included in App or release ZIP. |

Intentional importer hardening: 8 MiB input limit, explicit collection limits,
control-character rejection, nonnegative placements, and rejection of ambiguous
mixed-case duplicate keys. These do not establish full parser parity. Windows
ordinal Unicode path comparison, UTF-16/32 input, duplicate-property edge cases,
and platform drive/reparse-point authorization require additional fixtures before
the importer can replace the production reader. Source/clip IDs are stable only
within the imported document/undo history; persistent schema-2 IDs remain future work.

## Migration and deletion gates

| Slice | Implemented | Production reachable | Verified | Legacy owner removed |
| --- | --- | --- | --- | --- |
| M0 baseline/ledger | Yes | Existing 1.0.4 artifact | Baseline checks above | Not applicable |
| Rust timeline foundation | Split/delete/append/undo subset | Pure tests only | Focused contracts | No; C# editor still owns the application |
| Rust schema-1 importer | Read-only subset | sve-inspect executable only | Unit, CLI and C# differential checks | No; saves/recovery/media authorization remain unported |
| M1 rendering/HDR spike | Native/software backend experiment | sve-playback-lab only, not editor UI | CPU/native tests; physical HDR and toolkit integration pending | No |
| Full M2 domain/persistence | No | No | No | No |
| M3-M8 desktop/cutover | No | No | No | No |

There are no runtime bridges between the applications. The C# test oracle is
retained until differential migration coverage is complete, then removed when
fixtures provide an independent sufficient oracle. Delete C# production owners
only after their Rust replacements are connected to the desktop entry point,
verified in the actual package, and no remaining production callers use them.

Next: connect a UI candidate to the native/software backend experiment, compare
video-surface integration and generation-safe pointer scrubbing, and test actual
HDR presentation. The standalone native windows establish a backend baseline;
they do not prove embedding, widget interaction, accessibility or DPI behavior.
Validate on a physical HDR display before freezing the desktop framework or
promising full HDR preview. Full UI migration still waits for the M1 gate.

Initial metrics: four Rust source/test modules, approximately 750 lines including
tests, three workspace packages and eleven locked registry dependencies. The
optimized inspector is about 1.65 MB. No legacy production code has been removed
and no duplicate runtime event handlers or C#/Rust bridges have been introduced.
This expected temporary source growth must be reversed at verified desktop
cutover; the current tests and inspector do not justify removing the C# app.

Rollback for this slice is the unchanged 1.0.4 release and original projects.
No schema upgrades or source-file writes occur. Keep the PR open for user review
and testing; no merge, release tag or publication is authorized yet.

## M1 backend experiment

The Rust runtime now owns its libmpv handle and render-worker lifecycle in the
standalone lab. It loads only the existing checksum-pinned DLL, disables config,
scripts, external references and audio output, and never invokes the C# app.
The loader uses the library directory and system directory, without changing
the process-wide DLL search path. ABI declarations use official 0.41 headers;
the runtime client API major version is checked separately.

Unsafe application code is confined to `native-mpv` and the local-drive query in `media-path`.
The domain, parser, inspector and lab entry point retain the unsafe-code ban.
The software renderer uses an aligned allocation, one-frame mailbox and an
atomic-only callback. Its thread unregisters callbacks and frees the render
context before the player is destroyed. Source authorization supports local
links and rejects network/device namespaces before media access. It retains the
baseline's filesystem race limitations and is not yet wired into project loading.

Generated three-second FFV1 fixtures include real linear-light conversion to
10-bit PQ and HLG. The native SDR mode reports sRGB/BT.709 for each source; the
automatic mode reports a 10-bit PQ/BT.2020 output target on this machine. This is
libmpv pipeline evidence, not a measured luminance or visual correctness result.
The machine exposes NVIDIA RTX 5070 Ti (driver 32.0.16.1714), AMD integrated
graphics and a virtual display adapter; adapter-specific behavior, mixed-display
movement and Shadowplay alongside a game are not established by those reports.

The software reference is deliberately SDR-only. A CPU tone-mapped HDR fallback
still needs implementing and matching against native preview/export. UI toolkit
prototypes, HDR/SDR display-state handling, pointer latency, color/geometry visual
checks and preview/export agreement remain M1 exit conditions. No C# production
owner can be removed yet. The shipping App and release metadata remain 1.0.4.

After this slice: ten Rust source/test modules, 1,725 lines including tests, six
workspace packages and twenty Windows registry dependencies (additional target
dependencies remain in Cargo.lock). The optimized playback lab is about 1.74 MB.
The original production source/tests and native pins are unchanged. Package
size growth is limited to documentation because Rust is not bundled in App.

## Bounded seek measurements

The post-acknowledgement lab run on this desktop used generated 320x180 FFV1 recordings, CPU decoding, three player lifecycles and four local seeks per lifecycle. These 12-sample summaries measure native seek completion, not pointer latency or physical HDR presentation. Concurrent verification was active; cache was not flushed. They are diagnostic observations rather than a controlled comparison with 1.0.4.

| Lab mode | Median ms | p95 / max ms |
| --- | --- | --- |
| software-sdr | 2.55 | 28.09 |
| native-sdr-sdr | 248.93 | 500.27 |
| native-sdr-pq | 247.12 | 497.96 |
| native-sdr-hlg | 245.44 | 511.25 |
| native-auto-pq | 247.31 | 510.68 |
| native-auto-hlg | 247.11 | 501.03 |

Native completion varied into the hundreds of milliseconds on these small fixtures, so no universal seek-speed improvement is claimed. The UI keeps pointer ownership while decoding and offers Software SDR preview as an explicit option. Long-GOP/4K, physical display and game-capture performance acceptance remain open.
