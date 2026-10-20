# Rust 1.1 test candidate

This candidate is undergoing parity and HDR verification. Keep your original
project files and recordings. The shipping release remains 1.0.4 until cutover.

Run `Simple Video Editor.exe` with its `bin` and `docs` folders alongside it.
Import local recordings or drop them into the window. Click a recording to
preview it; double-click it or drag it onto the timeline to add a linked section.
**Add all to timeline** adds the entire library, including recordings hidden by
the search filter, as one undoable edit.

The video and audio lanes represent the same section. Drag a section to move
both; drag either edge to trim it. Existing gaps separate joined groups. Escape
cancels an active gesture. Click the ruler or drag the preview scrubber to seek.

| Action | Shortcut |
| --- | --- |
| Play / pause | Space |
| Previous / next frame | Left / Right |
| Split at playhead | S |
| Set selected section's in / out | Q / W |
| Delete linked section | Delete |
| Undo / redo | Ctrl+Z / Ctrl+Y or Ctrl+Shift+Z |
| Save / save as | Ctrl+S / Ctrl+Shift+S |

Use the selected section's inspector for audio stream, gain, mute, fades, fit,
fill, crop and rotation. These settings are stored in the project and follow
split/duplicate operations. Output settings control dimensions, rational frame
rate, SDR or HDR10 color, codec, quality, and NVIDIA encoding. Lower quality
numbers preserve more detail. The app uses a CPU encoder if the selected NVIDIA
encoder cannot initialize.

The normal preview is an SDR reference. PQ and HLG recordings are deliberately
tone-mapped for that preview and for thumbnails. HDR10 output can use HDR display
preview on a compatible Windows HDR display. Physical HDR accuracy, changes
between displays and game capture coexistence are still acceptance checks;
ordinary screenshots cannot certify them.

Open existing `.sveproject` files to retain their exact trim boundaries. Locate
missing recordings when prompted. Opening a legacy project initially leaves
its Save destination empty; choose a new 1.1 filename. Recovery sessions use
Windows application data and separate leases for each running editor.

Export freezes the current project settings, preserves black/silent gaps, and
writes a replacement only after the result passes decode and color checks.
Cancel preserves the previous destination. The app rejects a source recording
as a project or export destination. **Cache** clears recognized generated
thumbnails and waveforms while preserving unrelated files and recovery data.
