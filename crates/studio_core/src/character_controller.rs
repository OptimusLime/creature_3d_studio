//! Shared character controller for voxel world navigation.
//!
//! Provides a reusable third-person character controller with:
//! - W/S move forward/backward relative to character facing
//! - A/D rotate the character (yaw)
//! - Up/Down arrows tilt the camera (pitch)
//! - Q/E adjust camera distance
//! - Jump with coyote time
//! - Kinematic collision response using voxel terrain
//!
//! The camera follows the character with a fixed relative position.

use bevy::prelude::*;

use crate::physics_math::{
    compute_kinematic_correction, detect_terrain_collisions, has_ceiling_contact, has_floor_contact,
};
use crate::voxel_fragment::TerrainOccupancy;

/// Plugin that adds the character controller systems.
pub struct CharacterControllerPlugin;

impl Plugin for CharacterControllerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CharacterControllerConfig>()
            .add_systems(
                Update,
                (
                    character_input_system,
                    character_physics_system,
                    character_camera_system,
                )
                    .chain(),
            );
    }
}

/// Configuration for the character controller.
#[derive(Resource)]
pub struct CharacterControllerConfig {
    /// Horizontal movement speed (units/sec)
    pub move_speed: f32,
    /// Initial upward velocity when jumping
    pub jump_speed: f32,
    /// Downward acceleration (units/sec^2)
    pub gravity: f32,
    /// Turn speed for A/D rotation (radians/sec)
    pub turn_speed: f32,
    /// Camera pitch speed (radians/sec)
    pub pitch_speed: f32,
    /// Camera distance adjustment speed (units/sec)
    pub zoom_speed: f32,
    /// Minimum pitch angle (radians, negative = look up)
    pub min_pitch: f32,
    /// Maximum pitch angle (radians, positive = look down)
    pub max_pitch: f32,
    /// Minimum camera distance
    pub min_distance: f32,
    /// Maximum camera distance
    pub max_distance: f32,
}

impl Default for CharacterControllerConfig {
    fn default() -> Self {
        Self {
            move_speed: 10.0,
            jump_speed: 12.0,
            gravity: 25.0,
            turn_speed: 2.0,
            pitch_speed: 1.5,
            zoom_speed: 10.0,
            // Allow looking almost straight up (-PI/2) and straight down (+PI/2)
            min_pitch: -1.5, // ~86 degrees up
            max_pitch: 1.5,  // ~86 degrees down
            min_distance: 5.0,
            max_distance: 50.0,
        }
    }
}

/// Component that marks an entity as a player character.
/// The character has position and rotation (yaw). WASD controls it.
#[derive(Component)]
pub struct PlayerCharacter {
    /// Current velocity
    pub velocity: Vec3,
    /// Character's facing direction (yaw in radians)
    pub yaw: f32,
    /// Whether the character is on the ground
    pub grounded: bool,
    /// Timer for jump grace period
    jump_timer: f32,
    /// Half-extents of the character's collision box
    pub half_extents: Vec3,
}

impl Default for PlayerCharacter {
    fn default() -> Self {
        Self {
            velocity: Vec3::ZERO,
            yaw: 0.0,
            grounded: false,
            jump_timer: 0.0,
            half_extents: Vec3::new(0.4, 0.9, 0.4),
        }
    }
}

impl PlayerCharacter {
    /// Create a player character with custom collision box size.
    pub fn with_half_extents(half_extents: Vec3) -> Self {
        Self {
            half_extents,
            ..Default::default()
        }
    }
}

/// Component for third-person camera.
/// Camera follows the character with a fixed offset. Has its own pitch (tilt).
#[derive(Component)]
pub struct ThirdPersonCamera {
    /// Vertical tilt (radians, negative = look up, positive = look down)
    pub pitch: f32,
    /// Distance from character
    pub distance: f32,
    /// Height offset from character origin
    pub height_offset: f32,
}

