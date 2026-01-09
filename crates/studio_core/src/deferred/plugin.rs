//! Deferred rendering plugin for Bevy.
//!
//! This plugin sets up the full custom render graph for deferred rendering.

use bevy::core_pipeline::core_3d::graph::{Core3d, Node3d};
use bevy::prelude::*;
use bevy::render::{
    extract_component::ExtractComponentPlugin,
    extract_resource::ExtractResourcePlugin,
    render_graph::{RenderGraphExt, ViewNodeRunner},
    ExtractSchedule, Render, RenderApp, RenderSystems,
};

use super::bloom::{init_bloom_pipeline, prepare_bloom_textures, BloomConfig};
use super::bloom_node::BloomNode;
use super::collision_extract::{
    extract_fragments_system, extract_terrain_occupancy_system, ExtractedFragments,
    ExtractedTerrainChunks,
};
use super::collision_node::run_collision_compute_system;
use super::collision_prepare::{
    init_collision_pipeline, init_gpu_occupancy, prepare_collision_bind_groups,
    upload_terrain_to_gpu,
};
use super::collision_readback::GpuCollisionReadbackPlugin;
use super::extract::{
    extract_deferred_meshes, extract_moon_config, prepare_deferred_meshes, DeferredRenderable,
};
use super::gbuffer::DeferredCamera;
use super::gbuffer_geometry::{init_gbuffer_geometry_pipeline, update_gbuffer_mesh_bind_group};
use super::gbuffer_node::GBufferPassNode;
use super::gtao::{prepare_gtao_textures, GtaoConfig};
use super::gtao_denoise::{
    init_gtao_denoise_pipeline, prepare_gtao_denoised_textures, GtaoDenoiseNode,
};
use super::gtao_depth_prefilter::{
    init_depth_prefilter_pipeline, prepare_depth_mip_textures, DepthPrefilterNode,
};
use super::gtao_node::{
    init_gtao_noise_texture, init_gtao_pipeline, update_gtao_frame_count, GtaoFrameCount,
    GtaoPassNode,
};
use super::labels::DeferredLabel;
use super::lighting::DeferredLightingConfig;
use super::lighting_node::{init_lighting_pipeline, LightingPassNode};
use super::point_light::{extract_point_lights, prepare_point_lights};
use super::point_light_shadow::{
    init_point_shadow_pipeline, prepare_point_shadow_bind_groups, prepare_point_shadow_textures,
    prepare_shadow_casting_lights,
};
use super::point_light_shadow_node::PointShadowPassNode;
use super::prepare::{prepare_gbuffer_textures, prepare_gbuffer_view_uniforms};
use super::shadow::{init_shadow_pipeline, prepare_directional_shadow_textures, MoonConfig};
use super::shadow_node::{
    prepare_directional_shadow_uniforms, prepare_shadow_mesh_bind_groups, Moon1ShadowPassNode,
    Moon2ShadowPassNode,
};
use super::sky_dome::SkyDomeConfig;
use super::sky_dome_node::{init_sky_dome_pipeline, SkyDomeNode};
use crate::debug_screenshot::DebugModes;

/// Plugin that enables deferred rendering for voxels.
///
/// This sets up a full custom render graph with:
/// - G-Buffer pass (MRT rendering to color/normal/position)
/// - Lighting pass (fullscreen quad with deferred lighting)
///
/// ## Usage
///
/// ```rust,ignore
/// app.add_plugins(DeferredRenderingPlugin);
///
/// // Mark cameras for deferred rendering
/// commands.spawn((
///     Camera3d::default(),
///     DeferredCamera,
/// ));
/// ```
pub struct DeferredRenderingPlugin;

impl Plugin for DeferredRenderingPlugin {
    fn build(&self, app: &mut App) {
        // Main app resources
        app.init_resource::<DeferredLightingConfig>();
        app.init_resource::<BloomConfig>();
        app.init_resource::<MoonConfig>();
        app.init_resource::<GtaoConfig>();
        app.init_resource::<DebugModes>();
        app.init_resource::<SkyDomeConfig>();

        // Extract DeferredCamera and DeferredRenderable components to render world
        app.add_plugins(ExtractComponentPlugin::<DeferredCamera>::default());
        app.add_plugins(ExtractComponentPlugin::<DeferredRenderable>::default());

        // Extract GtaoConfig resource to render world (for hot-reloading parameters)
        app.add_plugins(ExtractResourcePlugin::<GtaoConfig>::default());

        // Extract DebugModes resource to render world (for runtime debug mode switching)
        app.add_plugins(ExtractResourcePlugin::<DebugModes>::default());

        // Extract BloomConfig resource to render world (for enabling/disabling bloom)
        app.add_plugins(ExtractResourcePlugin::<BloomConfig>::default());

        // Extract SkyDomeConfig resource to render world (for enabling/disabling sky dome)
        app.add_plugins(ExtractResourcePlugin::<SkyDomeConfig>::default());

        // Add GPU collision readback plugin (creates shared resource in both worlds)
        app.add_plugins(GpuCollisionReadbackPlugin);

        // Get render app
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            warn!("RenderApp not found - deferred rendering disabled");
            return;
        };

