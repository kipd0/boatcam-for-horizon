# BoatCam for Horizon / Nova iOS — improved source (v0.2.0)

This is a **source-only release**. It is not a compiled `.hmod`. Rebuild with a matching Horizon mod SDK before installing. It is a standalone WebAssembly mod; rebuilding Nova's IPA is not needed.

## What changed

- Directional yaw now follows the original BoatCam formula: blend **facing direction** toward **travel direction** by `speed / 3`, then ease the camera toward the blended angle by the smoothness percentage.
- The original uses **boat yaw** as the facing direction. This version of the Horizon API does not expose the vehicle's yaw to client WebAssembly mods, so the port uses the **rider's yaw** as the closest available approximation. This means it cannot perfectly match the original during drifting/steering.
- Camera angles can now be synchronized to the **actual player view**, rather than changing only the rendered camera. This also makes the crosshair and camera direction agree (the game still controls steering, collision and input).
- Auto-selects **third-person back** when it identifies a driven boat, and restores the user's previous camera perspective when exiting.
- Lets the normal camera/mouse look work when stopped (rather than freezing the last travelling heading).
- Ignores frequent `move_vehicle` and harmless passenger-update packets instead of discarding the smoothing state for every such packet. Tolerates up to two missing paddle-packet ticks.
- Filters seat-position movement noise and ignores large server correction jumps.

## Settings

Open **Options → Mods → BoatCam → Config**.

| Setting | Default | Meaning |
| --- | --- | --- |
| Enable BoatCam | on | Follow direction while driving a boat or raft |
| Turning response | 50 | Original BoatCam style: 1 = lazy, 100 = reacts quickly |
| Sync player direction to camera | on | Also rotates the actual client player view; turn off if it interferes with another mod or control scheme |
| Switch to third-person in boats | on | Like the original default perspective |

Previous versions used a different meaning for smoothness, so reset the setting to **50** for a comparable baseline if you already have a saved value.

## Build on Termux / Linux / macOS

If you already have the matching Nova checkout at `~/minecraft/nova-ios`, run the following from the source directory:

```sh
sh build-local.sh "$HOME/minecraft/nova-ios"
```

The resulting `boatcam.hmod` will appear beside `build-local.sh`. This uses the existing source checkout and its build cache; it does not download the entire repository again.

Without an existing checkout, run `sh build.sh` to download the pinned, compatible source. You'll need a working Rust/rustup toolchain and network access to Rust dependencies for the first build.

The source and packager target Nova revision: `f3cd51d53a82bb8ae6932793a75e61b39960fad6`.

## Install

Copy the **newly built** `boatcam.hmod` into **Files → Nova → Game → horizon-mods/**, replacing the previous one. Restart the game. In `horizon-mods.txt`, use `mod.boatcam:true` if enabling it manually.

## Caveats

- The client API at the pinned revision exposes the vehicle relationship and its rotation only on the **server** side, so the mod still infers driving from local outgoing `paddle_boat` packets and motion from the rider's position.
- The original BoatCam also intercepts mouse-look and boat passenger yaw clamping. Those behaviors are not replicated by this portable `.hmod` and can affect the feeling on turns.
- The port has no separate look-left/right/behind buttons or Velocity mode yet; this version focuses on achieving a more natural **Directional** mode.
- This version has not been tested on an iPad or compiled in this environment; the Rust/Wasm toolchain was unavailable here. The code was checked against the host API definitions and the motion math was tested separately.
