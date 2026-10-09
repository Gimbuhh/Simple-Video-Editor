# User guide

A portable Windows editor for trimming and combining gameplay recordings. Version 1.0.3.

## Run

Extract the entire portable ZIP, then double-click **Simple Video Editor.exe** at the top of the extracted folder. Keep **bin** beside it; it contains the video tools and libraries. Guides and licenses are in **docs**. No installer or separate runtime/video-tool setup is needed. Local builds create a **Simple Video Editor 1.0.3** shortcut pointing to the chosen app output folder.

Recordings must be on a local drive. Network shares, mapped network drives, device paths and unsupported linked/cloud locations are rejected before media access. Local symbolic links and junctions work when their targets are local. Copy unsupported recordings into an ordinary local folder first. Projects reject timelines outside the supported time range.

## Editing

1. Click **Import clips** or drop recordings into the window. The **Recordings** library keeps the original files separate from timeline edits.
2. Drag a recording onto the **Video** or **Audio** track. Double-click a library recording or use **Add to timeline** to append it. Dropping video files directly onto the timeline imports and inserts them. Moving a timeline clip carries its current preview frame along with it; a simple click seeks when you release the mouse.
3. Click the ruler or a clip to move the shared playhead. Scrubbing preserves the current playing or paused state, including across recordings. One **play/pause icon button**, or **Space**, toggles playback through all timeline cuts. **Left / Right** moves a frame while paused. Library recordings have a separate preview scrubber that also preserves playback state.
4. Drag either edge of a clip to trim it. Video and audio stay linked, and the preview keeps the current frame. If the new trim excludes that frame, the playhead clamps to the nearest retained frame. **Trim before (Q)** removes everything before the current frame; **Trim after (W)** removes everything after it. Both keep the current frame. You can also enter source in/out timestamps or press **I / O** at the playhead.
5. Press **Ctrl+B** or click **Split** to cut at the playhead. Split at both ends of an unwanted middle portion, select that middle clip, and press **Delete**. Joined following clips close the gap; clips across an existing gap stay in place.
6. Drag clips freely. Their edges automatically snap together when close, and moving a clip away detaches it. Trimming and deleting keep joined neighbors together without moving separate groups. Library drags show a thumbnail; existing timeline clips show their moving block. Cyan markers show drop destinations. Placement avoids overwriting other clips on this single linked track. Right-click for quick trims, split, duplicate, reset trim, move, and delete actions.
7. Use **Fit**, **+ / −**, or **Ctrl+mouse wheel** to zoom. The mouse wheel scrolls horizontally. Click **Export video** when ready.

The timeline has one video lane and one linked audio lane, with proportional durations and a global playhead. Both lanes edit the same clip; dragging or trimming either keeps video and audio together. Positions and gaps are preserved in projects, preview, and export; gaps show black video and silence. Snapping uses a small screen-distance threshold at every zoom level. While dragging a left trim edge, the opposite edge stays anchored where space permits; joined clips close the temporary gap when the gesture finishes. Isolated clips retain their positions when neighboring groups are edited. The audio lane shows a real waveform from the recording's first audio track, aligned to each clip's trim range. Peaks load in the background and are cached for reopening; the line is dotted while loading. Silence and video-only recordings show a flat line. The display makes quieter peaks easier to see; it does not change playback volume. A recording can be added repeatedly and split into as many separately editable parts as needed. Source files remain untouched.

Selecting a recording already visible in the library preserves the scroll position. Keyboard navigation reveals the selected item with pixel scrolling, and selecting an entirely offscreen recording from the timeline brings it into view.

**Add to timeline** appends the selected recording. When the library contains more than one recording, **Add all to timeline** appears beside it and appends every recording in library order, including recordings hidden by search. Each addition creates separately editable timeline clips, and a single undo removes the entire batch. Existing timeline edits and gaps stay intact.

Recording cards show the original duration once; exact source in/out points remain in the trim fields and timeline tooltips. New, Open, and Save disable while importing or opening a project. Save stays available for edited or existing projects, including a project intentionally cleared of all recordings.

Search the **Recordings** library by name with the search field or **Ctrl+F**. Search ignores letter case, shows the matching count, and filters only the library; the active preview and timeline edits stay intact. Use the clear button or **Escape** in the search field to show every recording again.

Playback moves a retained playhead drawing instead of reconstructing the timeline on every tick. Tracks redraw when clips, selection, zoom, the visible scroll region, or waveform data change. Timeline duration is calculated with clip offsets when edits occur, rather than rescanning all clips during every playhead update. Thumbnail images are released when the timeline is cleared.

The out point is the first excluded frame. Splitting requires at least one frame on each side. Trim handles move in frame-sized increments while preserving the original timestamp precision. Import, insert, split, delete, trim, reorder, and removal from the library support undo/redo. Removing a library recording from the project also removes its timeline clips, without deleting the file.