        // Initialize moon config in render world
        render_app.init_resource::<MoonConfig>();

        // Initialize GTAO frame counter for TAA noise index (XeGTAO.h L196)
        render_app.init_resource::<GtaoFrameCount>();

        // Initialize GPU collision extraction resources in render world
        render_app.init_resource::<ExtractedFragments>();
        render_app.init_resource::<ExtractedTerrainChunks>();

        // Add extraction systems for deferred meshes, point lights, moon config, and collision
        render_app.add_systems(
            ExtractSchedule,
            (
                extract_deferred_meshes,
                extract_point_lights,
                extract_moon_config,
                update_gtao_frame_count,
                extract_fragments_system,
                extract_terrain_occupancy_system,
            ),
        );

        // Add prepare systems
        // - init pipelines runs first to create pipeline resources
        // - prepare_gbuffer_textures creates the G-buffer textures
        // - prepare_gbuffer_view_uniforms extracts camera transforms to view uniforms
        // - prepare_deferred_meshes collects extracted meshes for rendering
        // - update_gbuffer_mesh_bind_group creates mesh bind group for fallback test cube
        // - prepare_bloom_textures creates bloom mip chain textures
        // - prepare_shadow_textures creates shadow map depth texture
        // - prepare_shadow_view_uniforms creates light-space matrices
        // - prepare_point_shadow_* systems for point light cube shadow maps
        // Pipeline initialization systems
        render_app.add_systems(
            Render,
            (
                init_gbuffer_geometry_pipeline.in_set(RenderSystems::Prepare),
                init_lighting_pipeline.in_set(RenderSystems::Prepare),
                init_bloom_pipeline.in_set(RenderSystems::Prepare),
                init_shadow_pipeline.in_set(RenderSystems::Prepare),
                init_point_shadow_pipeline.in_set(RenderSystems::Prepare),
                init_gtao_pipeline.in_set(RenderSystems::Prepare),
                init_gtao_noise_texture.in_set(RenderSystems::Prepare),
                init_depth_prefilter_pipeline.in_set(RenderSystems::Prepare),
                init_gtao_denoise_pipeline.in_set(RenderSystems::Prepare),
                init_sky_dome_pipeline.in_set(RenderSystems::Prepare),
            ),
        );

        // G-buffer and mesh prepare systems
        render_app.add_systems(
            Render,
            (
                prepare_gbuffer_textures.in_set(RenderSystems::PrepareResources),
                prepare_gbuffer_view_uniforms
                    .in_set(RenderSystems::PrepareResources)
                    .after(init_gbuffer_geometry_pipeline),
                prepare_deferred_meshes
                    .in_set(RenderSystems::PrepareResources)
                    .after(init_gbuffer_geometry_pipeline),
                update_gbuffer_mesh_bind_group
                    .in_set(RenderSystems::PrepareResources)
                    .after(init_gbuffer_geometry_pipeline),
                prepare_bloom_textures
                    .in_set(RenderSystems::PrepareResources)
                    .after(init_bloom_pipeline),
            ),
        );

        // Shadow prepare systems
        render_app.add_systems(
            Render,
            (
                prepare_directional_shadow_textures
                    .in_set(RenderSystems::PrepareResources)
                    .after(init_shadow_pipeline),
                prepare_directional_shadow_uniforms
                    .in_set(RenderSystems::PrepareResources)
                    .after(init_shadow_pipeline),
                prepare_shadow_mesh_bind_groups
                    .in_set(RenderSystems::PrepareResources)
                    .after(init_shadow_pipeline)
                    .after(prepare_deferred_meshes),
                prepare_point_lights.in_set(RenderSystems::PrepareResources),
            ),
        );

        // Point shadow and SSAO prepare systems
        render_app.add_systems(
            Render,
            (
                prepare_point_shadow_textures
                    .in_set(RenderSystems::PrepareResources)
                    .after(init_point_shadow_pipeline),
                prepare_shadow_casting_lights
                    .in_set(RenderSystems::PrepareResources)
                    .after(prepare_point_lights),
                prepare_point_shadow_bind_groups
                    .in_set(RenderSystems::PrepareResources)
                    .after(init_point_shadow_pipeline)
                    .after(prepare_shadow_casting_lights)
                    .after(prepare_deferred_meshes),
                prepare_gtao_textures
                    .in_set(RenderSystems::PrepareResources)
                    .after(init_gtao_pipeline),
                prepare_depth_mip_textures
                    .in_set(RenderSystems::PrepareResources)
                    .after(init_depth_prefilter_pipeline),
                prepare_gtao_denoised_textures
                    .in_set(RenderSystems::PrepareResources)
                    .after(init_gtao_denoise_pipeline),
            ),
        );

