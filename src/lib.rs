mod motion;
use mod_api::{
    CameraDistanceEvent, CameraPoseEvent, CameraType, ClientHost, ClientPacketEvent,
    ClientPacketSendEvent, ClientTickPostEvent, ClientTickPreEvent, ClipBlock, ClipFluid,
    EntityRef, Flow, Hooks, LevelRef, Registrar, RegistrationContext, SettingSpec,
};
use motion::Motion;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

/// Some servers skip an occasional paddle packet. Keep an existing ride across
/// a short gap instead of resetting the smoothing state on every missed tick.
const PADDLE_GRACE_TICKS: u8 = 2;

struct Ride {
    rider: EntityRef,
    level: LevelRef,
    camera: Motion,
    missed_paddles: u8,
    original_camera: CameraType,
}

#[derive(Default)]
pub struct Mod {
    paddle: AtomicBool,
    ride: Mutex<Option<Ride>>,
    boom: Mutex<Option<f32>>,
}

impl Mod {
    fn end_ride(&self, host: &dyn ClientHost) {
        let previous = self.ride.lock().unwrap().take();
        if let Some(ride) = previous {
            host.set_camera_type(ride.original_camera);
        }
        *self.boom.lock().unwrap() = None;
    }
}

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        r.register_setting(SettingSpec::toggle("enabled", "boatcam.enabled", true))
            .unwrap();
        r.register_setting(
            SettingSpec::range("smoothness", "boatcam.smoothness", 1.0, 100.0, 1.0, 50.0)
                .tooltip("boatcam.smoothness.tip"),
        )
        .unwrap();
        r.register_setting(SettingSpec::toggle("sync_view", "boatcam.sync_view", true))
            .unwrap();
        r.register_setting(SettingSpec::toggle("third_person", "boatcam.third_person", true))
            .unwrap();
    }

    fn client_tick_pre(&self, _: &mut dyn ClientHost, _: &mut ClientTickPreEvent) {
        self.paddle.store(false, Ordering::Relaxed);
    }

    fn client_packet_send(
        &self,
        _: &dyn ClientHost,
        event: &mut ClientPacketSendEvent<'_>,
    ) -> Flow {
        if event.packet.id.path() == "paddle_boat" {
            self.paddle.store(true, Ordering::Relaxed);
        }
        Flow::Continue
    }

    fn client_packet(&self, host: &mut dyn ClientHost, event: &mut ClientPacketEvent<'_>) {
        // `move_vehicle` arrives frequently in multiplayer and must NOT reset
        // smoothing. `set_passengers` is also sent for harmless seat updates.
        if matches!(event.packet.id.path(), "respawn" | "login") {
            self.end_ride(host);
            self.paddle.store(false, Ordering::Relaxed);
        }
    }

    fn client_tick_post(&self, host: &mut dyn ClientHost, _: &mut ClientTickPostEvent) {
        if host.is_paused() {
            return;
        }
        if !host.setting_bool("enabled").unwrap_or(true) {
            self.end_ride(host);
            return;
        }

        // This version of Horizon exposes vehicle relationships only server-side.
        // A local `paddle_boat` packet is the best supported client-side driving
        // signal. Retaining the same rider across brief gaps avoids jitter.
        let paddle = self.paddle.load(Ordering::Relaxed);
        let sample = (|| {
            let player = host.local_player()?.entity();
            if Some(player) != host.camera_entity() {
                return None;
            }
            let [x, _, z] = player.position(host)?;
            let [yaw, pitch] = player.rotation(host)?;
            Some((player, host.level(host.dimension())?, [x, z], yaw, pitch))
        })();

        let Some((rider, level, position, facing_yaw, pitch)) = sample else {
            self.end_ride(host);
            return;
        };

        let mut sync_yaw = None;
        let mut start_third_person = false;
        let mut camera_to_restore = None;
        {
            let mut ride = self.ride.lock().unwrap();
            if ride
                .as_ref()
                .is_some_and(|r| r.rider != rider || r.level != level)
            {
                camera_to_restore = ride.take().map(|r| r.original_camera);
            }

            if ride.is_none() && paddle {
                *ride = Some(Ride {
                    rider,
                    level,
                    camera: Motion::new(position, facing_yaw),
                    missed_paddles: 0,
                    original_camera: host.camera_type(),
                });
                start_third_person = host.setting_bool("third_person").unwrap_or(true);
            }

            if let Some(r) = ride.as_mut() {
                if paddle {
                    r.missed_paddles = 0;
                } else {
                    r.missed_paddles = r.missed_paddles.saturating_add(1);
                }
                if r.missed_paddles > PADDLE_GRACE_TICKS {
                    camera_to_restore = ride.take().map(|r| r.original_camera);
                } else {
                    let sync_view = host.setting_bool("sync_view").unwrap_or(true);
                    let tracking = r.camera.sample(
                        position,
                        facing_yaw,
                        (host.setting_f64("smoothness").unwrap_or(50.0) as f32 / 100.0)
                            .clamp(0.01, 1.0),
                        sync_view,
                    );
                    if tracking && sync_view {
                        sync_yaw = Some(r.camera.yaw());
                    }
                }
            }
        }
        if let Some(original) = camera_to_restore {
            host.set_camera_type(original);
        }
        if start_third_person {
            host.set_camera_type(CameraType::ThirdPersonBack);
        }
        if let Some(yaw) = sync_yaw {
            // The original BoatCam updates player YRot, not only rendered camera
            // yaw. Horizon's host method implements that without a game patch.
            if host.set_local_rotation(yaw, pitch).is_ok() {
                if let Some(r) = self.ride.lock().unwrap().as_mut()
                    && r.rider == rider && r.level == level
                {
                    r.camera.note_synced(yaw);
                }
            }
        }
    }

    fn camera_distance(&self, host: &dyn ClientHost, event: &mut CameraDistanceEvent<'_>) {
        let mut boom = self.boom.lock().unwrap();
        *boom = None;
        if let Some(r) = self.ride.lock().unwrap().as_ref()
            && host.camera_entity() == Some(r.rider)
            && r.camera.is_tracking()
        {
            // Keep vanilla's collision checks along the camera's NEW direction.
            *boom = Some(event.distance);
            event.distance = 0.0;
        }
    }

    fn camera_pose(&self, host: &dyn ClientHost, event: &mut CameraPoseEvent<'_>) {
        let boom = self.boom.lock().unwrap().take();
        let (yaw, level, rider) = {
            let ride = self.ride.lock().unwrap();
            let Some(r) = ride.as_ref() else { return };
            if !r.camera.is_tracking() {
                return;
            }
            (r.camera.at(event.partial), r.level, r.rider)
        };
        if host.camera_entity() != Some(rider) {
            return;
        }

        let perspective = host.camera_type();
        event.yaw = yaw + if perspective.is_mirrored() { 180.0 } else { 0.0 };
        if !perspective.is_first_person() && let Some(distance) = boom {
            let eye = *event.position;
            let (sy, cy) = f64::from(event.yaw).to_radians().sin_cos();
            let (sp, cp) = f64::from(event.pitch).to_radians().sin_cos();
            let back = [sy * cp, sp, -cy * cp];
            let mut length = f64::from(distance);
            for corner in 0..8 {
                let from = std::array::from_fn(|i| {
                    eye[i] + if corner & (1 << i) == 0 { -0.1 } else { 0.1 }
                });
                let to = std::array::from_fn(|i| from[i] + back[i] * length);
                let Some(hit) = level.clip(host, from, to, ClipBlock::Collider, ClipFluid::None)
                else {
                    length = 0.0;
                    break;
                };
                if !hit.miss {
                    let projected: f64 =
                        (0..3).map(|i| (hit.location[i] - from[i]) * back[i]).sum();
                    length = length.min((projected - 0.01).max(0.0));
                }
            }
            *event.position = std::array::from_fn(|i| eye[i] + back[i] * length);
        }
    }
}
