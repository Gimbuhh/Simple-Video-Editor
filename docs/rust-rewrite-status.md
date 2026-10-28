# Rust 1.1.0 status — 2026-10-28

The Rust production cutover, graphite interface and identified software fixes are implemented on PR #8. Version 1.1.0 remains an unreleased candidate prepared for final user acceptance. No merge, release tag or publication is authorized by this preparation.

## Current button-alignment candidate

Executable SHA-256: `de247583a86ad8d1c14f15da16e16f2df608361c1b051ef9526b2282c8900d97`. Action captions center their painted glyph bounds on the physical pixel grid. The regression covers 2,400 installed/fallback-font, scale, layout, size, label and interaction-state combinations. Actual raster measurements put Import at zero horizontal/vertical offset, correcting the prior one-pixel right/two-pixel down displacement.

The alignment pass verifies 42 core tests, strict formatting/Clippy, 525 desktop checks across 18 native/software runs, 63 checks across 40 scaled layouts, ordinary filename startup, relocated portable checks and nine packaging safeguards. Evidence: button-alignment/verification.json, rust-desktop/490cf1982552404c879bed2ac935269d and rust-desktop/c558af021b8c40e79c43df63bd9c42cb. Media/device/resource results below retain their earlier executable hash; those processing paths are unchanged by this caption-only change.

## Release baseline

Executable SHA-256: `1e3599a09e3537098ffcfbe097d05d549496e754a427be90f46459e30dfc05b0`. Cargo's desktop-ui version remains the single source of application/package versioning.

| Gate | Result and scope |
| --- | --- |
| Core and source protection | 42 workspace/executable tests; strict formatting/Clippy; immutable recordings, exact trim/gap/history contracts. |
| Full desktop matrix | 525 checks across 18 native/software SDR, PQ, HLG, geometry, two-track audio and VFR runs. |
| Audio recovery | 67 checks across two isolated runs, including recovery from an unavailable player endpoint. Windows audio settings remain untouched. |
| Native failure fallback | 68 checks across two runs; dequeued requests survive a forced native failure and reach software rendering. |
| Layout and accessibility | 63 checks across 40 layouts at 100–200% app scale; named position/transport controls and labelled exact-valued trim spinboxes in the accessibility tree. |
| Physical monitor movement | Native/software round trips between connected HDR and SDR displays preserve child bounds and select PQ/SDR correctly; both displays use 96 DPI. |
| Lifecycle | 20 generated PQ startup/import/seek/edit/shutdown cycles; New project releases playback, library previews and filmstrip resources. Opening a project uses the same preview-release path. |
| Media and packaging | GPU/media integrations, source hashes, export/cancel safety, ordinary filename startup, relocated portable verification and nine packaging safeguards pass. |
| Resource comparison | Three visible launches per case on the current executable. Native preview working set is lower in the generated 4K case; empty/software cases retain a memory tradeoff. |

The [editor audit](editor-audit.md) describes the fixes. The [C#/Rust comparison](rust-csharp-comparison.md) gives measurement conditions and avoids claiming a universal memory or speed win. The [release checklist](release-readiness.md) records approval and physical acceptance gates.

## Contracts retained

- `editor-core` owns linked sections, exact source timestamps, gaps, snapping, ripple rules and undo/redo.
- `project-store` owns bounded legacy import, schema-2 persistence, project validation and recovery metadata. Schema-1 originals and the published C# app remain available for rollback.
- `media-engine` owns the shared color/framing graph, probing, thumbnails/waveforms, verified atomic exports and cancellation. Export always reads original recordings.
- `media-path` and `platform-windows` enforce local file/source identity boundaries and Windows display/window integration.
- `native-mpv` owns the verified DLL, thread-confined control handle and renderer teardown. The desktop actor owns asynchronous generations, latest-request/frame delivery and audio-output recovery.
- The UI preserves keyboard focus, named transport controls, exact trim fields, drop placement and progressive disclosure. Ordinary startup and idle work use production event wakeups.

## Remaining physical acceptance

Current-display HDR appearance is user-confirmed. Bounded game/Shadowplay checks pass; the user independently confirmed the separate HDR-to-SDR game-switch recording corruption is external to this project. That investigation remains closed.

Manual Windows HDR toggles, genuine mixed-DPI monitor transitions and a complete screen-reader workflow still require the corresponding setup and user observation before final release approval. Current tests cover actual HDR/SDR monitor movement at 96 DPI, app scaling and accessibility-tree semantics. They do not measure physical HDR luminance, headphone loudness or every device hotplug combination.

An earlier Glow startup access violation remains unattributed. Production uses DX12; current startup stress passes. Prior investigation reports remain available rather than being described as a proven fix.

## Evidence and history

Current ignored artifacts: release-1.1/verification.json; rust-desktop/8c631cba89544efb9cfc785fb644d000, 1ab5904c15cb44d1a6a1556c82fbb0f8, 1ca59d874db84310955cd26ddf949d24 and 1b83dccc8b6f4651977b0219593b9e4d; rust-displays/987f0a6c2a2f4bd1b1f193e90cfd52fd; startup-stress/a87096facb1a45dd94496473720e4719; version-comparison/b7c46c74995e41e5909d73231255fe97; rust-package-verification/f9324bb3860c4d0a9dd7d7c47552edaa.

The [previous ledger](https://github.com/Gimbuhh/Simple-Video-Editor/blob/3817dd3002c495f64f92f18c7d0b6ef94d64ce4c/docs/rust-rewrite-status.md) preserves the detailed rewrite, HDR/capture, seek and startup investigation history. Earlier measurements keep their original scope and artifact hashes; they are not silently reassigned to this executable. Native pins/licenses remain unchanged. Published assets and original recordings are preserved.
