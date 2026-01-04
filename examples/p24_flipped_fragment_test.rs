//! Phase 24: Fragment Collision Alignment Test
//!
//! This example tests that fragment meshes align with their physics collision.
//! It places fragments at EXACT positions where they should NOT intersect terrain,
//! and verifies visually that the mesh doesn't clip through.
//!
//! Test cases:
//! 1. Fragment placed exactly on floor (bottom at Y=3, floor top at Y=3)
//! 2. Fragment placed on stairs to test side collision alignment
//!
//! Run with: `cargo run --example p24_flipped_fragment_test`

use bevy::app::AppExit;
use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};
use studio_core::{
    build_world_meshes_cross_chunk, DeferredRenderingPlugin, FragmentDebugConfig, FragmentPhysics,
    FragmentSurfaceParticles, OrbitCamera, OrbitCameraPlugin, TerrainOccupancy, Voxel,
    VoxelFragment, VoxelFragmentPlugin, VoxelMaterial, VoxelMaterialPlugin, VoxelWorld,
};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Phase 24: Collision Alignment Test".into(),
                resolution: bevy::window::WindowResolution::new(1280, 720),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(VoxelMaterialPlugin)
        .add_plugins(DeferredRenderingPlugin)
        .add_plugins(OrbitCameraPlugin)
        .add_plugins(VoxelFragmentPlugin)
        .add_systems(Startup, setup)
        .add_systems(Update, (take_screenshot, log_positions))
        .insert_resource(TestState::default())
        .insert_resource(FragmentCollisionConfigOverride { enabled: false }) // DISABLE physics!
        .run();
}

/// Override to disable physics so fragments stay where we put them
#[derive(Resource)]
struct FragmentCollisionConfigOverride {
    enabled: bool,
}

#[derive(Resource, Default)]
struct TestState {
    frame: u32,
    screenshot_taken: bool,
}

#[derive(Component)]
struct TestFragment {
    name: &'static str,
    expected_bottom_y: f32,
}

