use bevy::prelude::*;
use studio_core::{
    GpuCollisionAABB, GpuCollisionContacts, GpuCollisionMode, TerrainOccupancy, Voxel,
    VoxelFragmentPlugin, VoxelWorld,
};

#[test]
fn test_gpu_collision_detection() {
    let mut app = App::new();

    // Use DefaultPlugins but disable Winit/Window to run on macOS test thread
    app.add_plugins(
        DefaultPlugins
            .build()
            .disable::<bevy::winit::WinitPlugin>()
            .disable::<bevy::window::WindowPlugin>()
            .set(bevy::log::LogPlugin {
                filter: "wgpu=error,bevy_render=error,studio_core=info".into(),
                level: bevy::log::Level::INFO,
                ..default()
            }),
    );

    app.add_plugins(VoxelFragmentPlugin);

    // Manually register events that RenderPlugin expects (since we disabled WindowPlugin)
    app.add_event::<bevy::window::WindowResized>();
    app.add_event::<bevy::window::WindowCreated>();
    app.add_event::<bevy::window::WindowCloseRequested>();
    app.add_event::<bevy::window::WindowFocused>();
    app.add_event::<bevy::window::WindowScaleFactorChanged>();
    app.add_event::<bevy::window::WindowBackendScaleFactorChanged>();
    app.add_event::<bevy::window::WindowOccluded>();

    // Setup Terrain
    let mut terrain = VoxelWorld::new();
    // Create a solid floor at y=0..3
    let floor_voxel = Voxel::solid(1, 1, 1);
    for x in -2..2 {
        for z in -2..2 {
            for y in 0..3 {
                terrain.set_voxel(x, y, z, floor_voxel);
            }
        }
    }

    app.insert_resource(TerrainOccupancy::from_voxel_world(&terrain));
    app.insert_resource(GpuCollisionMode { enabled: true });

    // Spawn a player/AABB that is colliding with the floor
    // Floor is y=0..3. Top is y=3.0.
    // Spawn at y=2.5. Half-extents (0.5, 0.5, 0.5).
    // Bottom is 2.0. Top is 3.0.
    // It is FULLY inside the floor voxel layer y=2.
    // It should generate contacts.
    app.add_systems(Startup, |mut commands: Commands| {
        commands.spawn((
            GpuCollisionAABB::new(Vec3::new(0.5, 0.5, 0.5)),
            Transform::from_translation(Vec3::new(0.0, 2.5, 0.0)),
            GlobalTransform::default(),
        ));
        info!("Spawned AABB at y=2.5 (Inside floor y=0..3)");
    });

    // Run for 20 frames to allow pipeline init and readback
    info!("Running simulation...");
    for i in 0..20 {
        app.update();

        // Check contacts
        if let Some(contacts) = app.world().get_resource::<GpuCollisionContacts>() {
            let result = contacts.get();
            info!("Frame {}: Contacts found: {}", i, result.contacts.len());

            if !result.contacts.is_empty() {
                // Inspect the contact
                let contact = result.contacts[0];
                info!(
                    "  Contact: Pos={:?}, Normal={:?}, Pen={}",
                    contact.position, contact.normal, contact.penetration
                );

                // Assert reasonable values
                // We are at 2.5. Floor ends at 3.0.
                // We are inside. Should push UP (Normal 0,1,0).
                if contact.normal[1] > 0.9 {
                    info!("SUCCESS: Found valid floor contact!");
                    return; // Test passed
                }
            }
        }

        // Sleep a bit to simulate frame time for async GPU/WGPU mapping?
        // Usually app.update() blocks on wgpu if configured, but let's just run loop.
        std::thread::sleep(std::time::Duration::from_millis(16));
    }

    panic!("FAILED: No valid GPU contacts generated after 20 frames.");
}