| Shortcut | Action |
|---|---|
| Ctrl+I | Import recordings |
| Ctrl+F | Search recording names |
| Ctrl+N / Ctrl+O | New / open project |
| Ctrl+S / Ctrl+Shift+S | Save / save as |
| Ctrl+Z / Ctrl+Y | Undo / redo |
| Space | Play / pause, continuing through timeline cuts |
| Left / Right | Previous / next timeline frame |
| I / O | Set the selected clip's in / out point |
| Q / W | Trim before / after the current frame, keeping that frame |
| Ctrl+B | Split at the playhead |
| Delete | Delete the selected clip and close the gap within its joined group |
| Escape while dragging | Cancel a free move or trim gesture |
| Alt+Left / Alt+Right | Swap the selected clip with its previous / next neighbor |
| Menu key / Shift+F10 | Open the selected timeline clip's context menu |
| Ctrl+mouse wheel | Zoom the timeline |
| Escape during import | Cancel import |

Timestamp fields accept `HH:MM:SS.mmm`, `MM:SS.mmm`, or seconds, using a decimal point. Unchanged fields preserve full timestamp precision even though the display rounds to milliseconds. Ordinary shortcuts do not replace text while a timestamp field has focus. Clicking a trim handle without moving it, or canceling import before a recording is added, leaves undo history unchanged.

## Export

- MP4 with **AV1** (default) or **H.264**.
- High, Balanced, and Smaller file quality presets. Size varies with scene complexity; these are quality settings rather than fixed size targets.
- Output resolution and frame rate follow the first clip, with optional 1080p or 720p resolution.
- NVIDIA encoding is enabled by default. Turn it off for CPU encoding if the driver/GPU is unavailable.
- All selected sections are encoded with matching output settings. The final join copies encoded video and encodes AAC audio once.
- Export progress and cancellation are supported. Existing outputs are replaced only after the new video finishes and passes a duration check.
- Temporary exports live beside the output file and are removed after completion or cancellation. Leave space for intermediate files and the finished video. If the computer loses power during export, a `.sve-export-*` folder may remain in the output folder and can be removed once the app is closed.

Audio is stereo, 48 kHz, using the first audio track. Video-only inputs receive silence. Mixed resolutions use letterboxing to preserve the image. Output uses the first recording's nominal frame rate; sections are rounded to whole output frames. Preview uses CPU decoding and software rendering, with its display buffer limited to 1280×720. Source files and exports retain their full resolution. The interface also uses software rendering, so the editor does not create its own Direct3D/OpenGL/Vulkan display surface for capture overlays to detect. NVIDIA export encoding remains available.

## Projects and recovery

`.sveproject` files store the recording library, timeline positions, and source in/out points, including unused recordings and multiple parts of one recording. Existing projects retain their positions and gaps; the previous build's mode flag is ignored. They do not embed video files. When a recording has moved, **Locate recordings** opens before the project loads. Select **Locate file…** for each missing recording. Selecting a file also locates matching filenames from the same original folder in its new folder. Cancel leaves the current session intact. Replacements must cover the existing cuts and be supported SDR videos. All cuts and timeline positions are preserved, including repeated parts of a recording and unused library recordings. Save the project to keep the new file paths; relinking alone does not overwrite it.

Unsaved sessions are saved after a one-second editing pause in `%LOCALAPPDATA%/SimpleVideoEditor/recovery/`. Each editor window owns a separate recovery file. On the next launch, you can restore the latest interrupted session; sessions still open in another window are left alone. Saving or closing a window clears only its own recovery. The previous version's `recovery.sveproject` file is also recognized. Thumbnails, waveform caches, and error logs are stored under `%LOCALAPPDATA%/SimpleVideoEditor/`.

Use **Cache…** to see thumbnail and waveform disk usage and **Clear cache** to reclaim that space. Cleanup targets only generated files in the thumbnail and waveform cache folders, including incomplete generated files; projects, recovery sessions, recordings, and unrelated files stay intact. Files in use by another window are reported and left alone. Current previews and loaded waveforms remain in memory; disk caches are recreated when needed. A running waveform analysis is canceled before cleanup and can restart when the cache dialog closes.

The app runs locally. It does not upload recordings or require an account. You can also drop a project file onto the window or pass recording/project paths as command-line arguments.

## Scope

Windows x64; optimized and verified with SDR 2560×1440, approximately 60 fps AV1 Shadowplay recordings. HDR clips are rejected with a clear message. This version supports multiple parts per recording, straight cuts, and one video track with linked audio. Independent audio editing, additional tracks, transitions, titles, and effects are outside this version's scope.
