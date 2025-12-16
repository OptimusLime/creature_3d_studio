//! Physics simulation for Creature 3D Studio.
//!
//! This crate provides Rapier3D physics integration with Bevy, including:
//! - Physics world management (`PhysicsState`)
//! - Scene command queue (`CommandQueue`) for UI-driven physics operations
//! - Automatic transform synchronization between Rapier bodies and Bevy entities
//!
//! # Architecture
//!
//! Commands flow: UI -> `CommandQueue` -> `apply_scene_commands` -> Messages -> Handlers -> `PhysicsState`
//!
//! This decouples UI (including Lua scripts) from direct physics manipulation.

use bevy::prelude::*;
use rapier3d::prelude as rapier;
use rapier::nalgebra::Vector3;

/// Commands that can modify the physics scene.
/// Queued by UI (Lua or Rust) and applied by the physics system.
#[derive(Debug, Clone)]
pub enum SceneCommand {
    SpawnCube(Vec3),
    ClearBodies,
}

/// Queue of pending scene commands.
/// Decouples UI requests from physics execution.
#[derive(Resource, Default)]
pub struct CommandQueue {
    pending: Vec<SceneCommand>,
}

impl CommandQueue {
    /// Queue a command to spawn a cube at the given position.
    pub fn spawn_cube(&mut self, pos: Vec3) {
        self.pending.push(SceneCommand::SpawnCube(pos));
    }

    /// Queue a command to clear all dynamic bodies.
    pub fn clear(&mut self) {
        self.pending.push(SceneCommand::ClearBodies);
    }

    /// Drain all pending commands for processing.
    pub fn drain(&mut self) -> Vec<SceneCommand> {
        std::mem::take(&mut self.pending)
    }
}

/// Bevy plugin that initializes physics simulation.
///
/// Registers:
/// - `PhysicsState` resource (Rapier world)
/// - `CommandQueue` resource (pending scene commands)
/// - Physics stepping and transform sync systems
pub struct PhysicsPlugin;

impl Plugin for PhysicsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(PhysicsState::new())
            .insert_resource(CommandQueue::default())
            .add_message::<SpawnCubeEvent>()
            .add_message::<ClearBodiesEvent>()
            .add_systems(Startup, setup_physics_scene)
            .add_systems(Update, (
                apply_scene_commands,
                handle_spawn_cube,
                handle_clear_bodies,
                step_physics,
                sync_transforms,
            ).chain());
    }
}

/// Message to spawn a cube at a position
#[derive(Message, Debug)]
pub struct SpawnCubeEvent(pub Vec3);

/// Message to clear all dynamic bodies
#[derive(Message, Debug)]
pub struct ClearBodiesEvent;

/// Marker for dynamic bodies (can be cleared)
#[derive(Component)]
pub struct DynamicBody;

/// Holds all Rapier physics world state.
///
/// Contains rigid bodies, colliders, and simulation parameters.
/// Updated each frame by `step_physics` system.
#[derive(Resource)]
pub struct PhysicsState {
    pub gravity: Vector3<f32>,
    pub integration_parameters: rapier::IntegrationParameters,
    pub physics_pipeline: rapier::PhysicsPipeline,
    pub island_manager: rapier::IslandManager,
    pub broad_phase: rapier::DefaultBroadPhase,
    pub narrow_phase: rapier::NarrowPhase,
    pub rigid_body_set: rapier::RigidBodySet,
    pub collider_set: rapier::ColliderSet,
    pub impulse_joint_set: rapier::ImpulseJointSet,
    pub multibody_joint_set: rapier::MultibodyJointSet,
    pub ccd_solver: rapier::CCDSolver,
}

impl PhysicsState {
    pub fn new() -> Self {
        Self {
            gravity: Vector3::new(0.0, -9.81, 0.0),
            integration_parameters: rapier::IntegrationParameters::default(),
            physics_pipeline: rapier::PhysicsPipeline::new(),
            island_manager: rapier::IslandManager::new(),
            broad_phase: rapier::DefaultBroadPhase::new(),
            narrow_phase: rapier::NarrowPhase::new(),
            rigid_body_set: rapier::RigidBodySet::new(),
            collider_set: rapier::ColliderSet::new(),
            impulse_joint_set: rapier::ImpulseJointSet::new(),
            multibody_joint_set: rapier::MultibodyJointSet::new(),
            ccd_solver: rapier::CCDSolver::new(),
        }
    }

    /// Count of dynamic bodies
    pub fn dynamic_body_count(&self) -> usize {
        self.rigid_body_set
            .iter()
            .filter(|(_, b)| b.is_dynamic())
            .count()
    }
}

impl Default for PhysicsState {
    fn default() -> Self {
        Self::new()
    }
}

/// Links a Bevy entity to a Rapier rigid body
#[derive(Component)]
pub struct RigidBodyLink(pub rapier::RigidBodyHandle);

