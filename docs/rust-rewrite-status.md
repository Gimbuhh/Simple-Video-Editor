# Rust rewrite status

Target: **1.1.0**. Started 2026-10-20. This is an incremental migration record,
not a claim that the rewrite has completed its cutover and HDR release gates.

## Current implementation — 2026-10-21

The Rust application now has a real egui/eframe desktop entry point with an
embedded libmpv video surface, a recordings library, linked timeline lanes,
split/delete/move/trim/duplicate/swap, history, project persistence, recovery,
relinking, generated previews, audio controls, framing and output settings.
Media probing, playback control, cache generation and export run on workers.
There is no C# runtime bridge in the Rust candidate.

`desktop-ui/Cargo.toml` identifies the **1.1.0 test candidate**. The shipping
application and release metadata remain **1.0.4**, owned by the existing C#
project, until the verified cutover. `build-rust.ps1 -Zip` writes a separate
`App-Rust-Candidate` and ignored candidate ZIP/checksum. It does not replace App,
the current Releases pair, a running editor, or a published release.

The UI candidate uses eframe 0.32.3 with Glow and AccessKit. Slint was examined;
the egui integration was selected for this candidate after checking its pinned
source and licenses. Rust 1.90.0 remains pinned. A checksum-pinned LLVM-MinGW
20260616 supplies dlltool, LLD and Windows resource tools. LLD plus Rust's
self-contained CRT avoids unresolved mixed-generation Windows import tables;
an actual startup crash under GNU ld was reproduced and corrected. Build tools
stay outside the portable bundle. Windows version and icon resources identify
the candidate as 1.1.0.0.

Current verification includes:

- Pure tests for connected/free trim placement, duplicate/swap, bounded history,
  metadata inheritance and rollback, legacy parsing, UTF-8/16/32 documents,
  atomic saves, recovery leases and cancellation of obsolete recovery writes.
- Actual egui input tests for split, undo/redo, play/pause, linked pointer moves,
  one-step move undo and Escape cancellation. Native and software cases cover
  generated SDR, PQ and HLG: 63 checks across six desktop runs.
- Coalesced latest-request playback tests: rapid seeks settle at the final
  position; the UI follows the media clock and freezes while preparing media.
- Mixed SDR/PQ/HLG exports through CPU and NVIDIA H.264/AV1 and HDR10 HEVC/AV1.
  The strengthened CPU checks decode black/silent gaps, verify exact frame counts
  and gain-adjusted audio, roundtrip settings, relink moved recordings, and
  preserve prior output when canceled before or during encoding.
- Delayed opposite-phase stereo peaks retain their timing and amplitude. Cache
  cleanup deletes only recognized generated files, preserving unrelated files
  and recovery data.

The editor's software fallback deliberately converts HDR to SDR before rendering;
the diagnostic lab's original raw software mode still rejects unconverted HDR.
Native HDR display preview is selectable for HDR10 output. Negotiated target
metadata and screenshots do **not** prove physical HDR brightness/color accuracy.
Physical HDR, display changes, game/Shadowplay coexistence, the full interaction
and accessibility matrix, and remaining geometry/parity checks are still open.

The Rust candidate owns its own full workflow, but C# production owners have not
been deleted: packaged parity and the remaining release gates must justify that
cutover first. No merge, release tag or publication has occurred. Continue the
rewrite to those gates; this candidate is not the completed rewrite.

Portable verification extracted the candidate ZIP into a relocated folder,
checked its layout/checksum/version/native pins and absence of a CLR header or
.NET runtime, then ran persistence, export, software playback and all six
desktop cases from that packaged executable. Current Rust source/test inventory
is 23 modules and approximately 6,900 nonblank lines; the largest module is the
desktop coordinator/views (about 2,000 lines). The candidate executable is about
15.9 MB and the ZIP about 146 MB. Temporary source growth includes retained C#
owners, the native/OS boundaries and new HDR/audio/persistence tests. Delete the
superseded production tree only after the remaining parity gates are covered.

The following sections retain the dated foundation evidence and its original
deletion gates. Statements about missing desktop/persistence paths below describe
the 2026-10-20 foundation, superseded by the current implementation above.

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
