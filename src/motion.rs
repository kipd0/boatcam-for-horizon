//! BoatCam-inspired directional camera.
//!
//! Unlike the old velocity-only port, direction is blended with the rider's
//! rotation according to speed before applying BoatCam's angular smoothing.
//! The client-side Horizon API does not currently expose the vehicle's yaw;
//! the rider yaw is the closest available approximation for a standalone .hmod.

const MAX_VALID_TICK_DISPLACEMENT: f64 = 8.0;
const MIN_MOVING_SPEED: f64 = 0.01;
const MAX_DIRECTIONAL_BLEND_SPEED: f32 = 3.0;
const VELOCITY_FILTER_ALPHA: f64 = 0.5;
// Approximate boat rotation by observing the rider after the previous view sync.
// Larger deltas are likely touch/mouse movement, not the boat turning this tick.
const MAX_ESTIMATED_BOAT_TURN: f32 = 12.0;

pub(crate) struct Motion {
    position: [f64; 2],
    filtered_velocity: [f64; 2],
    previous_yaw: f32,
    yaw: f32,
    facing_yaw: f32,
    last_synced_yaw: Option<f32>,
    tracking: bool,
}

fn wrap_degrees(angle: f32) -> f32 {
    (angle + 180.0).rem_euclid(360.0) - 180.0
}

fn lerp_angle(alpha: f32, from: f32, to: f32) -> f32 {
    from + alpha.clamp(0.0, 1.0) * wrap_degrees(to - from)
}

impl Motion {
    pub(crate) fn new(position: [f64; 2], yaw: f32) -> Self {
        Self {
            position,
            filtered_velocity: [0.0; 2],
            previous_yaw: yaw,
            yaw,
            facing_yaw: yaw,
            last_synced_yaw: None,
            tracking: false,
        }
    }

    /// Returns whether automatic camera tracking should run this tick.
    /// `response` matches the original's Directional.smoothness / 100:
    /// low values turn more lazily; high values react faster.
    pub(crate) fn sample(
        &mut self,
        position: [f64; 2],
        rider_yaw: f32,
        response: f32,
        sync_view: bool,
    ) -> bool {
        let dx = position[0] - self.position[0];
        let dz = position[1] - self.position[1];
        self.position = position;
        let speed = dx.hypot(dz);

        // Avoid noise from stationary riders and teleports/server corrections.
        if !speed.is_finite()
            || !rider_yaw.is_finite()
            || !(MIN_MOVING_SPEED..=MAX_VALID_TICK_DISPLACEMENT).contains(&speed)
        {
            self.reset_to_view(rider_yaw);
            return false;
        }

        // With view sync, feeding the last camera yaw straight back as the
        // "boat yaw" would cause an endless slow turn toward travel direction.
        // Keep a separate heading estimate instead, advancing it only by the
        // rider's *new* rotation since our last view sync. Riding applies the
        // boat's turn to its rider, so this captures turns on ice as well.
        // It remains an approximation: mouse input cannot be distinguished
        // perfectly without the actual client boat rotation API.
        if sync_view && self.tracking {
            if let Some(last) = self.last_synced_yaw {
                let change = wrap_degrees(rider_yaw - last)
                    .clamp(-MAX_ESTIMATED_BOAT_TURN, MAX_ESTIMATED_BOAT_TURN);
                self.facing_yaw += change;
            }
        } else {
            self.facing_yaw = rider_yaw;
        }
        self.last_synced_yaw = None;

        if self.tracking {
            self.filtered_velocity[0] =
                self.filtered_velocity[0] * (1.0 - VELOCITY_FILTER_ALPHA)
                    + dx * VELOCITY_FILTER_ALPHA;
            self.filtered_velocity[1] =
                self.filtered_velocity[1] * (1.0 - VELOCITY_FILTER_ALPHA)
                    + dz * VELOCITY_FILTER_ALPHA;
        } else {
            self.filtered_velocity = [dx, dz];
        }

        // Minecraft yaw: south=0, west=90, north=180, east=-90.
        let travel_yaw = (-self.filtered_velocity[0])
            .atan2(self.filtered_velocity[1])
            .to_degrees() as f32;

        // Original BoatCam: lerp(speed / 3, boatYaw, direction).
        // The best available substitute is our estimated facing direction.
        let travel_weight = (speed as f32 / MAX_DIRECTIONAL_BLEND_SPEED).clamp(0.0, 1.0);
        let target = lerp_angle(travel_weight, self.facing_yaw, travel_yaw);

        let base = if self.tracking { self.yaw } else { rider_yaw };
        self.previous_yaw = base;
        self.yaw = lerp_angle(response.clamp(0.01, 1.0), base, target);
        self.tracking = true;
        true
    }

