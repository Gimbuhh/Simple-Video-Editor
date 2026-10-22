# Simple Video Editor 1.1 — Rust Rewrite Plan

Updated: 2026-10-21 (original proposal: 2026-10-09)

Target application version: **1.1.0**

Status: Rust production cutover is implemented on PR #8. The UI is egui/eframe on DX12 with native D3D11 playback and an SDR software fallback. Current-display HDR preview and local capture checks pass; the user confirmed the separate HDR-to-SDR game-switch recording issue is external to the project. Broader display/DPI/accessibility acceptance and the historical startup-crash investigation remain tracked; merge/publication require separate approval. Current ownership and measured evidence are in the [status ledger](rust-rewrite-status.md).

## 1. Goal and release definition

Replace the C#/WPF application with a Rust desktop application, preserve the existing editing workflow and saved projects, and add a usable HDR workflow plus a small set of practical editing improvements.

Ship Windows x64 first. Keep the architecture portable, but do not make macOS/Linux releases a requirement for 1.1.0. Keep recordings local, edits non-destructive, and the portable ZIP self-contained. No account or online service is required.

A successful rewrite means Rust owns editor state, persistence, playback coordination, export orchestration, and UI interaction. The released app has no C#/.NET runtime dependency. FFmpeg, FFprobe, and a native playback engine can remain bundled dependencies; rewriting codecs is outside scope.

Rust alone is not a guarantee of faster encoding, smoother scrubbing, or a safer native decoder. Measure those outcomes and retain the existing input and output protections.

## 2. Reference baseline and evidence

Implementation reference: published 1.0.4 at commit `dd65df13a41490e8168a624b65fcff6a27956940`. The starting checkout is `fb1fbf7641146d7a1d9211c56ad77bb44f1ec46b`, with unchanged app source and existing tests relative to that release. Exact artifact identity and reproduction evidence are in the [status ledger](rust-rewrite-status.md).

The inspected app uses WPF, libmpv software preview, FFmpeg/FFprobe processes, JSON projects, and linked video/audio clips. The source contains approximately 3,000 lines of C#/XAML. HDR is currently detected and rejected. Preview uses an 8-bit BGR buffer, while export normalizes video to SDR BT.709 and 8-bit YUV420.

The reproduced baseline passes 103 core/media checks and 233 WPF UI checks. These are starting evidence, not proof of Rust parity. Current shipping-version documentation identifies 1.0.4.

Before implementation:

- [x] Select the latest approved, merged 1.0.x commit and identify its exact release artifact and SHA-256.
- [x] Record toolchain versions, native dependency pins, source inventory, build commands, fixtures, and known limitations.
- [x] Reproduce the baseline build and relevant checks. Compare repeated builds only if reproducible binary output is claimed.
- [x] Record each retained behavior in a contract ledger, with its current evidence, new owner, and test.
- [x] Keep original recordings untouched. Automated verification uses generated fixtures; no private recordings were added to shared fixtures.

If the baseline cannot be reproduced, resolve that gate before dependent migration work. Planning and investigation can continue.

## 3. Proposed feature scope

The extra features below are recommendations. Confirm the required-versus-stretch split at the first milestone.

| Priority | Feature | 1.1 acceptance requirement |
| --- | --- | --- |
| Required | Rust application and 1.x feature parity | All retained editing contracts pass in the packaged Rust application. |
| Required | HDR10/PQ and HLG import | Detect color properties and bit depth; display understandable HDR labels; handle missing/conflicting metadata explicitly. |
| Required | HDR preview and SDR fallback | Actual HDR presentation on supported Windows HDR displays; deliberate tone-mapped preview on SDR displays or incompatible hardware. |
| Required | HDR export | Verified 10-bit HDR10 output through supported AV1 and HEVC paths, with correct signaling; SDR export remains available. |
| Required | Mixed HDR/SDR projects | A project output profile defines conversions consistently for preview and export. |
| Recommended for 1.1 | Audio stream selection and simple audio controls | Choose game/microphone streams where present; per-clip gain, mute, and fades are saved, undoable, previewed, and exported consistently. |
| Recommended for 1.1 | Explicit project/export profiles | Resolution, rational frame rate, color profile, and quality settings are visible and saved; presets include source size, 1080p, and vertical output. |
| Recommended for 1.1 | Non-destructive crop, framing, and rotation | Fit/fill/custom crop with predictable preview/export results, including rotated and non-square-pixel media. |
| Stretch | Proxy editing | Low-resolution editing copies improve difficult sources without changing original-source export or timestamp mapping. |
| Stretch | Timeline markers | Named markers support navigation and are stored in the project. |
| Stretch | Export queue | Jobs freeze a project snapshot and output settings; canceling one job leaves originals and unrelated outputs intact. |
| Later | Independent music tracks, titles, transitions, effects, full multitrack compositing | Separate design and verification work after 1.1, unless a feature is explicitly promoted into scope. |
| Later | macOS/Linux, Dolby Vision, HDR10+ metadata preservation, automatic updates | Separate platform/format/security work. |

