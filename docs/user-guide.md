# User guide

Simple Video Editor 1.1 edits local recordings non-destructively. Keep the executable, bin and docs folders together. The portable app needs no .NET runtime.

## Recordings and linked sections

Import files or drop them into the window. Click a recording to preview it; double-click it, use Add to timeline, or drag it onto the timeline to add a section. Add all includes the entire library in library order, including search-hidden recordings, as one undoable edit. A hidden selection cannot be added by the single-recording button. Escape clears search.

Dropping recordings from the library or File Explorer onto the timeline places linked sections at the drop point, choosing the nearest free space if needed. Dropping elsewhere imports into the library. You can also drop a project or recordings onto the executable, or pass filenames when launching it; a project opens with its saved trims and gaps, while recordings import in order.

The video and audio lanes are the same linked section. Moving, splitting, trimming or deleting a section moves both. Repeated sections of one recording have separate identities. Right-click a recording to remove it and all its sections from the project; the original file remains untouched and Undo restores the edit.

Move sections freely; nearby edges snap. Existing gaps separate joined groups. Trimming a left edge follows the pointer while the right edge/following sections stay anchored; releasing closes the temporary gap within a joined group. The preview retains the current source frame or clamps to the nearest retained frame. Escape restores the original gesture state. Delete closes only the connected group, preserving separate gaps.

Out points exclude the next frame. A split needs at least one retained frame on each side. Timestamp fields accept seconds, MM:SS.mmm or HH:MM:SS.mmm with a decimal point. Unchanged fields preserve their original precision. Frame stepping uses decoded timestamps, including VFR media, rather than the output frame rate.

## Navigation and shortcuts

Drag the recording scrubber or timeline ruler to seek. The pointer owns the playhead during the drag. Release submits the final seek and restores the previous play/pause intent after it settles. The timeline endpoint shows the final retained frame. Playback continues across sections and black/silent gaps.

| Shortcut | Action |
| --- | --- |
| Ctrl+I / Ctrl+F | Import / search recordings |
| Ctrl+N / Ctrl+O | New / open project |
| Ctrl+S / Ctrl+Shift+S | Save / save as |
| Ctrl+Z / Ctrl+Y or Ctrl+Shift+Z | Undo / redo |
| Ctrl+B or S | Split at the playhead |
| Space / Left / Right | Play or pause / previous or next frame |
| Q / W | Trim before / after, retaining the current frame |
| I / O | Set selected section in / out |
| Delete | Delete selected linked section |
| Alt+Left / Alt+Right | Swap with the previous / next section |
| Shift+F10 | Selected section context menu |
| Escape while dragging | Cancel the gesture |
| Escape during import | Cancel import |

Tab moves keyboard focus between controls, including timeline sections; Enter selects the focused section. A white inner outline identifies the focused section. Frame stepping, Split, Delete, Undo and Space playback remain available while a section has focus. Shift+F10 opens its context menu; Escape closes the menu and restores section focus. Text fields own ordinary editing keys while focused. Use the timeline Zoom slider or Fit to change the visible scale.

## Audio, framing and color

The inspector keeps In/Out and quick trims visible. Expand Audio, Framing or Recording color when needed; opening these groups does not change the project. Reset beside Trim restores the full recording.

Audio selects one source audio stream per section; the default uses the first stream. Gain, mute and fades are saved, undoable, previewed and exported. Output audio is stereo AAC at 48 kHz. Video-only sources receive silence. Separate music tracks and arbitrary mixing are later work.

Fit adds letterboxing. Fill covers the output frame. Crop uses normalized source coordinates; rotation is non-destructive. Intrinsic display rotation and non-square pixels are normalized before user framing. Export always reads original recordings.

Sources report transfer, primaries, matrix, range and pixel format. PQ and HLG are detected from transfer metadata; a 10-bit pixel format alone does not imply HDR. Unspecified YUV metadata defaults to SDR Rec.709 and unspecified RGB to sRGB. The selector under Recording color overrides incorrect source signaling and persists in the project.

Choose SDR Rec.709 or HDR10/PQ output. PQ and HLG sources are deliberately tone-mapped for SDR preview/export and library thumbnails. HDR10 output uses 10-bit Rec.2020/PQ through AV1 or HEVC. H.264 remains SDR. Native HDR preview is requested only when Windows reports active HDR on the preview display; software rendering always uses SDR. The preview shows an HDR or SDR indicator; hover it for the rendering mode. Select Software preview to explicitly use CPU-rendered tone-mapped video if native playback or capture compatibility causes problems. Changing Windows/display state is detected on the playback worker without changing system settings.

Current-display HDR appearance is user-confirmed and local game/Shadowplay controls pass. The user-confirmed Windows/NVIDIA HDR-to-SDR game-switch recording issue is external to the editor. Broader display switching and DPI/assistive-technology combinations remain tracked in the rewrite ledger. A negotiated PQ target or ordinary screenshot alone does not prove physical HDR output.

## Export

Choose dimensions, rational frame rate, color, codec, quality and GPU preference before exporting. Lower quality numbers preserve more detail. NVIDIA support is probed using the actual encoder; unsupported hardware falls back to CPU. Export freezes its project/settings snapshot, encodes sections and black/silent gaps consistently, then joins video with AAC audio.

Existing output is replaced only after duration/color/full-decode verification. Cancel preserves the prior destination. Project saves and exports reject source paths and source file identities, including hard links. Temporary .sve-export-* work folders are removed after completion or cancellation. An interrupted export may leave its own work folder beside the destination; remove it only once that editor is closed.

## Projects, recovery and cache

Opening schema-1 projects preserves library order, repeated recordings, gaps and exact trims without rewriting the original. Save initially asks for a new schema-2 filename; 1.0.x cannot read the new format. Keep the original project for rollback. Locate each missing recording when prompted; loading commits only after all replacements cover the saved cuts. Cancel leaves the current project open.

Projects save stable identities, audio/framing/color choices and output settings. Atomic replacement protects the previous saved file if writing fails. Unsaved recovery sessions live under %LOCALAPPDATA%/SimpleVideoEditor/recovery, each with an independent exclusive lease. Recovery skips live windows and other active prompts. Closing/discarding affects only the current session; the legacy recovery file is recognized.

Cache clears recognized generated thumbnails and waveform files. It preserves unrelated files, recovery and current source identities, and refuses linked cache folders. Native dependencies and decoded caches stay local; no recording is uploaded by the editor.
