//! SimSitting — Rendering
//!
//! 2D particle rendering for 100,000 agents, camera controls (zoom/pan),
//! and knockout visuals. Agents are rendered as small sprites colored by
//! opinion value (purple = polarized, teal = moderate).
//!
//! Camera shake uses quadratic Perlin-style trauma (trauma² × amplitude)
//! for "Mike Tyson Knockout" moments when engagement spikes.

use bevy::prelude::*;
use crate::sim::{SimAgent, AgentVisual};
use crate::ui::UiState;
use rand::Rng;

/// Marker for the main game camera
#[derive(Component)]
pub struct GameCamera;

/// Camera control state
#[derive(Resource)]
pub struct CameraState {
    pub zoom: f32,
    /// Desired zoom level — scroll input sets this, actual `zoom` interpolates toward it
    pub target_zoom: f32,
    pub min_zoom: f32,
    pub max_zoom: f32,
    /// Interpolation speed (higher = snappier). 8.0 is AAA-feeling.
    pub zoom_speed: f32,
}

impl Default for CameraState {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            target_zoom: 1.0,
            min_zoom: 0.2,
            max_zoom: 3.0,
            zoom_speed: 8.0,
        }
    }
}

/// Camera shake intensity, driven by polarization and knockout state
#[derive(Resource)]
pub struct CameraShake {
    pub intensity: f32,
    pub trauma: f32,      // 0.0 to 1.0, decays over time
    pub decay_rate: f32,
}

impl Default for CameraShake {
    fn default() -> Self {
        Self {
            intensity: 0.0,
            trauma: 0.0,
            decay_rate: 1.5, // Decays to 0 in ~0.7s
        }
    }
}

impl CameraShake {
    /// Calculate shake offset using trauma squared (Vlambeer-style juice)
    pub fn offset(&self, seed: f32) -> Vec2 {
        let shake = self.trauma * self.trauma; // Quadratic for feel
        let max_offset = 15.0 * self.intensity;
        Vec2::new(
            (seed * 127.1).sin() * max_offset * shake,
            (seed * 269.5).sin() * max_offset * shake,
        )
    }

    /// Add trauma (clamped to 1.0)
    pub fn add_trauma(&mut self, amount: f32) {
        self.trauma = (self.trauma + amount).min(1.0);
    }
}

/// Setup: spawn 2D camera
pub fn setup_camera(mut commands: Commands) {
    commands.spawn((
        Camera2d,
        GameCamera,
        Transform::from_xyz(0.0, 0.0, 999.0),
    ));
}

/// Setup: create sprite meshes for all agents
pub fn setup_agent_sprites(
    mut commands: Commands,
    agents: Query<(Entity, &SimAgent, &Transform), With<AgentVisual>>,
) {
    for (entity, agent, _transform) in agents.iter() {
        let color = crate::sim::opinion_to_color(agent.opinion);

        commands.entity(entity).insert(Sprite {
            color,
            custom_size: Some(Vec2::new(4.0, 4.0)),
            ..default()
        });
    }
}

/// Compute one frame's smooth zoom step via exponential interpolation.
///
/// Pure function — fully testable without ECS.
/// Returns the new `current` value after one step toward `target`.
pub fn smooth_zoom_step(current: f32, target: f32, speed: f32, dt: f32) -> f32 {
    // Exponential interpolation: lerp factor = 1 - e^(-speed * dt)
    // Produces AAA-feeling smooth deceleration as it approaches target
    let t = 1.0 - (-speed * dt).exp();
    current + (target - current) * t
}

/// System: camera zoom with scroll wheel
pub fn camera_zoom(
    mut scroll_events: EventReader<bevy::input::mouse::MouseWheel>,
    mut cam_state: ResMut<CameraState>,
    time: Res<Time>,
) {
    for event in scroll_events.read() {
        let zoom_delta = -event.y * 0.1;
        cam_state.target_zoom = (cam_state.target_zoom + zoom_delta)
            .clamp(cam_state.min_zoom, cam_state.max_zoom);
    }
    // Smooth interpolation toward target
    cam_state.zoom = smooth_zoom_step(
        cam_state.zoom,
        cam_state.target_zoom,
        cam_state.zoom_speed,
        time.delta_secs(),
    );
}

/// System: camera pan with right mouse button drag + zoom via projection scale
/// Also applies camera shake offset during knockout
pub fn camera_pan(
    mouse: Res<ButtonInput<MouseButton>>,
    mut motion_events: EventReader<bevy::input::mouse::MouseMotion>,
    mut camera_q: Query<(&mut Transform, &mut Projection), With<GameCamera>>,
    cam_state: Res<CameraState>,
    shake: Res<CameraShake>,
    time: Res<Time>,
) {
    if let Ok((mut transform, mut projection)) = camera_q.single_mut() {
        // Apply zoom via orthographic projection (correct for frustum culling)
        if let Projection::Orthographic(ref mut ortho) = *projection {
            ortho.scale = cam_state.zoom;
        }

        // Apply shake offset
        let shake_offset = shake.offset(time.elapsed_secs());
        transform.translation.x += shake_offset.x * time.delta_secs();
        transform.translation.y += shake_offset.y * time.delta_secs();
    }

    if !mouse.pressed(MouseButton::Right) {
        motion_events.clear();
        return;
    }

    let mut delta = Vec2::ZERO;
    for event in motion_events.read() {
        delta += event.delta;
    }

    if delta != Vec2::ZERO {
        if let Ok((mut transform, _)) = camera_q.single_mut() {
            // Use cam_state.zoom for correct world-space panning
            transform.translation.x -= delta.x * cam_state.zoom;
            transform.translation.y += delta.y * cam_state.zoom;
        }
    }
}

