# BoatCam for Horizon / Nova iOS (v0.3.0)

Standalone client-side WebAssembly mod for Nova iOS. The matching Nova SDK is
[commit `1934f13b7992f123bbf5e200dbca8836d9cc3779`](https://github.com/kipd0/nova-ios/commit/1934f13b7992f123bbf5e200dbca8836d9cc3779)
on the `newupdate` branch. The game/IPA does not need to be rebuilt to install
the mod on that matching launcher.

## What's new

- Uses the client API's actual mounted vehicle, hull position and hull yaw
  instead of estimating boat orientation from the rider's camera.
- Blends hull facing toward travel direction by speed / 3, then applies
  configurable angular smoothing. Moving the camera cannot feed back into
  the hull heading estimate.
- Checks the actual mount each tick, restoring perspective when the player
  dismounts or changes vehicles. Recognizes vanilla boats, chest boats and
  rafts; outgoing paddle packets still identify the driver (the controlling
  passenger API remains server-side), with a two-tick grace period.
- Preserves free look while stopped, optional view sync and auto third-person
  back, interpolation between ticks, and camera collision clipping.
- Camera-only: does not modify hull physics, steering, movement packets or
  gameplay rules.

## Settings

Open **Options → Mods → BoatCam → Config**.

| Setting | Default | Meaning |
| --- | --- | --- |
| Enable BoatCam | on | Track direction while driving a boat or raft |
| Turning response | 50 | 1 = lazy, 100 = reacts quickly |
| Sync player direction to camera | on | Also rotates the actual player view |
| Switch to third-person in boats | on | Automatically select third-person back |

## Build

Run `sh build.sh` on Linux/macOS/Termux with Git and rustup installed. It
fetches the exact Nova revision above, runs camera math regression tests, and
uses the matching Horizon packager to produce `boatcam.hmod`.

If you already have a compatible Nova checkout, use
`sh build-local.sh "$HOME/minecraft/nova-ios"`. The local checkout must match
the game build you install the mod on. The scripts use the SDK's pinned Rust
toolchain and may need network access for dependencies.

GitHub Actions also builds and uploads the `boatcam-hmod` artifact:
[Build BoatCam](https://github.com/kipd0/boatcam-for-horizon/actions).

## Install

Copy the newly built `boatcam.hmod` into **Files → Nova → Game → horizon-mods/**,
replacing the old file, and restart the game. In `horizon-mods.txt`, use
`mod.boatcam:true` if enabling manually.

## Caveats

The original BoatCam also intercepts mouse-look and passenger yaw clamping;
this portable mod does not reproduce those hooks or separate look buttons.
The driver check still uses paddle packets, so the exact mount alone does not
activate the camera for a passenger. Device testing is needed for camera
behavior and runtime compatibility; build success is checked by GitHub Actions.