impl Default for ThirdPersonCamera {
    fn default() -> Self {
        Self {
            pitch: 0.2,
            distance: 15.0,
            height_offset: 1.5,
        }
    }
}

impl ThirdPersonCamera {
    pub fn with_distance(distance: f32) -> Self {
        Self {
            distance,
            ..Default::default()
        }
    }
}

/// System that handles player input for movement and camera control.
fn character_input_system(
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    config: Res<CharacterControllerConfig>,
    mut player_query: Query<&mut PlayerCharacter>,
    mut camera_query: Query<&mut ThirdPersonCamera>,
) {
    let Ok(mut player) = player_query.single_mut() else {
        return;
    };
    let Ok(mut camera) = camera_query.single_mut() else {
        return;
    };

    let dt = time.delta_secs();

    // Update jump timer
    if player.jump_timer > 0.0 {
        player.jump_timer -= dt;
    }

    // A/D rotate the CHARACTER (yaw)
    if keyboard.pressed(KeyCode::KeyA) {
        player.yaw -= config.turn_speed * dt;
    }
    if keyboard.pressed(KeyCode::KeyD) {
        player.yaw += config.turn_speed * dt;
    }

    // Up/Down arrows tilt the CAMERA (pitch) - normal speed
    if keyboard.pressed(KeyCode::ArrowUp) {
        camera.pitch =
            (camera.pitch - config.pitch_speed * dt).clamp(config.min_pitch, config.max_pitch);
    }
    if keyboard.pressed(KeyCode::ArrowDown) {
        camera.pitch =
            (camera.pitch + config.pitch_speed * dt).clamp(config.min_pitch, config.max_pitch);
    }

    // I/K for fast free-look pitch (2x speed for quickly looking at sky)
    let fast_pitch_speed = config.pitch_speed * 2.0;
    if keyboard.pressed(KeyCode::KeyI) {
        camera.pitch =
            (camera.pitch - fast_pitch_speed * dt).clamp(config.min_pitch, config.max_pitch);
    }
    if keyboard.pressed(KeyCode::KeyK) {
        camera.pitch =
            (camera.pitch + fast_pitch_speed * dt).clamp(config.min_pitch, config.max_pitch);
    }

    // Q/E adjust camera distance
    if keyboard.pressed(KeyCode::KeyQ) {
        camera.distance = (camera.distance - config.zoom_speed * dt)
            .clamp(config.min_distance, config.max_distance);
    }
    if keyboard.pressed(KeyCode::KeyE) {
        camera.distance = (camera.distance + config.zoom_speed * dt)
            .clamp(config.min_distance, config.max_distance);
    }

    // W/S move forward/backward relative to CHARACTER facing
    let mut input = Vec3::ZERO;
    if keyboard.pressed(KeyCode::KeyW) {
        input.z -= 1.0;
    }
    if keyboard.pressed(KeyCode::KeyS) {
        input.z += 1.0;
    }

    // Rotate input by character yaw
    let rotation = Quat::from_rotation_y(-player.yaw);
    let mut move_dir = rotation * input;
    if move_dir.length_squared() > 0.0 {
        move_dir = move_dir.normalize();
    }

    // Set horizontal velocity
    player.velocity.x = move_dir.x * config.move_speed;
    player.velocity.z = move_dir.z * config.move_speed;

    // Jump
    if keyboard.just_pressed(KeyCode::Space) && player.grounded {
        player.velocity.y = config.jump_speed;
        player.grounded = false;
        player.jump_timer = 0.15;
    }
}