    pub(crate) fn reset_to_view(&mut self, rider_yaw: f32) {
        self.filtered_velocity = [0.0; 2];
        self.last_synced_yaw = None;
        self.tracking = false;
        if rider_yaw.is_finite() {
            self.yaw = rider_yaw;
            self.previous_yaw = rider_yaw;
            self.facing_yaw = rider_yaw;
        }
    }

    /// Called only when Horizon successfully synchronized player rotation.
    pub(crate) fn note_synced(&mut self, applied_yaw: f32) {
        self.last_synced_yaw = Some(applied_yaw);
    }

    pub(crate) fn is_tracking(&self) -> bool {
        self.tracking
    }

    pub(crate) fn yaw(&self) -> f32 {
        self.yaw
    }

    /// Render at the partial tick without feeding render frames into the filter.
    pub(crate) fn at(&self, partial: f32) -> f32 {
        self.previous_yaw
            + (self.yaw - self.previous_yaw) * partial.clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::{lerp_angle, Motion};

    #[test]
    fn angle_wrap_uses_shortest_turn() {
        assert!((lerp_angle(0.5, 179.0, -179.0) - 180.0).abs() < 0.01);
        assert!((lerp_angle(0.5, -179.0, 179.0) + 180.0).abs() < 0.01);
    }

    #[test]
    fn slow_sideways_slide_changes_yaw_gently() {
        let mut cam = Motion::new([0.0, 0.0], 0.0);
        assert!(cam.sample([0.2, 0.0], 0.0, 0.5, true));
        // Original directional weighting: (-90deg * 0.2/3) * 0.5 = -3deg.
        assert!((cam.yaw() + 3.0).abs() < 0.01, "yaw={}", cam.yaw());
        assert!((cam.at(0.5) + 1.5).abs() < 0.01);
    }

    #[test]
    fn high_speed_follows_travel_but_smoothly() {
        let mut cam = Motion::new([0.0, 0.0], 0.0);
        assert!(cam.sample([3.0, 0.0], 0.0, 0.5, true));
        assert!((cam.yaw() + 45.0).abs() < 0.01);
    }

    #[test]
    fn idle_returns_to_normal_free_look() {
        let mut cam = Motion::new([0.0, 0.0], 0.0);
        assert!(cam.sample([0.2, 0.0], 0.0, 0.5, true));
        assert!(!cam.sample([0.2, 0.0], 14.0, 0.5, true));
        assert!(!cam.is_tracking());
        assert_eq!(cam.yaw(), 14.0);
    }

    #[test]
    fn synced_view_does_not_feed_the_camera_yaw_back_as_boat_yaw() {
        let mut cam = Motion::new([0.0, 0.0], 0.0);
        assert!(cam.sample([0.2, 0.0], 0.0, 0.5, true));
        assert!((cam.yaw() + 3.0).abs() < 0.01);
        cam.note_synced(cam.yaw());
        // Rider reports the yaw we just applied. Boat facing is still 0deg.
        assert!(cam.sample([0.4, 0.0], -3.0, 0.5, true));
        assert!((cam.yaw() + 4.5).abs() < 0.01, "yaw={}", cam.yaw());
    }

    #[test]
    fn corrections_do_not_make_the_camera_spin() {
        let mut cam = Motion::new([0.0, 0.0], 25.0);
        assert!(!cam.sample([500.0, 0.0], 25.0, 0.5, true));
        assert_eq!(cam.yaw(), 25.0);
    }
}
