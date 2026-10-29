# Rust 1.1.0 status — 2026-10-29

The Rust production cutover and graphite interface are implemented on PR #8. Version 1.1.0 remains unreleased. The user confirms that the installed HDR-to-SDR correction looks much better. Final release acceptance remains with the user. No merge, release tag or publication is authorized.

## Temporary-file reliability and cleanup

Executable SHA-256: `b979e3d5ed3b9ae1b36b81382b8251f64533487f0d6abfae6d501748f7675cf8`. Temporary identifiers used by project saves, recovery, thumbnail/waveform workers and exports now include a process-local atomic sequence. Clock readings can repeat across workers; the sequence makes names distinct within a process even if the clock stops or moves backwards. Exclusive project-file creation also completes before cleanup assumes ownership of that temporary file. Project schemas and media/color processing are unchanged.

The concurrent regression generates 131,072 names across 32 workers: the previous implementation returns only 29,435 distinct names on this host; the corrected implementation returns all 131,072. Project round trips, recovery isolation, undo/settings contracts, formatting, strict Clippy, the workspace suite and release inspector pass. Focused native-media checks cover waveform/thumbnail cache ownership, safe cleanup, mixed SDR/PQ/HLG exports, cancellation and source protection. Evidence: cleanup-audit/ids-before.log, ids-after.log, workspace.log and media.log.

The audit retains the C# parity oracle, project compatibility, source validation, CPU fallback and asynchronous generation checks because their supported consumers remain active. Root launcher instructions point to the actual executable. The preceding HDR correction commit `a71af30` passes Windows CI and the user reports improved appearance; that does not authorize a merge or publication. See the release checklist for remaining physical acceptance.

## Earlier tiny HDR highlight correction

Executable SHA-256: `f4baa1b0ddd75e5b3ac005af865fe231c7829eb680392caa680ffd850eed1d93`. The reported inventory frame at 153.1 seconds exposes an additional scene-exposure problem: the original PQ recording contains a roughly 4,800-nit cursor beside 170-nit menu text. The prior preview closely matches an independent generic renderer (mean screenshot channel difference 1.31/255), but both compress normal white unnecessarily to preserve that tiny peak. Renderer agreement alone did not establish correct scene exposure.

Native/software SDR conversion and exports now use the 99.99th peak percentile, excluding the brightest 0.01% from scene-peak estimation. Tiny isolated cursor/specular detail can clip instead of reducing the whole frame's exposure. Scene smoothing remains disabled for deterministic seeks. Cache generation `v5-highlight-sdr` replaces images made with the earlier peak policy. No exposure or saturation multiplier is added, and HDR-preserving output is unchanged.

An independent linear-light fixture reproduces the problem with 170-nit white plus a tiny 5,000-nit cursor. Previously, adding the cursor changes converted/software/native/thumbnail/export white codes from 237/239/240/238/238 to 145/149/148/146/146. The corrected paths retain exactly their no-cursor white codes. The independent previously approved gameplay scene changes by only 0.23/255 on average. Evidence: recording-153-review/analysis.json, regression-before.log and regression-after.log; rust-playback/tiny-hdr-cursor/white-levels.json.

The reported ARC inventory scene also passes native/software preview, thumbnails and CPU/NVIDIA H.264/AV1 SDR export comparisons; its source hash is unchanged. Mean channel errors are 2.70/1.91 for software/native preview, 1.80–1.90 for exports and 1.72 for the thumbnail. Evidence: recording-153-review/real-recording/adaptive-sdr/comparison.json.

All six full/limited PQ/HLG reference-chart cases pass with the revised policy (recording-153-review/chart-comparisons.json). 268 native/software desktop checks and connected HDR/SDR monitor round trips also pass. Evidence: rust-desktop/4fe07f9e6b8d486fab9f910359716b00 and rust-displays/98eeadc6826146c0b7d2dda6102f086f.

