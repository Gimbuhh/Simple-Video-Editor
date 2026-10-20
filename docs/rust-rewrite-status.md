# Rust rewrite status

Target: **1.1.0**. Started 2026-10-20. This is an incremental migration record,
not a claim that the Rust desktop editor or HDR workflow is ready.

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
| Rust foundation | 15 tests passed, formatting and Clippy passed, optimized inspector smoke passed. |
| C#/Rust differential import | Both committed fixtures passed exact numeric field comparisons against unchanged C# ProjectStore/TimelineLayout. |
| Exact release rollback | Downloaded original ZIP extracted and passed relocated saved-project preview/native pin/clean-close verification. |
| GPU/HDR/display/capture | Not verified by this slice. No UI framework or HDR rendering claim. |

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
| C08 | Original recordings remain untouched; LocalRecordingPath/ExportService | future Windows media authorization + export | Inspector has no recording access or write path. Full drive/link authorization and source-as-output protection still require porting. |
| C09 | Pointer owns scrubbing; stale decoder events cannot change play/pause intent; MainWindow/NativePlayer | future playback + desktop-ui | Baseline harness only; not ported. |
| C10 | Save/recovery/relink/cache and concurrent-session isolation | future project-store/cache | Baseline harness only; Rust importer cannot save or upgrade projects. |
| C11 | Exact exclusive cuts, silent/black gaps, linked audio and cancellation | future export/playback | Baseline media checks only; Rust does not export or play media yet. |
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
| M1 rendering/HDR spike | No | No | No | No |
| Full M2 domain/persistence | No | No | No | No |
| M3-M8 desktop/cutover | No | No | No | No |

There are no runtime bridges between the applications. The C# test oracle is
retained until differential migration coverage is complete, then removed when
fixtures provide an independent sufficient oracle. Delete C# production owners
only after their Rust replacements are connected to the desktop entry point,
verified in the actual package, and no remaining production callers use them.

Next: rendering/HDR spike comparing UI/video-surface integration, native libmpv
ownership and shutdown, genuine Windows HDR versus labeled SDR fallback, and
scrubbing responsiveness. Validate on a physical HDR display before freezing
the desktop framework or promising full HDR preview. Independent core work may
continue, but full UI migration waits for that gate.

Initial metrics: four Rust source/test modules, approximately 750 lines including
tests, three workspace packages and eleven locked registry dependencies. The
optimized inspector is about 1.65 MB. No legacy production code has been removed
and no duplicate runtime event handlers or C#/Rust bridges have been introduced.
This expected temporary source growth must be reversed at verified desktop
cutover; the current tests and inspector do not justify removing the C# app.

Rollback for this slice is the unchanged 1.0.4 release and original projects.
No schema upgrades or source-file writes occur. Keep the PR open for user review
and testing; no merge, release tag or publication is authorized yet.