// Shared mesh/material handles for cubes
#[derive(Resource)]
struct CubeMeshMaterial {
    mesh: Handle<Mesh>,
    material: Handle<StandardMaterial>,
}

fn setup_physics_scene(
    mut commands: Commands,
    mut physics: ResMut<PhysicsState>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Store shared cube mesh/material
    let cube_mesh = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    let cube_material = materials.add(Color::srgb(0.8, 0.2, 0.2));
    commands.insert_resource(CubeMeshMaterial {
        mesh: cube_mesh,
        material: cube_material,
    });

    let p = physics.as_mut();

    // Ground plane (fixed body)
    let ground_body = rapier::RigidBodyBuilder::fixed().translation(Vector3::new(0.0, -0.5, 0.0));
    let ground_handle = p.rigid_body_set.insert(ground_body);
    let ground_collider = rapier::ColliderBuilder::cuboid(10.0, 0.5, 10.0);
    p.collider_set.insert_with_parent(
        ground_collider,
        ground_handle,
        &mut p.rigid_body_set,
    );

    // Spawn ground visual
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(20.0, 1.0, 20.0))),
        MeshMaterial3d(materials.add(Color::srgb(0.3, 0.5, 0.3))),
        Transform::from_xyz(0.0, -0.5, 0.0),
    ));

    // Add light
    commands.spawn((
        DirectionalLight {
            illuminance: 10000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.5, 0.5, 0.0)),
    ));
}

/// Drains the command queue and emits corresponding messages.
fn apply_scene_commands(
    mut commands: ResMut<CommandQueue>,
    mut spawn_events: MessageWriter<SpawnCubeEvent>,
    mut clear_events: MessageWriter<ClearBodiesEvent>,
) {
    for cmd in commands.drain() {
        match cmd {
            SceneCommand::SpawnCube(pos) => {
                spawn_events.write(SpawnCubeEvent(pos));
            }
            SceneCommand::ClearBodies => {
                clear_events.write(ClearBodiesEvent);
            }
        }
    }
}

fn handle_spawn_cube(
    mut commands: Commands,
    mut physics: ResMut<PhysicsState>,
    mut events: MessageReader<SpawnCubeEvent>,
    cube_assets: Option<Res<CubeMeshMaterial>>,
) {
    let Some(assets) = cube_assets else { return };

    for SpawnCubeEvent(pos) in events.read() {
        let p = physics.as_mut();

        // Create physics body
        let body = rapier::RigidBodyBuilder::dynamic()
            .translation(Vector3::new(pos.x, pos.y, pos.z));
        let handle = p.rigid_body_set.insert(body);
        let collider = rapier::ColliderBuilder::cuboid(0.5, 0.5, 0.5);
        p.collider_set.insert_with_parent(collider, handle, &mut p.rigid_body_set);

        // Spawn visual entity
        commands.spawn((
            Mesh3d(assets.mesh.clone()),
            MeshMaterial3d(assets.material.clone()),
            Transform::from_translation(*pos),
            RigidBodyLink(handle),
            DynamicBody,
        ));
    }
}

fn handle_clear_bodies(
    mut commands: Commands,
    mut physics: ResMut<PhysicsState>,
    mut events: MessageReader<ClearBodiesEvent>,
    query: Query<(Entity, &RigidBodyLink), With<DynamicBody>>,
) {
    for _ in events.read() {
        let p = physics.as_mut();

        // Remove all dynamic body entities and their physics bodies
        for (entity, link) in query.iter() {
            p.rigid_body_set.remove(
                link.0,
                &mut p.island_manager,
                &mut p.collider_set,
                &mut p.impulse_joint_set,
                &mut p.multibody_joint_set,
                true,
            );
            commands.entity(entity).despawn();
        }
    }
}

fn step_physics(mut physics: ResMut<PhysicsState>) {
    let p = physics.as_mut();
    p.physics_pipeline.step(
        &p.gravity,
        &p.integration_parameters,
        &mut p.island_manager,
        &mut p.broad_phase,
        &mut p.narrow_phase,
        &mut p.rigid_body_set,
        &mut p.collider_set,
        &mut p.impulse_joint_set,
        &mut p.multibody_joint_set,
        &mut p.ccd_solver,
        None,
        &(),
        &(),
    );
}

fn sync_transforms(physics: Res<PhysicsState>, mut query: Query<(&RigidBodyLink, &mut Transform)>) {
    for (link, mut transform) in query.iter_mut() {
        if let Some(body) = physics.rigid_body_set.get(link.0) {
            let pos = body.translation();
            let rot = body.rotation();
            transform.translation = Vec3::new(pos.x, pos.y, pos.z);
            transform.rotation = Quat::from_xyzw(rot.i, rot.j, rot.k, rot.w);
        }
    }
}
