//! Speed-weighted directional camera using the actual hull position and yaw.

const MAX_VALID_TICK_DISPLACEMENT: f64 = 8.0;
const MIN_MOVING_SPEED: f64 = 0.01;
const MAX_DIRECTIONAL_BLEND_SPEED: f32 = 3.0;
const VELOCITY_FILTER_ALPHA: f64 = 0.5;

pub(crate) struct Motion {
    position: [f64; 2],
    filtered_velocity: [f64; 2],
    previous_yaw: f32,
    yaw: f32,
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
            tracking: false,
        }
    }

    /// Sample the hull, not the rider's offset seat. The actual boat yaw is
    /// independent of mouse/touch input and of the camera's last synced yaw.
    pub(crate) fn sample(
        &mut self,
        position: [f64; 2],
        boat_yaw: f32,
        rider_yaw: f32,
        response: f32,
    ) -> bool {
        let dx = position[0] - self.position[0];
        let dz = position[1] - self.position[1];
        self.position = position;
        let speed = dx.hypot(dz);

        // Keep free look while stopped; ignore teleports and invalid samples.
        if !speed.is_finite()
            || !boat_yaw.is_finite()
            || !rider_yaw.is_finite()
            || !(MIN_MOVING_SPEED..=MAX_VALID_TICK_DISPLACEMENT).contains(&speed)
        {
            self.reset_to_view(rider_yaw);
            return false;
        }

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
        // When opposing samples cancel, don't invent a travel heading.
        let travel_yaw = if self.filtered_velocity[0].hypot(self.filtered_velocity[1]) < MIN_MOVING_SPEED {
            boat_yaw
        } else {
            (-self.filtered_velocity[0]).atan2(self.filtered_velocity[1]).to_degrees() as f32
        };
        let travel_weight = (speed as f32 / MAX_DIRECTIONAL_BLEND_SPEED).clamp(0.0, 1.0);
        let target = lerp_angle(travel_weight, boat_yaw, travel_yaw);

        let base = if self.tracking { self.yaw } else { rider_yaw };
        self.previous_yaw = base;
        self.yaw = lerp_angle(response.clamp(0.01, 1.0), base, target);
        self.tracking = true;
        true
    }

    fn reset_to_view(&mut self, rider_yaw: f32) {
        self.filtered_velocity = [0.0; 2];
        self.tracking = false;
        if rider_yaw.is_finite() {
            self.yaw = rider_yaw;
            self.previous_yaw = rider_yaw;
        }
    }

    pub(crate) fn is_tracking(&self) -> bool {
        self.tracking
    }

    pub(crate) fn yaw(&self) -> f32 {
        self.yaw
    }

    /// Render between ticks without feeding frames back into the filter.
    pub(crate) fn at(&self, partial: f32) -> f32 {
        self.previous_yaw + (self.yaw - self.previous_yaw) * partial.clamp(0.0, 1.0)
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
        assert!(cam.sample([0.2, 0.0], 0.0, 0.0, 0.5));
        assert!((cam.yaw() + 3.0).abs() < 0.01);
        assert!((cam.at(0.5) + 1.5).abs() < 0.01);
    }

    #[test]
    fn high_speed_follows_travel_but_smoothly() {
        let mut cam = Motion::new([0.0, 0.0], 0.0);
        assert!(cam.sample([3.0, 0.0], 0.0, 0.0, 0.5));
        assert!((cam.yaw() + 45.0).abs() < 0.01);
    }

    #[test]
    fn idle_returns_to_normal_free_look() {
        let mut cam = Motion::new([0.0, 0.0], 0.0);
        assert!(cam.sample([0.2, 0.0], 0.0, 0.0, 0.5));
        assert!(!cam.sample([0.2, 0.0], 0.0, 14.0, 0.5));
        assert!(!cam.is_tracking());
        assert_eq!(cam.yaw(), 14.0);
    }

    #[test]
    fn camera_feedback_and_free_look_do_not_change_hull_heading() {
        let mut a = Motion::new([0.0, 0.0], 0.0);
        let mut b = Motion::new([0.0, 0.0], 0.0);
        assert!(a.sample([0.2, 0.0], 0.0, 0.0, 0.5));
        assert!(b.sample([0.2, 0.0], 0.0, 0.0, 0.5));
        assert!(a.sample([0.4, 0.0], 0.0, -3.0, 0.5));
        assert!(b.sample([0.4, 0.0], 0.0, 80.0, 0.5));
        assert!((a.yaw() + 4.5).abs() < 0.01);
        assert_eq!(a.yaw(), b.yaw());
    }

    #[test]
    fn actual_hull_turn_changes_target_even_if_rider_yaw_does_not() {
        let mut cam = Motion::new([0.0, 0.0], 0.0);
        assert!(cam.sample([0.0, 0.3], 90.0, 0.0, 1.0));
        assert!((cam.yaw() - 81.0).abs() < 0.01);
    }

    #[test]
    fn corrections_do_not_make_the_camera_spin() {
        let mut cam = Motion::new([0.0, 0.0], 25.0);
        assert!(!cam.sample([500.0, 0.0], 0.0, 25.0, 0.5));
        assert_eq!(cam.yaw(), 25.0);
        assert!(cam.sample([500.0, 0.2], 0.0, 25.0, 0.5));
    }
}
