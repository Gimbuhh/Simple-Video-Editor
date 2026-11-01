# NVIDIA captures the editor as a game

NVIDIA can classify the editor's GPU-rendered window as a game and display its overlay banner. In the local reproduction, NVIDIA also reported the editor's process as its current game. This is separate from the earlier Windows/NVIDIA game-switch color corruption.

An editor-only NVIDIA driver profile prevented that classification in isolated native SDR, native HDR and software-preview launches. The NVIDIA capture hook was absent, preview initialized, and capture settings remained unchanged. A real game was not running during these checks; they do not certify every game/editor focus transition or Instant Replay behavior.

## Apply or remove the local workaround

From a source checkout, use PowerShell 7. Inspection is read-only:

```powershell
./Development/scripts/nvidia-editor-profile.ps1
```

Close the editor, then run this once in an administrator PowerShell:

```powershell
./Development/scripts/nvidia-editor-profile.ps1 -Action Exclude
```

The default target is `App/Simple Video Editor.exe`. To use another packaged location, pass its full path with `-AppPath`. The profile matches that exact path, so another location needs its own exclusion. Replacing the build at the same path retains the profile. Run the editor normally afterward; it does not need administrator permission.

To undo the exclusion, close that editor and run in an administrator PowerShell:

```powershell
./Development/scripts/nvidia-editor-profile.ps1 -Action Restore
```

The helper creates a dedicated, single-application profile. It preserves existing profiles, changes no global/game settings, and does not start, stop or reset recording or replay. Restore removes only its unchanged owned profile; if someone has edited or shared that profile, it refuses to delete it. Repeated Exclude calls make no further changes.

## Scope and compatibility

This is optional machine configuration, separate from the portable executable. It is not run at startup or installed silently. The current machine's NVIDIA profiles for OBS, VLC and Blender use the same `0x809D5F60 = 0x10000000` flag. [NVIDIA Profile Inspector documents it](https://github.com/Orbmu2k/nvidiaProfileInspector/commit/9124f3c369afea634e54e5ba1843286dd7289935) as an overlay/statistics exclusion; NVIDIA does not publish it as a supported application opt-out contract. Driver updates may reset or change it, so inspect/retest if the problem returns.

NVIDIA has [described its application detection as proprietary](https://forums.developer.nvidia.com/t/how-to-prevent-in-game-overlay-for-my-glfw-opengl-application/276247). This workaround cannot guarantee game capture across every driver version. It does not hide the editor using capture-protection flags or prevent it from appearing in an intentional desktop recording.
