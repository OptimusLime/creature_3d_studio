//! Deferred rendering plugin for Bevy.
//!
//! This plugin sets up the full custom render graph for deferred rendering.

use bevy::core_pipeline::core_3d::graph::{Core3d, Node3d};
use bevy::prelude::*;
use bevy::render::{
    extract_component::ExtractComponentPlugin,
    render_graph::{RenderGraphExt, ViewNodeRunner},
    ExtractSchedule, Render, RenderApp, RenderSystems,
};

use super::extract::{extract_deferred_meshes, prepare_deferred_meshes, DeferredRenderable};
use super::gbuffer::DeferredCamera;
use super::gbuffer_geometry::{init_gbuffer_geometry_pipeline, update_gbuffer_mesh_bind_group};
use super::gbuffer_node::GBufferPassNode;
use super::labels::DeferredLabel;
use super::lighting::DeferredLightingConfig;
use super::lighting_node::{init_lighting_pipeline, LightingPassNode};
use super::prepare::{prepare_gbuffer_textures, prepare_gbuffer_view_uniforms};

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

        // Extract DeferredCamera and DeferredRenderable components to render world
        app.add_plugins(ExtractComponentPlugin::<DeferredCamera>::default());
        app.add_plugins(ExtractComponentPlugin::<DeferredRenderable>::default());

        // Get render app
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            warn!("RenderApp not found - deferred rendering disabled");
            return;
        };

        // Add extraction system for deferred meshes
        render_app.add_systems(ExtractSchedule, extract_deferred_meshes);

        // Add prepare systems
        // - init pipelines runs first to create pipeline resources
        // - prepare_gbuffer_textures creates the G-buffer textures
        // - prepare_gbuffer_view_uniforms extracts camera transforms to view uniforms
        // - prepare_deferred_meshes collects extracted meshes for rendering
        // - update_gbuffer_mesh_bind_group creates mesh bind group for fallback test cube
        render_app.add_systems(
            Render,
            (
                init_gbuffer_geometry_pipeline.in_set(RenderSystems::Prepare),
                init_lighting_pipeline.in_set(RenderSystems::Prepare),
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
            ),
        );

        // Add render graph nodes
        render_app
            // G-Buffer pass node
            .add_render_graph_node::<ViewNodeRunner<GBufferPassNode>>(
                Core3d,
                DeferredLabel::GBufferPass,
            )
            // Lighting pass node
            .add_render_graph_node::<ViewNodeRunner<LightingPassNode>>(
                Core3d,
                DeferredLabel::LightingPass,
            );

        // Define render graph edges (execution order)
        // For now: Run AFTER main opaque so we can composite/see our output
        // Later we'll move to before and render geometry ourselves
        render_app.add_render_graph_edges(
            Core3d,
            (
                Node3d::StartMainPass,
                DeferredLabel::GBufferPass,
            ),
        );
        
        // Lighting pass runs after everything, writes final output
        render_app.add_render_graph_edges(
            Core3d,
            (
                Node3d::MainOpaquePass,
                DeferredLabel::LightingPass,
                Node3d::MainTransparentPass,
            ),
        );

        info!("DeferredRenderingPlugin initialized with custom render graph");
    }
}