Do not expand 1.1 into a general-purpose professional NLE. If scope grows, move stretch features into 1.2 instead of weakening HDR, parity, or release verification.

## 4. Technology choice: decide through a prototype

### Recommended starting direction

Use a Cargo workspace with a UI-independent Rust editor core, a native desktop UI, and existing proven media engines.

Prototype **Slint** first as a UI candidate, but do not select it solely from screenshots or a basic playback demo. Its renderer choices include software rendering; the documented software renderer has rotation/scaling limitations that matter for this editor. Validate the chosen renderer and video surface together. [Slint renderer documentation](https://docs.slint.dev/latest/docs/slint/guide/backends-and-renderers/backends_and_renderers/)

Keep an alternative Rust UI candidate, such as egui/eframe or iced, for the prototype comparison. Evaluate their current rendering, accessibility, licensing, and packaging capabilities before adopting one. Tauri is another option if an HTML/CSS interface is acceptable, but it adds a webview layer and would require a different video-integration design.

### Playback and HDR prototype

Retain libmpv as the first playback candidate behind a Rust adapter. It exposes a C embedding API. Native calls need a narrow, reviewed unsafe boundary with explicit ownership and thread rules. [mpv embedding documentation](https://mpv.io/manual/master/#embedding-into-other-programs-libmpv), [Rust FFI guidance](https://doc.rust-lang.org/stable/nomicon/ffi.html)

Prototype two presentation options:

1. A native Windows video surface controlled by the playback engine, with HDR-capable output.
2. An application-owned video surface, with explicitly controlled color conversion and display presentation.

Verify the selected libmpv embedding mode supports the intended renderer and HDR path. Options available to a standalone mpv video output must not be assumed to work through every embedding/render API. Pin a tested version: current online documentation may describe features newer than the repository's existing native bundle. [mpv output color options](https://mpv.io/manual/master/#options-target-colorspace-hint)

Windows HDR presentation requires the display state, surface format, and color-space signaling to agree. A DirectX/scRGB route is one candidate, not a promise that the UI toolkit already implements it. Test moving the window between displays and changing HDR mode. [Microsoft Advanced Color guidance](https://learn.microsoft.com/en-us/windows/win32/direct3darticles/high-dynamic-range)

### Prototype exit gate

- [ ] Smooth pointer-following timeline and recording scrubbing at normal and small window sizes.
- [ ] Correct SDR, PQ, and HLG preview, including portrait/rotated/non-square-pixel media.
- [ ] Confirmed true HDR output on a physical HDR display; an ordinary screenshot is insufficient.
- [ ] Deliberate SDR fallback with a visible preview-mode label.
- [ ] Preview/export agreement for a small HDR fixture set.
- [ ] Correct keyboard focus, drag capture, accessibility, DPI scaling, and portable startup.
- [ ] No unacceptable interference with a running game's NVIDIA Shadowplay capture.
- [ ] Documented framework, renderer, dependency, licensing, and packaging decision.

Do not begin the full UI port until this gate passes. If true HDR presentation fails, change the architecture or explicitly narrow the release promise to HDR import/export with SDR preview; do not advertise that narrower result as full HDR preview support.

## 5. Proposed architecture and ownership

Use these as logical boundaries. They can begin as modules and become separate crates when independent builds or APIs justify it.

| Component | Authoritative responsibility |
| --- | --- |
| `editor-core` | Timeline model, clip identities, linked relationships, edit commands, snapping, time mapping, and undo/redo. No UI or native-player dependency. |
| `project-store` | Schema parsing/validation, migration, atomic saves, recovery sessions, and persistence of project settings. |
| `media-probe` | Validated local recording access, stream enumeration, frame/timestamp metadata, and capability inspection. |
| `color-pipeline` | Project color profiles, source interpretation, conversion plans, and shared preview/export color settings. |
| `playback` | Clock state, seek generations, source switching, pause/resume intent, and decoder adapter lifecycle. |
| `render` | Video presentation, display capabilities, aspect/crop/rotation, and explicit SDR/HDR output modes. |
| `export` | Immutable job snapshots, FFmpeg plans, progress, cancellation, validation, and final output replacement. |
| `cache` | Thumbnails, waveforms, optional proxies, cache keys, invalidation, size accounting, and safe cleanup. |
| `desktop-ui` | Views, keyboard/pointer input translation, accessibility, dialogs, and commands sent to the core. |
| `platform-windows` | Display/HDR notifications, approved filesystem rules, native handles, and package integration. |

The UI submits commands and renders state. It must not implement a second copy of timeline rules, validation, or color policy. Preview and export consume the same edit and color descriptions.

Use rational frame-rate/time representations where appropriate and checked arithmetic for durations and bounds. Preserve the meaning and precision of existing imported trim points; do not snap or round legacy edits merely because the new model uses a different representation. Handle variable-frame-rate media through source timestamps, rather than pretending every source has a constant frame duration.

Native decoding/rendering and FFmpeg work stay off the UI thread. Use bounded queues and generation IDs so obsolete loads, seeks, and frames cannot update the active project.

## 6. HDR design

### Input analysis and project policy

Replace the single `IsHdr` flag with explicit source color information: transfer function, primaries, matrix, range, pixel format/bit depth, and available mastering/content-light metadata.

Detect PQ and HLG separately. Handle unknown metadata with an explainable default or explicit user override; never silently guess that every 10-bit file is HDR.

Persist the chosen project output profile and any source overrides. Recommended profiles:

- **SDR Rec.709:** existing compatibility profile; HDR sources are deliberately tone-mapped.
- **HDR10/PQ:** BT.2020/PQ 10-bit output; SDR and HLG sources receive defined conversions.

HLG input is required; HLG output is a later option. Dynamic HDR metadata preservation is outside initial scope.

### Preview and thumbnails

Choose preview output from actual display capabilities and the project profile. Show labels such as **HDR preview** or **SDR tone-mapped preview**. Keep SDR controls and UI text at an appropriate reference brightness.

Use a sufficient-precision HDR pipeline; the existing 8-bit BGR display path cannot serve as the HDR-preserving path. Produce intentionally tone-mapped SDR library thumbnails and waveforms without changing source files.

Preview transforms must account for transfer decoding, gamut conversion, brightness/reference-white policy, tone mapping where needed, and output encoding. Avoid applying tone mapping twice.

### Export

Keep SDR H.264 and AV1 exports. Add HDR10 AV1 and HEVC export where the selected encoder supports the required 10-bit mode. Detect actual encoder/filter/pixel-format capabilities from the bundled tools; an NVIDIA model name alone is insufficient. Provide a tested CPU fallback where available.

Create actual color transforms, not just new metadata labels. FFmpeg offers color-conversion and tone-mapping filters, but required filters must exist in the pinned build. Its CPU tone-mapping filter expects floating-point input. Prototype filter graphs before freezing them. [FFmpeg tone mapping](https://ffmpeg.org/ffmpeg-filters.html#tonemap), [FFmpeg zscale](https://ffmpeg.org/ffmpeg-filters.html#zscale)

Validate pixel format/bit depth, transfer, primaries, matrix/range, duration, decoding, and container/bitstream consistency. Verify metadata after the final mux. Do not invent mastering or light-level values, or copy them blindly after mixing sources or applying transforms. Define when metadata is measured, preserved, regenerated, or omitted.

### Mixed sources

A project profile determines the working/output color policy. Handle HDR-to-SDR tone mapping and SDR-to-HDR reference-white mapping explicitly. Use consistent conversion settings in preview and export. Black gaps must represent black in the selected output profile.

### HDR acceptance fixtures

Use licensed or locally generated fixtures covering PQ, HLG, SDR, missing metadata, limited/full range, gradients, bright highlights, saturated colors, mixed timelines, and 10-bit AV1/HEVC. Check metadata and decoded samples automatically, plus visual output on actual HDR and SDR displays. Record the display, Windows HDR state, GPU/driver, player, and settings used.

## 7. Preserve the existing editing contracts

| Contract | Required evidence |
| --- | --- |
| Original recordings are never edited or overwritten | Before/after hashes, source-as-output rejection, and cancellation/failure tests. |
| Linked video/audio stay aligned through edits | Trim/split/move/delete/undo scenarios and decoded export checks. |
| Out points exclude the next frame | Non-keyframe cuts, single-frame sections, and exact output-frame assertions. |
| Gaps, leading gaps, and repeated source sections survive | Project round trips, silent/black exports, and playback across boundaries. |
| Library and timeline have separate identities | Repeated insertion, library removal, undo/redo, and unused-source persistence. |
| Add-all includes search-hidden recordings as one edit | Library ordering, existing gaps, and one-step undo tests. |
| Dragging a clip or trim edge preserves the intended preview | Real pointer/capture tests and nearest-retained-frame behavior. |
| Scrubbing follows the pointer smoothly | Interleaved input, timer, load, and stale-frame events; forward/backward and cross-source/gap cases. |
| Play/pause intent survives seeks and source changes | Paused/playing cases, final-seek completion, endpoint behavior, and capture loss. |
| Projects, recovery, relinking, and cache cleanup remain reliable | Legacy fixtures, failure recovery, concurrent-session tests, and unrelated-file preservation. |
| Portable packaging contains one current app | Embedded version, ZIP structure, checksum, relocation, and stale-package regression checks. |

Keep intentionally changed behavior in a separate ledger entry. Do not carry forward a bug solely because a baseline test happened to encode it.

## 8. Interaction and performance design

Use explicit playback states: paused, playing, scrubbing, loading, waiting for the final seek, failed, and closing.

During a scrub, the pointer owns the displayed playhead. Coalesce preview seeks; show the latest available preview without letting stale decoder positions move the control. On release, submit the final exact seek and restore the intended play/pause state after completion. Use request generations rather than assuming any decoder event acknowledges the latest request.

Retain/cached-draw timeline geometry and virtualize long clip libraries. Generate waveforms, thumbnails, and proxies in cancellable bounded background jobs. Keep timeline navigation responsive while media work is delayed.

Proposed measurement targets, to ratify after the prototype:

- Pointer-to-playhead latency: p95 at or below 16.7 ms on the reference desktop.
- 60 Hz UI updates during sustained drags; no backward playhead movement caused by obsolete events.
- No synchronous probe, decode, export, or disk scan on the UI thread.
- Benchmark cached and uncached seek-to-preview latency separately, with explicit codec, resolution, GOP, hardware, and storage details.
- Bounded queue lengths and memory usage under rapid seeks, long recordings, and large libraries.

Do not promise a universal seek time before benchmarking long-GOP 4K/HDR recordings. Add proxies if measured performance requires them. Proxy timestamps and color interpretation must map back to originals; exports use originals by default.

## 9. Project format and migration

Read existing schema-version-1 `.sveproject` files, including older files without an explicit recording library or placement mode. Preserve source paths, order, trim values, timeline gaps, and unused recordings.

Introduce schema version 2 only when needed for the new settings. Store stable source/clip IDs, selected streams, audio adjustments, framing, project color/output settings, and markers if implemented. Include application version separately from schema version.

Open old projects without rewriting them. Save upgraded projects to a new filename by default and explain that 1.0.x cannot read new 1.1 features. Do not silently overwrite the only compatible original.

Migration requirements:

- [x] Fixture-based import and semantic round-trip comparison against 1.x.
- [x] Finite/range/size validation before filesystem or media access.
- [x] Clear rejection of unsupported future schemas.
- [x] Atomic saves and recoverable failure during replacement.
- [x] Independent recovery-session ownership, with safe upgrade behavior.
- [x] Cache namespace/version migration that never treats original media as disposable.

A return to 1.0.x uses the preserved original project. Do not promise a downgrade export until a tested conversion can explain which 1.1 features would be lost.

## 10. Additional feature details

### Audio controls

Start with stream selection, gain/mute, and short fades on the existing linked audio lane. Define whether multiple selected streams are mixed and how clipping is prevented. Persist every setting and make it undoable. Keep the existing first-stream behavior for legacy projects.

A separate music track, arbitrary automation, and a full mixer remain later work. Check preview/export synchronization and audio levels using known fixtures.

### Output profiles and framing

Show resolution, frame rate, codec, color profile, and quality before export. Provide source, landscape, and vertical presets. A vertical preset requires a clear fit-versus-crop choice; it must not silently remove content.

Store crop and rotation as non-destructive transforms. Validate rotated dimensions, sample aspect ratio, and preview/export parity.

### Optional workflow improvements

Markers are a modest navigation addition. An export queue requires immutable job snapshots and independent cancellation. Proxies require storage estimates, invalidation, regeneration, and original-source export checks. Implement each only after parity and HDR gates are stable.

## 11. Security and dependency requirements

Carry forward validation before access to project-provided recording paths, including local-link resolution and rejection of unsupported remote/device locations. Preserve the distinction between untrusted recording references and a destination deliberately selected for export.

Invoke media tools with argument arrays and approved executable paths; never construct shell commands from filenames. Bound project sizes, durations, logs, worker queues, allocations, and cache cleanup scope.

Keep unsafe Rust isolated around native handles and buffers. Review buffer sizes, callback lifetimes, thread affinity, shutdown order, and errors. Native libraries retain their own vulnerabilities; Rust does not remove that risk. Consider playback-process isolation if the prototype shows an acceptable complexity/performance tradeoff.

Pin the Rust toolchain, Cargo lockfile, UI/runtime crates, media binaries, and checksums. Check actual dependency licenses and include matching notices. Use dependency/advisory checks and a software bill of materials; automated scanning supplements code and boundary review.

Keep the app offline by default. Telemetry, automatic updates, and remote media inputs require separate user-approved designs.

## 12. Implementation milestones

| Milestone | Work | Exit gate |
| --- | --- | --- |
| M0: baseline and scope | Freeze approved 1.x artifact, contract ledger, fixture inventory, required/stretch list. | Reproducible baseline and agreed 1.1 scope. |
| M1: rendering/HDR spike | Compare UI/video integration, actual HDR, SDR fallback, scrubbing, capture compatibility, and one HDR export. | Technology decision backed by working packaged prototypes. |
| M2: Rust domain and persistence | Editor commands, time model, undo/redo, validation, schema-1 import, schema-2 saves/recovery. | Pure/differential tests and lossless legacy-project semantics. |
| M3: working vertical slice | Import -> library -> timeline -> playback -> trim/split -> SDR export. | Real Rust desktop workflow runs end to end without C# as runtime owner. |
| M4: parity | Complete linked edits, gaps, relinking, recovery, search, batch insertion, keyboard/accessibility, cache and cancellation. | Existing required contracts verified in the Rust artifact. |
| M5: HDR | Color model, mixed timelines, SDR tone mapping, native HDR preview, 10-bit output and metadata verification. | HDR acceptance matrix passes on declared hardware/display combinations. |
| M6: selected additions | Audio controls and profiles/framing; promote stretch features only if capacity permits. | Each feature is saved, undoable where applicable, previewed and exported correctly. |
| M7: beta and release candidate | Performance, failure injection, security review, packaging, documentation and user testing. | Release checklist passes; remaining limitations are explicit. |
| M8: cutover | Remove superseded C# production paths and temporary adapters; finalize 1.1 metadata and packages. | One authoritative Rust production implementation; user approval before merge/publication. |

Track each replacement through: **implemented -> production reachable -> verified -> legacy owner removed**.

Keep the old application as a frozen test oracle or independent release during migration. Do not make it a runtime dependency of the final Rust app. Remove superseded production implementations after verified cutover; Git history retains source.

Provide schedule and effort estimates after M1. HDR presentation and UI integration are the largest unknowns; a precise delivery date before that spike would be speculative.

## 13. Verification plan

Run verification in layers:

1. Pure Rust tests: edit commands, checked time arithmetic, snapping, stream choices, color-plan selection, schema parsing and migration.
2. Differential tests: frozen 1.x fixtures and expected editing outcomes, with approved feature differences recorded separately.
3. Native integration: load/seek generations, repeated shutdown, stale events, source switching, encoder/filter capability detection.
4. Media output: full decode, frame counts, trim boundaries, silent gaps, audio alignment/levels, 10-bit/color metadata, cancellation and replacement.
5. Real desktop interaction: pointer capture, rapid scrubbing, focus, keyboard shortcuts, dialogs, accessibility, 100-200% DPI and small/wide windows.
6. Hardware matrix: CPU/SDR fallback; available NVIDIA/AMD/Intel paths; HDR on/off and mixed-display movement. Record unavailable combinations honestly.
7. Actual portable artifact: clean machine/VM where applicable, paths with spaces/Unicode, bundled dependency resolution, source protection, single current package pair.
8. Recovery and rollback: failed saves/exports, disk full, locked files, interrupted sessions, old-project preservation, and return to the exact previous release.

Use generated/licensed fixtures for automated CI. Keep private recordings, screenshots, and test outputs ignored. Screenshot comparison alone cannot validate HDR luminance or display signaling.

## 14. Git, versioning, packaging, and release

Follow the existing repository policy throughout implementation:

- Feature branches use the `codex/` prefix; changes reach main through reviewed PRs.
- Keep PRs open for user testing. Merge only after testing confirmation and authorization, with checks passing.
- Never push directly to main, force-push published history, rewrite published tags, or silently replace assets.
- At verified cutover, Rust's application package version becomes authoritative. Derive all artifact names and metadata from that one value; eliminate dual C#/Cargo version authority.
- Use development prerelease versions while needed, then **1.1.0** and tag **v1.1.0** for the final release. Extend package validation explicitly if prerelease names are introduced.
- Keep the portable app layout understandable: one app executable, instructions, bundled binaries, and documentation/licenses.
- Final ZIP: `SimpleVideoEditor-1.1.0-win-x64.zip` plus its SHA-256 checksum.
- Check the embedded executable version, structure, and checksum before replacing/pruning older generated local package pairs.
- Port the existing packaging safeguard tests to the Rust build pipeline.
- Preserve published 1.x releases and the dependency mirror named by `MediaMirrorTag`. Change native pins and mirror policy together only after verifying a recoverable matching bundle.
- Build and test alternate app output folders when a user editor is running. Never stop or replace the running app.
- Tag the merged main commit only after checks pass. Create a draft release, verify downloaded assets, and publish only with user authorization.
- Clean reproducible build tools/output only after the final app bundle is verified. Preserve source, release artifacts, original media, projects, and rollback evidence.

## 15. Definition of done

- [x] Rust owns all planned production responsibilities; no C#/.NET dependency or expired bridge remains in the candidate package.
- [x] Retained 1.x editing behavior and project compatibility have evidence.
- [ ] HDR import, true HDR preview where supported, labeled SDR fallback, mixed projects, and HDR/SDR exports meet the agreed scope.
- [x] Selected additional features behave consistently across persistence, undo, preview, and export.
- [ ] Smooth scrubbing, async cancellation, source protection, recovery, and safe native shutdown pass.
- [ ] Performance measurements and the supported hardware/format matrix are documented.
- [ ] Dependency pins, licenses, security review, and package checks are complete.
- [x] Version, changelog, release notes, user guide, shortcuts, and portable artifacts agree.
- [ ] The user has tested the release candidate and approved merge.
- [ ] The merged commit is tagged and the verified draft is published only when authorized.
- [x] Rollback remains possible with preserved 1.x releases and original project files.

## 16. Decisions to settle at kickoff

1. Confirm the required 1.1 feature list: audio controls and framing/profiles are recommended; markers, proxies, and queues are stretch work.
2. Identify the physical HDR display(s) and representative PQ/HLG recordings available for testing.
3. Ratify the UI/rendering choice after the HDR prototype, including the tradeoff between true HDR/GPU presentation and the existing software-only capture behavior.
4. Confirm Windows versions and encoder paths that must be supported.
5. Decide whether the first 1.1 release should include any independent audio track or keep that for 1.2.

Implementation started on 2026-10-20. See [rewrite status](rust-rewrite-status.md) for evidence and outstanding gates.


## 17. Confirmed follow-up: transitions

Version 1.1.0 keeps hard cuts as the default and adds no transition effects. After the rewrite and HDR verification, consider a per-boundary cross dissolve with adjustable duration and an independently enabled audio crossfade. Use available source handles without moving clips, shortening the timeline, changing saved trims, or closing gaps. Limit duration when handles are insufficient. Preview and export must blend in a shared working color space, and transitions must persist and support undo/redo. Reserve a boundary-effect design seam; do not ship placeholder transition controls or commit to a schema before implementation.