/// System: update camera shake from simulation state.
/// Shake scales with polarization and spikes during knockout.
pub fn update_camera_shake(
    ui_state: Res<UiState>,
    mut shake: ResMut<CameraShake>,
    stats: Res<crate::economy::GlobalStats>,
    time: Res<Time>,
) {
    if ui_state.is_glitching {
        // Knockout: constant max trauma, high intensity
        shake.trauma = 1.0;
        shake.intensity = 2.0;
    } else {
        // Ambient: shake scales with polarization (subtle unease)
        shake.intensity = stats.polarization_heat.clamp(0.0, 1.0);
        shake.add_trauma(stats.polarization_heat * 0.01);
        // Decay trauma
        shake.trauma = (shake.trauma - shake.decay_rate * time.delta_secs()).max(0.0);
    }
}

/// System: during knockout, agents scatter at 100x speed (the simulation losing control)
pub fn knockout_agent_scatter(
    ui_state: Res<UiState>,
    mut agents: Query<&mut Transform, With<AgentVisual>>,
    time: Res<Time>,
) {
    if !ui_state.is_glitching {
        return;
    }

    let mut rng = rand::thread_rng();
    let speed_multiplier = 100.0;
    let dt = time.delta_secs();

    for mut transform in agents.iter_mut() {
        // Brownian explosion — agents scatter in random directions
        let dx = (rng.gen::<f32>() - 0.5) * speed_multiplier * dt;
        let dy = (rng.gen::<f32>() - 0.5) * speed_multiplier * dt;
        transform.translation.x += dx;
        transform.translation.y += dy;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // === Smooth Zoom Tests (TDD RED → GREEN) ===

    #[test]
    fn test_smooth_zoom_converges() {
        // After 10 steps at dt=0.016 (60fps), zoom should approach target
        let target = 2.0;
        let speed = 8.0;
        let mut current = 1.0;
        for _ in 0..10 {
            current = smooth_zoom_step(current, target, speed, 0.016);
        }
        // Should be within 30% of the target after 10 frames
        assert!((current - target).abs() < 0.7 * (target - 1.0),
            "After 10 frames, zoom should converge toward target. Got {}, expected near {}",
            current, target);
    }

    #[test]
    fn test_smooth_zoom_zero_dt_no_change() {
        let result = smooth_zoom_step(1.0, 2.0, 8.0, 0.0);
        assert!((result - 1.0).abs() < 0.001,
            "Zero dt should produce no movement. Got {}", result);
    }

    #[test]
    fn test_smooth_zoom_large_dt_snaps() {
        // Very large dt should effectively snap to target
        let result = smooth_zoom_step(1.0, 2.0, 8.0, 10.0);
        assert!((result - 2.0).abs() < 0.01,
            "Large dt should snap to target. Got {}", result);
    }

    #[test]
    fn test_smooth_zoom_already_at_target() {
        let result = smooth_zoom_step(1.5, 1.5, 8.0, 0.016);
        assert!((result - 1.5).abs() < 0.001,
            "Already at target — should stay. Got {}", result);
    }

    #[test]
    fn test_smooth_zoom_zooms_out() {
        // Zooming out: current > target
        let mut current = 2.0;
        for _ in 0..10 {
            current = smooth_zoom_step(current, 1.0, 8.0, 0.016);
        }
        assert!(current < 2.0 && current > 1.0,
            "Should move toward 1.0 from 2.0. Got {}", current);
    }

    // === Camera State Defaults ===

    #[test]
    fn test_camera_state_defaults() {
        let state = CameraState::default();
        assert!((state.zoom - 1.0).abs() < 0.001);
        assert!((state.target_zoom - 1.0).abs() < 0.001);
        assert!(state.min_zoom < state.max_zoom);
        assert!((state.zoom_speed - 8.0).abs() < 0.001);
    }

    #[test]
    fn test_camera_state_tightened_limits() {
        let state = CameraState::default();
        assert!((state.min_zoom - 0.2).abs() < 0.01, "Min zoom should be 0.2");
        assert!((state.max_zoom - 3.0).abs() < 0.01, "Max zoom should be 3.0");
    }

    // === Existing Camera Shake Tests ===

    #[test]
    fn test_camera_shake_offset_zero_trauma() {
        let shake = CameraShake::default();
        let offset = shake.offset(1.0);
        assert!((offset.x).abs() < 0.001);
        assert!((offset.y).abs() < 0.001);
    }

    #[test]
    fn test_camera_shake_offset_full_trauma() {
        let shake = CameraShake {
            intensity: 1.0,
            trauma: 1.0,
            decay_rate: 1.5,
        };
        let offset = shake.offset(1.0);
        assert!(offset.x.abs() > 0.0 || offset.y.abs() > 0.0);
        assert!(offset.x.abs() <= 15.0);
        assert!(offset.y.abs() <= 15.0);
    }

    #[test]
    fn test_camera_shake_quadratic_feel() {
        let shake = CameraShake {
            intensity: 1.0,
            trauma: 0.5,
            decay_rate: 1.5,
        };
        let offset_half = shake.offset(42.0).length();

        let shake_full = CameraShake {
            intensity: 1.0,
            trauma: 1.0,
            decay_rate: 1.5,
        };
        let offset_full = shake_full.offset(42.0).length();

        if offset_full > 0.01 {
            let ratio = offset_half / offset_full;
            assert!(ratio < 0.5, "Quadratic: half trauma should be < half offset, got {}", ratio);
        }
    }

    #[test]
    fn test_camera_shake_add_trauma_clamps() {
        let mut shake = CameraShake::default();
        shake.add_trauma(0.5);
        assert!((shake.trauma - 0.5).abs() < 0.001);
        shake.add_trauma(0.8);
        assert!((shake.trauma - 1.0).abs() < 0.001);
    }
}
