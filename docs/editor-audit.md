# Editor improvement audit — 2026-10-28

Scope: preview playback and audio routing, button layouts, source-derived previews, worker ownership, and the build/verification paths they use. This is a focused improvement audit, not a security review or a complete physical-device certification.

| Finding | Result |
| --- | --- |
| Embedded playback forces `ao=auto`, which the pinned libmpv rejects as an unknown driver. | Preview leaves driver selection at its platform default; Windows opens WASAPI. Offline lab playback keeps its intentional null output. |
| Wide buttons inherit left alignment in vertical panels. | Action text grows within the button, centering its label without moving the control. Menu entries keep their usual alignment. A rendered-shape regression fails before the change and passes afterward, including disabled buttons. |
| Recording-color undo/redo keeps stale library thumbnails. | Metadata edits and history share the same invalidation path. Desktop checks wait for regenerated previews and verify exact project restoration. |
| Waveform/thumbnail startup spawns two workers with no pending sources. | An empty work list returns immediately. No latency improvement is claimed. |
| Thumbnail and waveform results duplicate their source/interpretation validity check. | Both use one check, retaining rejection of results for changed or removed sources. |
| Existing export and lab tests cannot detect silent desktop playback. | A generated two-track desktop fixture verifies a real negotiated audio output and the probed track in native and software modes. |

Retained deliberately: source authorization, stale-result checks, asynchronous generation handling, exact trim/seek boundaries, native-to-software fallback, and the isolated C# comparison oracle. They have active consumers or explicit compatibility/verification roles; no dependency removal is justified by this audit.

## Release-readiness follow-up

- Playback detects an unavailable output while sound is expected, shows an actionable status message, and retries only the audio output on a bounded schedule. A selected stream that disappears is reported as unavailable. Muted, zero-volume, paused and video-only playback do not trigger recovery warnings.
- Native/software fault tests recover the default endpoint without resetting the video clock or editing the project. Physical audio-device hotplug combinations remain a separate scope.
- Starting or opening a project releases its previous player, filmstrip worker/textures and preview images. Empty projects do not create a filmstrip worker. Desktop verification checks resource ownership after reset.
- The position slider has an accessible name/value. Trim fields link to their visible In/Out labels while retaining their spinbox/text-edit roles and exact values; the accessibility-tree test verifies those contracts.
- Live native/software moves between the connected HDR and SDR monitors check fresh display state, PQ/SDR selection and physical child bounds. Windows settings are preserved. Both monitors use 96 DPI, so actual mixed-DPI transitions remain unverified.
- Current visible resource measurements replace the earlier stale candidate assumptions. Native preview has lower process working set in the generated 4K case; empty/software cases still use more memory than C#. GPU memory is excluded. See the [comparison](rust-csharp-comparison.md).

Verification: 42 core tests, strict formatting/Clippy, 525 desktop checks, separate audio-recovery/fallback runs, 40 scaled layouts, GPU/media integrations, 20 startup cycles, normal filename startup, physical monitor round trips, relocated portable verification and nine packaging safeguards pass. Exact reports and executable hashes are in the [status ledger](rust-rewrite-status.md). Tests use generated fixtures; original recordings are not modified.

The [release checklist](release-readiness.md) retains manual HDR-toggle, genuine mixed-DPI and full screen-reader acceptance. Physical speaker/headphone loudness is not measured by negotiated-output checks. Existing HDR and game-capture conclusions remain unchanged.

## Button glyph alignment follow-up

The earlier alignment check centered a font layout box. Actual Import screenshots still put the visible text one pixel right and two pixels down at 100% scale. The shared action-button renderer now centers painted glyph bounds and snaps to physical pixels, while retaining stock button interaction and accessibility. Raster measurements of the corrected Import button show zero offset on both axes and equal margins. The installed-font/fallback-font regression covers 2,400 combinations; the full native/software desktop matrix, 40 scaled layouts and portable checks pass. This changes action and menu-trigger captions; menu entries retain their established alignment and editable fields retain their normal text layout.


## Cleanup follow-up — 2026-10-29

The post-HDR pass traces temporary-file ownership through project saves, recovery, thumbnail/waveform workers and export jobs. The clock-only identifier repeats under concurrent calls: 131,072 calls across 32 workers produced only 29,435 unique names on the test host. A process-local atomic sequence now prevents repeated clock readings from reusing names; process/time components remain for separate sessions. Project saves enter cleanup only after exclusive temporary-file creation succeeds. Recovery compatibility, atomic replacement and original-file protection remain intact.

The root launcher instructions now point to the actual App executable and stop promising a missing shortcut or new source backups. The release checklist records the user's improved HDR result and the passing HDR-commit CI without treating that as merge/publication approval.

Retained: the frozen C# parity oracle, schema-1/2 compatibility, CPU color fallback, source validation, asynchronous generations and isolated native verification tools. Their callers and supported contracts remain active. No dependency removal or large UI restructuring is justified by this pass. Genuine mixed-DPI transitions, manual HDR toggles and a full screen-reader workflow remain physical acceptance items; broader UI polish is a separate refinement task.

Verification: the new concurrency regression fails before the fix and passes after it. The full workspace formatting/lint/contracts and release inspector pass, along with focused cache/export/cancellation/source-protection integrations. Media/color and UI behavior are unchanged; prior HDR and interface evidence retains its own executable hashes.