        // GPU collision systems
        // init_gpu_occupancy creates the GPU texture arrays for terrain
        // init_collision_pipeline creates the compute pipeline
        // upload_terrain_to_gpu uploads terrain chunks when dirty
        // prepare_collision_bind_groups prepares per-frame fragment data
        // run_collision_compute_system runs the compute shader and reads back results
        render_app.add_systems(
            Render,
            (
                init_gpu_occupancy.in_set(RenderSystems::Prepare),
                init_collision_pipeline
                    .in_set(RenderSystems::Prepare)
                    .after(init_gpu_occupancy),
                upload_terrain_to_gpu
                    .in_set(RenderSystems::PrepareResources)
                    .after(init_gpu_occupancy),
                prepare_collision_bind_groups
                    .in_set(RenderSystems::PrepareResources)
                    .after(init_collision_pipeline)
                    .after(upload_terrain_to_gpu),
                run_collision_compute_system
                    .in_set(RenderSystems::Render)
                    .after(prepare_collision_bind_groups),
            ),
        );

        // Add render graph nodes
        render_app
            // Moon 1 shadow pass (purple moon)
            .add_render_graph_node::<ViewNodeRunner<Moon1ShadowPassNode>>(
                Core3d,
                DeferredLabel::Moon1ShadowPass,
            )
            // Moon 2 shadow pass (orange moon)
            .add_render_graph_node::<ViewNodeRunner<Moon2ShadowPassNode>>(
                Core3d,
                DeferredLabel::Moon2ShadowPass,
            )
            // Point light shadow pass node (cube shadow maps)
            .add_render_graph_node::<ViewNodeRunner<PointShadowPassNode>>(
                Core3d,
                DeferredLabel::PointShadowPass,
            )
            // G-Buffer pass node
            .add_render_graph_node::<ViewNodeRunner<GBufferPassNode>>(
                Core3d,
                DeferredLabel::GBufferPass,
            )
            // GTAO depth prefilter (generates depth MIP chain)
            .add_render_graph_node::<ViewNodeRunner<DepthPrefilterNode>>(
                Core3d,
                DeferredLabel::GtaoDepthPrefilter,
            )
            // GTAO pass node (after G-buffer and depth prefilter, before lighting)
            .add_render_graph_node::<ViewNodeRunner<GtaoPassNode>>(Core3d, DeferredLabel::GtaoPass)
            // GTAO denoise node (after main GTAO pass)
            .add_render_graph_node::<ViewNodeRunner<GtaoDenoiseNode>>(
                Core3d,
                DeferredLabel::GtaoDenoise,
            )
            // Lighting pass node
            .add_render_graph_node::<ViewNodeRunner<LightingPassNode>>(
                Core3d,
                DeferredLabel::LightingPass,
            )
            // Bloom pass node
            .add_render_graph_node::<ViewNodeRunner<BloomNode>>(Core3d, DeferredLabel::BloomPass)
            // Sky dome pass node (after bloom, before transparent)
            .add_render_graph_node::<ViewNodeRunner<SkyDomeNode>>(
                Core3d,
                DeferredLabel::SkyDomePass,
            );

        // Define render graph edges (execution order)
        // Shadow passes run first, then G-buffer, then connect to MainOpaquePass
        // Order: Start -> Moon1 -> Moon2 -> Point Shadow -> GBuffer -> MainOpaque
        render_app.add_render_graph_edges(
            Core3d,
            (
                Node3d::StartMainPass,
                DeferredLabel::Moon1ShadowPass, // Purple moon shadow
                DeferredLabel::Moon2ShadowPass, // Orange moon shadow
                DeferredLabel::PointShadowPass, // Point light cube shadow
                DeferredLabel::GBufferPass,
                Node3d::MainOpaquePass,
            ),
        );

        // GTAO runs after G-buffer: first depth prefilter, then main GTAO pass, then denoise
        // Then Lighting runs after opaque, bloom after lighting, then transparent
        render_app.add_render_graph_edges(
            Core3d,
            (
                DeferredLabel::GBufferPass,
                DeferredLabel::GtaoDepthPrefilter,
                DeferredLabel::GtaoPass,
                DeferredLabel::GtaoDenoise,
                Node3d::MainOpaquePass,
            ),
        );

        render_app.add_render_graph_edges(
            Core3d,
            (
                Node3d::MainOpaquePass,
                DeferredLabel::LightingPass,
                DeferredLabel::BloomPass,
                DeferredLabel::SkyDomePass,
                Node3d::MainTransparentPass,
            ),
        );
    }
}
