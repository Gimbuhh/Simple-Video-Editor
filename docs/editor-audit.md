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

Follow-up: broader audio-device switching/disconnection, per-monitor DPI transitions, screen-reader navigation, and representative memory profiling remain separate acceptance work. Negotiated audio output establishes delivery to the Windows driver, not a measurement of physical speaker/headphone loudness. Existing HDR and game-capture conclusions remain unchanged.

Verification: 40 workspace/executable tests, strict formatting/Clippy, 507 desktop checks across 18 runs, 40 layouts at 100–200% app scale, 48 playback lifecycle checks, GPU/media integrations, normal filename startup, relocated portable verification and nine packaging safeguards pass. Generated fixtures and detailed reports remain in ignored local artifacts. Original recordings are not used or modified by this audit.