Formatting, strict Clippy, workspace contracts and the release inspector pass. The initial workspace run encountered a missing temporary path in the unchanged encoding persistence test; the complete repeat passes. This patch changes no project persistence code.

## Earlier HDR-to-SDR reference correction

Executable SHA-256: `ae43da1915563b48bb5df0d6e67693a894c30402d024f3e963fcf5a05316240c`. HDR-to-SDR preview, thumbnails and exports use scene-aware libplacebo tone/gamut mapping on compatible graphics, selected by a bounded 10-bit PQ capability probe. Eligible fitted PQ/HLG recordings retain hardware decoding and a persistent D3D11 renderer across seeks. Software preview and complex geometry use the shared processing graph. The standard CPU fallback remains available; ordinary SDR and HDR-to-HDR color paths retain their prior behavior.

Identical decoded HDR planes reached preview/export with different inferred chroma locations (center versus unspecified/left). The new optional source metadata preserves explicit locations; missing values use one left-sited default before scaling. HDR input precision is retained into the mapper. Thumbnail conversion explicitly uses Rec.709 limited YUV to full-range bitmap RGB. Cache generation `v4-adaptive-sdr` includes processor/chroma identity and regenerates older images. Delayed peak use and temporal smoothing are disabled so repeated seeks do not inherit another scene's brightness.

Six full/limited-range PQ/HLG AV1 charts compare native/software preview, repeated seeks, thumbnails and CPU/NVIDIA H.264/AV1 exports with an independent literal renderer. Native window pixels use an sRGB reference, matching production display coding; SDR files/software frames use Rec.709. The user's ARC recording passes: mean per-channel differences are 2.35/1.69 for software/native preview, 1.78–2.02 for exported frames and 1.33 for the thumbnail (8-bit RGB units). Original-file hashes remain unchanged. These are pixel comparisons against the approved reference, not physical display measurements. Evidence: color-and-ui-refinement/adaptive-fixtures-fast.json and adaptive-arc-fast.json. The earlier CPU absolute-luminance regression and broader GPU media matrix retain their separate scope and evidence.

The real-recording production preview benchmark completes 24 exact seeks with median 49.8 ms, p95 73.8 ms, 38 completed updates during continuous pointer scrubbing and a 63.0 ms final-release acknowledgement, using D3D11VA. It measures UI request to playback-ready acknowledgement, not physical display latency; OS/file caching is uncontrolled. Evidence: preview-benchmark/e01deccb88e14f65beb3189d34133a24. Recreating the processing filter per seek failed this continuous-scrub gate and was replaced before installation.

Formatting, strict Clippy, workspace contracts and 268 desktop checks across native/software SDR/PQ/HLG/audio runs pass. Evidence: rust-desktop/e2ac42d9c4944e6bb1520903c1a27bdd. Native/software round trips across the connected HDR/SDR monitors also pass (rust-displays/2202837e20bf4485893b09a70b2f4d49). The monitor verifier now selects its visible editor window: libmpv's hidden temporary window previously made its software-mode movement check invalid.

## Earlier timeline and action-row refinement

Executable SHA-256: `a85d94c9ff85c967ce2ee72ca42d1286331b2e77c496c6c855f5a67555745689`. Video and independent-audio menus expose Split, Trim before and Trim after, preserving the timeline playhead when selecting the target. The inspector keeps exact trims. Menu actions outside the target are disabled; unavailable audio separation is omitted. Timeline and export-status captions share a text baseline, while framed buttons retain optical centering. The Windows title omits the unsaved dot; the project label retains it.

Formatting, strict Clippy, workspace contracts, 73 native/software audio interaction checks with 40 scaled layouts, and 16 native/software VFR checks pass. Unit checks inject menu input for both item types and paint row baselines at 100–200% scale. Evidence: rust-desktop/9649689c650c4718877b42b47cef5fb2 and rust-desktop/3e99fde2d3c14251b55bde7847a67fb7. This candidate changes no media conversion and does not replace the running App.

