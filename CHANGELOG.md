# Changelog

## 1.0.4 — 2026-10-09

### Changed

- Portable ZIP names include the app version. Local packaging verifies the new bundle before removing older generated ZIPs and checksums.

### Fixed

- Recording and timeline scrubbers follow the pointer smoothly without jumping to older decoded frames. Release seeks the final frame and restores the previous play/pause state.

## 1.0.3 — 2026-10-09

### Changed

- Timeline actions sit side by side, and compact recording rows leave more clips visible in small windows.
- The wider Recordings panel scales with the window and has a draggable divider for longer filenames.
- Preview frames fit the video's display aspect ratio, including portrait and rotated clips, without oversized black padding caused by window shape.
- Local build shortcuts include the app version and work for alternate app output folders.

### Fixed

- Project recording references are authorized before filesystem access. Network paths and unsupported linked locations are rejected; local symbolic links and junctions resolve to validated local targets.
- Extreme project timestamps are rejected, and timeline ruler rendering and zoom remain bounded at very small display scales.

### Added

- The Recordings section shows **Add all to timeline** alongside **Add to timeline** when the library contains multiple recordings. It appends all recordings in library order, including search-hidden recordings, as one undoable action.

## 1.0.2 — 2026-10-09

### Changed

- Portable downloads show one clearly named executable and short instructions, with video tools in `bin` and guides and licenses in `docs`.
- Moving an existing timeline clip shows the moving block without a floating thumbnail. Recordings dragged from the library retain their thumbnail preview.

### Fixed

- Moving or dragging the edges of a timeline clip preserves its preview frame and keeps the playhead aligned. Edge trims that remove the current frame clamp to the nearest retained frame. Ordinary clicks seek on release.

## 1.0.1 — 2026-10-09

### Fixed

- The white playhead stays steady while freely dragging a clip instead of jumping between two positions.
- Export cleanup retries briefly when Windows keeps a temporary file locked.

## 1.0.0 — 2026-10-09

### Added

- The recording library imports and previews gameplay clips, with name search and drag-and-drop onto the timeline.
- Linked video and audio tracks support free placement, automatic edge snapping, trimming, splitting, quick trims, duplication, and undo/redo. The audio track displays real waveforms.
- Playback follows cuts and gaps and preserves play/pause while scrubbing.
- AV1 and H.264 MP4 exports offer quality and resolution choices, CPU or NVIDIA encoding, progress, and cancellation without changing source recordings.
- Projects preserve the library and timeline edits, with interrupted-session recovery, missing-recording relinking, and preview-cache cleanup.
- The portable Windows app includes its runtime, video tools, and application icon.

### Known limitations

- Windows x64 and SDR video are supported. Editing uses one video track with linked audio and the first audio stream; HDR, separate audio editing, effects, transitions, and titles are unsupported.
- Software preview can briefly pause when switching recordings. NVIDIA Shadowplay capture targeting needs verification while a game is running.
