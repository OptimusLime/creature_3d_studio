//! Orbit camera controller for the creature studio.
//!
//! Provides a simple orbit camera that rotates around a target point.
//! - Left mouse drag: rotate camera (azimuth and elevation)
//! - Scroll wheel: zoom in/out

use bevy::prelude::*;
use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll};

/// Marker component for the orbit camera.
#[derive(Component)]
pub struct OrbitCamera {
    /// Point the camera orbits around
    pub target: Vec3,
    /// Distance from target
    pub distance: f32,
    /// Horizontal angle (radians)
    pub azimuth: f32,
    /// Vertical angle (radians), clamped to avoid gimbal lock
    pub elevation: f32,
    /// Mouse sensitivity for rotation
    pub sensitivity: f32,
    /// Zoom sensitivity
    pub zoom_sensitivity: f32,
}

impl Default for OrbitCamera {
    fn default() -> Self {
        Self {
            target: Vec3::ZERO,
            distance: 10.0,
            azimuth: 0.0,
            elevation: 0.5, // ~30 degrees
            sensitivity: 0.005,
            zoom_sensitivity: 1.0,
        }
    }
}

impl OrbitCamera {
    /// Create a new orbit camera with the given distance from target.
    pub fn new(distance: f32) -> Self {
        Self {
            distance,
            ..default()
        }
    }

    /// Set the target point to orbit around.
    pub fn with_target(mut self, target: Vec3) -> Self {
        self.target = target;
        self
    }

    /// Calculate the camera position based on current orbit parameters.
    pub fn calculate_position(&self) -> Vec3 {
        let x = self.distance * self.elevation.cos() * self.azimuth.sin();
        let y = self.distance * self.elevation.sin();
        let z = self.distance * self.elevation.cos() * self.azimuth.cos();
        self.target + Vec3::new(x, y, z)
    }
}

/// System that updates orbit camera based on mouse input.
pub fn orbit_camera_system(
    mouse_button: Res<ButtonInput<MouseButton>>,
    mouse_motion: Res<AccumulatedMouseMotion>,
    mouse_scroll: Res<AccumulatedMouseScroll>,
    mut query: Query<(&mut OrbitCamera, &mut Transform)>,
) {
    for (mut orbit, mut transform) in query.iter_mut() {
        // Rotate on left mouse drag
        if mouse_button.pressed(MouseButton::Left) {
            let delta = mouse_motion.delta;
            orbit.azimuth -= delta.x * orbit.sensitivity;
            orbit.elevation += delta.y * orbit.sensitivity;
            
            // Clamp elevation to avoid gimbal lock
            orbit.elevation = orbit.elevation.clamp(-1.4, 1.4); // ~80 degrees
        }

        // Zoom on scroll
        let scroll = mouse_scroll.delta.y;
        if scroll != 0.0 {
            orbit.distance -= scroll * orbit.zoom_sensitivity;
            orbit.distance = orbit.distance.clamp(1.0, 100.0);
        }

        // Update transform
        let position = orbit.calculate_position();
        transform.translation = position;
        transform.look_at(orbit.target, Vec3::Y);
    }
}

/// Plugin that adds orbit camera functionality.
pub struct OrbitCameraPlugin;

impl Plugin for OrbitCameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, orbit_camera_system);
    }
}

/// Bundle for spawning an orbit camera.
#[derive(Bundle, Default)]
pub struct OrbitCameraBundle {
    pub camera: Camera3d,
    pub orbit: OrbitCamera,
    pub transform: Transform,
}

impl OrbitCameraBundle {
    pub fn new(distance: f32, target: Vec3) -> Self {
        let orbit = OrbitCamera::new(distance).with_target(target);
        let position = orbit.calculate_position();
        Self {
            camera: Camera3d::default(),
            orbit,
            transform: Transform::from_translation(position).looking_at(target, Vec3::Y),
        }
    }
}