## Audio-item click seeking

Executable SHA-256: `a3993cb54f88c16f3b48e72038437db15fbb1583ce0cdcbbe8bc250bae10994b`. Clicking an independent audio item seeks to the pressed point rather than its start. Mapping accounts for timeline placement and zoom without adding the source trim offset. Item bounds, keyboard fallback, drag behavior and project contents are preserved.

46 core tests, strict formatting/Clippy and 73 desktop checks across native/software audio runs pass. Injected quarter/three-quarter clicks on a trimmed, offset item reach their expected playhead positions without changing the saved project. The bundle also contains the verified HDR exposure correction below; its media processing is unchanged by this click fix. Evidence: audio-click/verification.json and rust-desktop/010ae8ca3e1e4d739cb23176a630b3ac.

## Earlier HDR-to-SDR exposure verification

Executable SHA-256: `0a0469fd03ca09d3ad6f3f7fb66e6986b3761d65532b31ab630b9e7b63cdc89d`. The shared SDR conversion uses 203 cd/m² HDR reference white and signal ceilings expressed in those same units. This corrects raised midtones and highlight clipping in HDR preview, thumbnails and SDR exports. HDR-to-HDR exposure and user project/output settings remain unchanged. Cache generation v3 regenerates images from the earlier conversion.

An independent 0–10,000 cd/m² PQ ramp demonstrates the regression: the old conversion rendered the 18 cd/m² patch at code 124 and flattened the 1,000/2,000/4,000 cd/m² patches to code 255. The corrected conversion renders that midtone at 93 and those highlights at 245/250/253. Full/limited-range AV1 fixtures pass on CPU and NVIDIA, with neutral blacks, reference-white checks and absolute luminance preserved in HDR exports. Comparisons on the user's ARC recording reproduce the previous brightness difference against native playback; private frames and paths remain in ignored artifacts.

46 core tests, strict formatting/Clippy, nine GPU media integration tests, the two CPU HDR regressions, 266 desktop checks across eight native/software SDR/PQ/HLG/audio runs, ordinary filename startup and relocated portable verification pass. Evidence: preview-exposure/verification.json, rust-desktop/6dd9732ca74d42d3a347271089ca9549 and rust-package-verification/a4d1ba8cad00453f9da459baee3f31cf. Earlier layout/device/resource measurements below retain their hashes and scope.

## Earlier independent-audio verification

Executable SHA-256: `c67bc1565d83bdc8d6ad446d967ebb9ab1ffa4257c4b82c417bed43598d09b4c`. Separate audio preserves exact source trims and placement, disables embedded sound for that video section and creates an independently editable audio item. Per-item volume, track, mute and fades follow audio edits, playback, save/reopen and export. Video and audio edits share atomic undo/redo. The single independent lane preserves gaps and rejects overlapping audio items; it can play alongside sections that still have linked audio. Audio-only tails export with black video. Schema 3 protects these edits from being silently discarded by older Rust candidates; schema 1/2 import remains supported.

46 core tests, strict formatting/Clippy, 549 desktop checks across 18 native/software runs, 71 checks across 40 scaled layouts, relocated portable verification and nine packaging safeguards pass. Generated-media tests verify actual silence, gain ratios, mixing without normalization, fades, duration, source hashes and project round trips alongside the existing NVIDIA/HDR export matrix. Actual desktop input checks separate-item selection, both volume directions, movement, trim-edge cursor/geometry, split/delete and undo, plus Windows audio output and video-clock alignment. Evidence: audio-items/verification.json, rust-desktop/08eab325121d44c5b219a3e2d1e559b9 and rust-desktop/ba0e21b1936f438498a9e2385cdc912a. Earlier device/resource measurements below retain their original artifact hashes and scope.

## Earlier timeline drop-preview verification