#[derive(Resource)]
struct MaterialHandle(Handle<VoxelMaterial>);

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<VoxelMaterial>>,
    mut collision_config: ResMut<studio_core::FragmentCollisionConfig>,
) {
    // DISABLE physics so fragments stay exactly where we place them
    collision_config.enabled = false;

    // Create terrain with floor and stairs
    let mut terrain = VoxelWorld::new();
    let floor_color = Voxel::solid(80, 80, 90);
    let stair_color = Voxel::solid(90, 70, 70);

    // Floor at Y=0,1,2 (top surface at Y=3)
    for x in -10..10 {
        for z in -10..10 {
            for y in 0..3 {
                terrain.set_voxel(x, y, z, floor_color);
            }
        }
    }

    // Stairs: step up from Y=3 to Y=6
    // Step 1: Y=3 at X=2
    for x in 2..5 {
        for z in -3..3 {
            terrain.set_voxel(x, 3, z, stair_color);
        }
    }
    // Step 2: Y=3,4 at X=5
    for x in 5..8 {
        for z in -3..3 {
            terrain.set_voxel(x, 3, z, stair_color);
            terrain.set_voxel(x, 4, z, stair_color);
        }
    }
    // Step 3: Y=3,4,5 at X=8
    for x in 8..11 {
        for z in -3..3 {
            terrain.set_voxel(x, 3, z, stair_color);
            terrain.set_voxel(x, 4, z, stair_color);
            terrain.set_voxel(x, 5, z, stair_color);
        }
    }

    commands.insert_resource(TerrainOccupancy::from_voxel_world(&terrain));

    let material = materials.add(VoxelMaterial { ambient: 0.1 });
    commands.insert_resource(MaterialHandle(material.clone()));

    // Spawn terrain mesh
    let chunk_meshes = build_world_meshes_cross_chunk(&terrain);
    commands
        .spawn((
            Name::new("Terrain"),
            Transform::default(),
            Visibility::default(),
        ))
        .with_children(|parent| {
            for chunk_mesh in chunk_meshes {
                let translation = chunk_mesh.translation();
                let mesh_handle = meshes.add(chunk_mesh.mesh);
                parent.spawn((
                    Mesh3d(mesh_handle),
                    MeshMaterial3d(material.clone()),
                    Transform::from_translation(translation),
                ));
            }
        });

    // Enable debug gizmos
    commands.insert_resource(FragmentDebugConfig {
        show_particles: true,
        show_bounds: true,
        show_center: true,
        show_terrain: true,
        terrain_radius: 12.0,
        ..default()
    });

    // ========== TEST FRAGMENTS ==========
    // For a 3x3x3 fragment:
    // - Physics particles are at local positions -1, 0, +1 (centered at origin)
    // - Particle radius = 0.5
    // - So fragment bottom = center_y - 1 - 0.5 = center_y - 1.5
    // - To place bottom at Y=3: center_y = 3 + 1.5 = 4.5

    let floor_top = 3.0;
    let fragment_half_extent = 1.5; // 3x3x3 centered, particles at -1,0,+1 with radius 0.5

    // TEST 1: Fragment on floor - bottom should be exactly at Y=3
    let test1_center_y = floor_top + fragment_half_extent; // 4.5
    spawn_test_fragment(
        &mut commands,
        &mut meshes,
        material.clone(),
        Vec3::new(-5.0, test1_center_y, 0.0),
        Quat::IDENTITY,
        Color::srgb(1.0, 0.4, 0.4), // Red
        "FloorTest",
        floor_top,
    );

    // TEST 2: Fragment on first stair step - bottom at Y=4 (stair top)
    let stair1_top = 4.0;
    let test2_center_y = stair1_top + fragment_half_extent; // 5.5
    spawn_test_fragment(
        &mut commands,
        &mut meshes,
        material.clone(),
        Vec3::new(3.0, test2_center_y, 0.0),
        Quat::IDENTITY,
        Color::srgb(0.4, 1.0, 0.4), // Green
        "Stair1Test",
        stair1_top,
    );

    // TEST 3: Fragment NEXT TO stair (testing side collision)
    // Place fragment at X=1.5 (fragment extends from X=0 to X=3)
    // Stair starts at X=2, so fragment edge should touch but not penetrate
    let test3_center_y = floor_top + fragment_half_extent; // 4.5 (on floor)
    let test3_center_x = 2.0 - fragment_half_extent; // 0.5 (right edge at X=2)
    spawn_test_fragment(
        &mut commands,
        &mut meshes,
        material.clone(),
        Vec3::new(test3_center_x, test3_center_y, 0.0),
        Quat::IDENTITY,
        Color::srgb(0.4, 0.4, 1.0), // Blue
        "SideTest",
        floor_top,
    );

    // TEST 4: Flipped fragment on floor
    let test4_center_y = floor_top + fragment_half_extent;
    spawn_test_fragment(
        &mut commands,
        &mut meshes,
        material.clone(),
        Vec3::new(-5.0, test4_center_y, 5.0),
        Quat::from_rotation_x(std::f32::consts::PI), // Flipped 180
        Color::srgb(1.0, 0.4, 1.0),                  // Magenta
        "FlippedTest",
        floor_top,
    );

    // Camera
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 12.0, 20.0).looking_at(Vec3::new(2.0, 4.0, 0.0), Vec3::Y),
        OrbitCamera {
            target: Vec3::new(2.0, 4.0, 0.0),
            distance: 25.0,
            ..default()
        },
    ));

    // Light
    commands.spawn((
        DirectionalLight {
            illuminance: 15000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(10.0, 20.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    commands.insert_resource(AmbientLight {
        color: Color::WHITE,
        brightness: 200.0,
        ..default()
    });

    info!("========================================");
    info!("COLLISION ALIGNMENT TEST");
    info!("========================================");
    info!("Physics DISABLED - fragments at exact positions");
    info!("Floor top at Y=3");
    info!("Fragment half-extent: 1.5 (bottom = center - 1.5)");
    info!("");
    info!("RED: On floor, center Y=4.5, bottom Y=3");
    info!("GREEN: On stair1, center Y=5.5, bottom Y=4");
    info!("BLUE: Next to stair, right edge at X=2");
    info!("MAGENTA: Flipped on floor");
    info!("========================================");
    info!("If mesh clips through terrain, alignment is WRONG");
    info!("========================================");
}

fn spawn_test_fragment(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    material: Handle<VoxelMaterial>,
    position: Vec3,
    rotation: Quat,
    color: Color,
    name: &'static str,
    expected_bottom_y: f32,
) {
    // Create 3x3x3 voxel fragment
    let mut fragment_world = VoxelWorld::new();
    let r = (color.to_srgba().red * 255.0) as u8;
    let g = (color.to_srgba().green * 255.0) as u8;
    let b = (color.to_srgba().blue * 255.0) as u8;
    for x in 0..3 {
        for y in 0..3 {
            for z in 0..3 {
                fragment_world.set_voxel(x, y, z, Voxel::solid(r, g, b));
            }
        }
    }

    // Build mesh
    let chunk_meshes = build_world_meshes_cross_chunk(&fragment_world);
    if chunk_meshes.is_empty() {
        return;
    }

    let fragment = VoxelFragment::new(fragment_world.clone(), IVec3::ZERO);
    let size = fragment.occupancy.size;
    let max_dim = size.x.max(size.y).max(size.z);
    let surface_particles = FragmentSurfaceParticles::from_size(max_dim as u32, 27.0);

    // Physics particles are at -1, 0, +1 (for 3x3x3)
    // With particle radius 0.5, physics bounds are -1.5 to +1.5
    //
    // Mesh vertices: voxel x has corner at (x - 16)
    // For voxels 0,1,2: corners at -16, -15, -14
    // Each voxel is 1 unit, so mesh spans -16 to -13
    //
    // To align mesh with physics (both centered at origin):
    // Mesh min = -16, max = -13, center = -14.5
    // Physics min = -1.5, max = +1.5, center = 0
    //
    // We want mesh min at -1.5, so offset = -1.5 - (-16) = 14.5
    // OR: offset = physics_center - mesh_center = 0 - (-14.5) = 14.5
    let mesh_offset = Vec3::splat(14.5);

    info!(
        "  {} mesh_offset = {:?}, max_dim = {}",
        name, mesh_offset, max_dim
    );

    let entity = commands
        .spawn((
            Name::new(name),
            TestFragment {
                name,
                expected_bottom_y,
            },
            fragment,
            FragmentPhysics {
                velocity: Vec3::ZERO,
                angular_velocity: Vec3::ZERO,
                mass: 27.0,
            },
            surface_particles,
            Transform::from_translation(position).with_rotation(rotation),
            Visibility::default(),
        ))
        .with_children(|parent| {
            for chunk_mesh in chunk_meshes {
                let mesh_handle = meshes.add(chunk_mesh.mesh);
                parent.spawn((
                    Mesh3d(mesh_handle),
                    MeshMaterial3d(material.clone()),
                    Transform::from_translation(mesh_offset),
                ));
            }
        })
        .id();

    info!(
        "Spawned {} at center ({:.1}, {:.1}, {:.1}), expected bottom Y={:.1}",
        name, position.x, position.y, position.z, expected_bottom_y
    );
}

fn log_positions(
    mut state: ResMut<TestState>,
    fragments: Query<(&Name, &Transform, &TestFragment)>,
) {
    state.frame += 1;

    if state.frame == 60 {
        info!("");
        info!("=== POSITION CHECK (frame 60) ===");
        for (name, transform, test) in fragments.iter() {
            let center_y = transform.translation.y;
            let actual_bottom = center_y - 1.5;
            let error = actual_bottom - test.expected_bottom_y;
            info!(
                "{}: center Y={:.2}, bottom Y={:.2}, expected={:.2}, error={:.3}",
                name, center_y, actual_bottom, test.expected_bottom_y, error
            );
            if error.abs() > 0.01 {
                warn!("  ^ ERROR > 1cm!");
            }
        }
    }
}

#[allow(deprecated)]
fn take_screenshot(
    mut commands: Commands,
    mut state: ResMut<TestState>,
    mut app_exit: EventWriter<AppExit>,
) {
    if state.frame == 120 && !state.screenshot_taken {
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk("screenshots/p24_alignment_test.png"));
        state.screenshot_taken = true;
        info!("Screenshot saved to screenshots/p24_alignment_test.png");
    }

    if state.frame >= 180 {
        app_exit.write(AppExit::Success);
    }
}
