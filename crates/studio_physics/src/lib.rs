use bevy::prelude::*;
use rapier3d::prelude as rapier;
use rapier::nalgebra::Vector3;

pub struct PhysicsPlugin;

impl Plugin for PhysicsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(PhysicsState::new())
            .add_systems(Startup, setup_physics_scene)
            .add_systems(Update, (step_physics, sync_transforms).chain());
    }
}

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
}

impl Default for PhysicsState {
    fn default() -> Self {
        Self::new()
    }
}

/// Links a Bevy entity to a Rapier rigid body
#[derive(Component)]
pub struct RigidBodyLink(pub rapier::RigidBodyHandle);

fn setup_physics_scene(
    mut commands: Commands,
    mut physics: ResMut<PhysicsState>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
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

    // Falling cube (dynamic body)
    let cube_body = rapier::RigidBodyBuilder::dynamic().translation(Vector3::new(0.0, 5.0, 0.0));
    let cube_handle = p.rigid_body_set.insert(cube_body);
    let cube_collider = rapier::ColliderBuilder::cuboid(0.5, 0.5, 0.5);
    p.collider_set.insert_with_parent(
        cube_collider,
        cube_handle,
        &mut p.rigid_body_set,
    );

    // Spawn cube visual with link to physics body
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(1.0, 1.0, 1.0))),
        MeshMaterial3d(materials.add(Color::srgb(0.8, 0.2, 0.2))),
        Transform::from_xyz(0.0, 5.0, 0.0),
        RigidBodyLink(cube_handle),
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
