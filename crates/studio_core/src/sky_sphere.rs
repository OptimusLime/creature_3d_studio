//! Sky Sphere - Option A implementation
//!
//! A large sphere mesh with custom material for sky rendering.
//! The sphere follows the camera position (not rotation), so clouds stay fixed in world space.
//!
//! Usage:
//! ```ignore
//! app.add_plugins(SkySpherePlugin);
//! ```

use bevy::{
    mesh::MeshVertexBufferLayoutRef,
    pbr::{Material, MaterialPipeline, MaterialPipelineKey},
    prelude::*,
    render::render_resource::{
        AsBindGroup, CompareFunction, Face, RenderPipelineDescriptor, ShaderType,
        SpecializedMeshPipelineError,
    },
    shader::ShaderRef,
};

/// Plugin that adds sky sphere rendering.
pub struct SkySpherePlugin;

impl Plugin for SkySpherePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<SkySphereMaterial>::default())
            .init_resource::<SkySphereConfig>()
            .add_systems(Startup, setup_sky_sphere)
            .add_systems(PostUpdate, sky_follow_camera);
    }
}

/// Configuration for the sky sphere.
#[derive(Resource)]
pub struct SkySphereConfig {
    /// Radius of the sky sphere
    pub radius: f32,
    /// Path to cloud texture
    pub cloud_texture_path: Option<String>,
    /// Time of day (0.0 = midnight, 0.5 = noon)
    pub time_of_day: f32,
    /// Whether sky sphere is enabled
    pub enabled: bool,
}

impl Default for SkySphereConfig {
    fn default() -> Self {
        Self {
            radius: 900.0,
            cloud_texture_path: Some("textures/generated/mj_clouds_001.png".to_string()),
            time_of_day: 0.35,
            enabled: true,
        }
    }
}

/// Marker component for the sky sphere entity.
#[derive(Component)]
pub struct SkySphere;

/// Custom material for the sky sphere.
#[derive(Asset, AsBindGroup, Clone, TypePath)]
pub struct SkySphereMaterial {
    #[texture(0)]
    #[sampler(1)]
    pub cloud_texture: Handle<Image>,

    #[uniform(2)]
    pub uniforms: SkyUniforms,
}

/// Uniforms passed to the sky shader.
#[derive(Clone, Copy, Default, ShaderType)]
pub struct SkyUniforms {
    /// Time of day (0.0 = midnight, 0.5 = noon)
    pub time_of_day: f32,
    /// Cloud opacity (0.0 - 1.0)
    pub cloud_opacity: f32,
    /// Padding for alignment
    pub _padding: Vec2,
}

impl Material for SkySphereMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/sky_sphere_material.wgsl".into()
    }

    fn vertex_shader() -> ShaderRef {
        "shaders/sky_sphere_material.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Opaque
    }

    fn specialize(
        _pipeline: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        layout: &MeshVertexBufferLayoutRef,
        _key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        // Use standard mesh attributes: position, normal, uv
        let vertex_layout = layout.0.get_layout(&[
            Mesh::ATTRIBUTE_POSITION.at_shader_location(0),
            Mesh::ATTRIBUTE_NORMAL.at_shader_location(1),
            Mesh::ATTRIBUTE_UV_0.at_shader_location(2),
        ])?;
        descriptor.vertex.buffers = vec![vertex_layout];

        // Render inside faces (we're inside the sphere looking out)
        descriptor.primitive.cull_mode = Some(Face::Front);

        // Sky renders at max depth - only draws where nothing else has been drawn
        // Use GreaterEqual so sky only renders where depth buffer is at max (1.0 = far plane)
        if let Some(ref mut depth_stencil) = descriptor.depth_stencil {
            depth_stencil.depth_write_enabled = false;
            depth_stencil.depth_compare = CompareFunction::GreaterEqual;
        }

        Ok(())
    }
}

/// System to spawn the sky sphere at startup.
fn setup_sky_sphere(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<SkySphereMaterial>>,
    asset_server: Res<AssetServer>,
    config: Res<SkySphereConfig>,
) {
    if !config.enabled {
        return;
    }

    // Load cloud texture (use default white texture if not specified)
    let cloud_texture = match &config.cloud_texture_path {
        Some(path) => asset_server.load::<Image>(path),
        None => asset_server.load::<Image>("textures/white.png"),
    };

    // Create sky material
    let material = materials.add(SkySphereMaterial {
        cloud_texture,
        uniforms: SkyUniforms {
            time_of_day: config.time_of_day,
            cloud_opacity: 0.8,
            _padding: Vec2::ZERO,
        },
    });

    // Create sphere mesh - UV sphere with good resolution
    let mesh = meshes.add(
        Sphere::new(config.radius).mesh().uv(64, 32), // 64 horizontal, 32 vertical segments
    );

    // Spawn sky sphere at origin (will follow camera)
    commands.spawn((
        Mesh3d(mesh),
        MeshMaterial3d(material),
        Transform::from_translation(Vec3::ZERO),
        SkySphere,
    ));

    info!("Sky sphere spawned with radius {}", config.radius);
}

/// System to make sky sphere follow camera position (not rotation).
fn sky_follow_camera(
    camera_query: Query<&Transform, With<Camera3d>>,
    mut sky_query: Query<&mut Transform, (With<SkySphere>, Without<Camera3d>)>,
) {
    let Ok(camera_transform) = camera_query.single() else {
        return;
    };

    for mut sky_transform in &mut sky_query {
        // Only copy position, not rotation
        sky_transform.translation = camera_transform.translation;
    }
}

/// System to update sky material uniforms (call this if time_of_day changes).
pub fn update_sky_uniforms(
    config: Res<SkySphereConfig>,
    mut materials: ResMut<Assets<SkySphereMaterial>>,
    sky_query: Query<&MeshMaterial3d<SkySphereMaterial>, With<SkySphere>>,
) {
    for material_handle in &sky_query {
        if let Some(material) = materials.get_mut(&material_handle.0) {
            material.uniforms.time_of_day = config.time_of_day;
        }
    }
}