Executable SHA-256: `791314afa871a972d1fa10b722b93cc8dd07423efcfc6b0e14cba3d7d76875eb`. Library drags show a thumbnail/filmstrip ghost at the actual insertion position while over the timeline; the floating card remains outside it. Placement uses the same source frame rate, snapping and free-space calculation as the committed drop. No project edit occurs during hover. The ghost follows movement and disappears on release; the inserted linked section matches its shown position and remains one undoable edit.

43 core tests, including 162 placement-parity cases, strict formatting/Clippy, 541 desktop checks across 18 native/software runs, 65 checks across 40 scaled layouts and relocated portable verification pass. Evidence: drop-preview/verification.json, rust-desktop/f356520be67342a2bdec684ceedfde8c and rust-desktop/8bea067243224c188caf2f7b91ffb992. Existing media/device/resource results below keep their original hashes and scope.

## Earlier clip-cursor verification

Executable SHA-256: `42c706562cc1df9450ead95f9bbfcbaf637368639bb43550acd244824b6ea81f`. Both ends of linked video/audio sections use the horizontal resize cursor. Hover and drag start share their existing bounded hit region; a captured trim retains the cursor beyond that edge. Clip bodies use grab/grabbing for moves. Core checks and actual native/software pointer verification pass; source timestamps, trim behavior and media processing are unchanged. Current evidence is recorded in ignored clip-cursor/verification.json. Earlier button/media/resource evidence below keeps its own artifact hashes.

## Earlier button-alignment verification

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
- `project-store` owns bounded legacy import, schema-2/3 persistence, project validation and recovery metadata. Schema-1 originals and the published C# app remain available for rollback.
- `media-engine` owns the shared color/framing graph, probing, thumbnails/waveforms, verified atomic exports and cancellation. Export always reads original recordings.
- `media-path` and `platform-windows` enforce local file/source identity boundaries and Windows display/window integration.
- `native-mpv` owns the verified DLL, thread-confined control handle and renderer teardown. The desktop actor owns asynchronous generations, latest-request/frame delivery and audio-output recovery.
- The UI preserves keyboard focus, named transport controls, exact trim fields, drop placement and progressive disclosure. Ordinary startup and idle work use production event wakeups.

## Remaining physical acceptance

Current-display HDR appearance is user-confirmed. Bounded game/Shadowplay checks pass; the user independently confirmed the separate Windows/NVIDIA game-switch capture corruption is external to this project. That investigation remains closed.

Manual Windows HDR toggles, genuine mixed-DPI monitor transitions and a complete screen-reader workflow still require the corresponding setup and user observation before final release approval. Current tests cover actual HDR/SDR monitor movement at 96 DPI, app scaling and accessibility-tree semantics. They do not measure physical HDR luminance, headphone loudness or every device hotplug combination.

An earlier Glow startup access violation remains unattributed. Production uses DX12; current startup stress passes. Prior investigation reports remain available rather than being described as a proven fix.

## Evidence and history

Current ignored artifacts: release-1.1/verification.json; rust-desktop/8c631cba89544efb9cfc785fb644d000, 1ab5904c15cb44d1a6a1556c82fbb0f8, 1ca59d874db84310955cd26ddf949d24 and 1b83dccc8b6f4651977b0219593b9e4d; rust-displays/987f0a6c2a2f4bd1b1f193e90cfd52fd; startup-stress/a87096facb1a45dd94496473720e4719; version-comparison/b7c46c74995e41e5909d73231255fe97; rust-package-verification/f9324bb3860c4d0a9dd7d7c47552edaa.

The [previous ledger](https://github.com/Gimbuhh/Simple-Video-Editor/blob/3817dd3002c495f64f92f18c7d0b6ef94d64ce4c/docs/rust-rewrite-status.md) preserves the detailed rewrite, HDR/capture, seek and startup investigation history. Earlier measurements keep their original scope and artifact hashes; they are not silently reassigned to this executable. Native pins/licenses remain unchanged. Published assets and original recordings are preserved.