/// System that applies physics (gravity, collision) to the player character.
fn character_physics_system(
    time: Res<Time>,
    config: Res<CharacterControllerConfig>,
    terrain: Option<Res<TerrainOccupancy>>,
    mut player_query: Query<(&mut PlayerCharacter, &mut Transform)>,
) {
    let Some(terrain) = terrain else {
        return;
    };

    let dt = time.delta_secs().min(0.05); // Cap delta time

    for (mut player, mut transform) in player_query.iter_mut() {
        let check_ground = player.jump_timer <= 0.0;

        // Apply gravity when not grounded
        if !player.grounded {
            player.velocity.y -= config.gravity * dt;
        }

        // Apply character rotation (yaw) to transform
        transform.rotation = Quat::from_rotation_y(-player.yaw);

        // Integrate position
        transform.translation += player.velocity * dt;

        // Collision detection - sample multiple points on the character
        let half = player.half_extents;
        let particle_diameter = 0.5;

        let sample_offsets = [
            // Bottom layer (feet)
            Vec3::new(-half.x, -half.y, -half.z),
            Vec3::new(half.x, -half.y, -half.z),
            Vec3::new(-half.x, -half.y, half.z),
            Vec3::new(half.x, -half.y, half.z),
            Vec3::new(0.0, -half.y, 0.0),
            // Middle layer (body)
            Vec3::new(-half.x, 0.0, 0.0),
            Vec3::new(half.x, 0.0, 0.0),
            Vec3::new(0.0, 0.0, -half.z),
            Vec3::new(0.0, 0.0, half.z),
            // Top layer (head)
            Vec3::new(0.0, half.y, 0.0),
        ];

        let mut all_contacts = Vec::new();
        for offset in &sample_offsets {
            let sample_pos = transform.translation + *offset;
            let contacts =
                detect_terrain_collisions(sample_pos, &terrain.occupancy, particle_diameter);
            all_contacts.extend(contacts);
        }

        // Check floor/ceiling contacts
        let floor_contact = check_ground && has_floor_contact(&all_contacts);
        let ceiling_contact = has_ceiling_contact(&all_contacts);

        // Apply position correction
        let correction = compute_kinematic_correction(&all_contacts);
        transform.translation += correction;

        // Update grounded state and velocity
        if floor_contact {
            player.grounded = true;
            if player.velocity.y < 0.0 {
                player.velocity.y = 0.0;
            }
        } else {
            player.grounded = false;
        }

        if ceiling_contact && player.velocity.y > 0.0 {
            player.velocity.y = 0.0;
        }

        // Cancel velocity into walls
        if correction.x.abs() > 0.001 {
            if (correction.x > 0.0 && player.velocity.x < 0.0)
                || (correction.x < 0.0 && player.velocity.x > 0.0)
            {
                player.velocity.x = 0.0;
            }
        }
        if correction.z.abs() > 0.001 {
            if (correction.z > 0.0 && player.velocity.z < 0.0)
                || (correction.z < 0.0 && player.velocity.z > 0.0)
            {
                player.velocity.z = 0.0;
            }
        }

        // Small downward velocity when grounded to maintain contact
        if player.grounded && player.velocity.y == 0.0 {
            player.velocity.y = -0.5;
        }
    }
}

/// System that updates the camera to follow the player.
fn character_camera_system(
    player_query: Query<&Transform, With<PlayerCharacter>>,
    mut camera_query: Query<(&mut Transform, &ThirdPersonCamera), Without<PlayerCharacter>>,
) {
    let Ok(player_transform) = player_query.single() else {
        return;
    };
    let Ok((mut camera_transform, camera)) = camera_query.single_mut() else {
        return;
    };

    // Camera is BEHIND the player - use player's back direction
    let player_back = player_transform.rotation * Vec3::Z; // +Z is back

    // Offset: behind player horizontally, plus pitch for vertical
    let horizontal_dist = camera.distance * camera.pitch.cos();
    let vertical_dist = camera.distance * camera.pitch.sin();

    let offset = player_back * horizontal_dist + Vec3::Y * vertical_dist;

    // Target point is player position plus height offset
    let target_pos = player_transform.translation + Vec3::Y * camera.height_offset;

    camera_transform.translation = target_pos + offset;
    camera_transform.look_at(target_pos, Vec3::Y);
}
