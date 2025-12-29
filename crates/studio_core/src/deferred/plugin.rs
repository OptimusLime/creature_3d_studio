//! Deferred rendering plugin for Bevy.
//!
//! This plugin sets up the full custom render graph for deferred rendering.

use bevy::core_pipeline::core_3d::graph::{Core3d, Node3d};
use bevy::prelude::*;
use bevy::render::{
    extract_component::ExtractComponentPlugin,
    render_graph::{RenderGraphExt, ViewNodeRunner},
    Render, RenderApp, RenderSystems,
};

use super::gbuffer::DeferredCamera;
use super::gbuffer_node::GBufferPassNode;
use super::labels::DeferredLabel;
use super::lighting::DeferredLightingConfig;
use super::lighting_node::{init_lighting_pipeline, LightingPassNode};
use super::prepare::prepare_gbuffer_textures;

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

        // Extract DeferredCamera component to render world
        app.add_plugins(ExtractComponentPlugin::<DeferredCamera>::default());

        // Get render app
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            warn!("RenderApp not found - deferred rendering disabled");
            return;
        };

        // Add prepare systems - init_lighting_pipeline runs first to create the resource
        // then prepare_gbuffer_textures creates the G-buffer textures
        render_app.add_systems(
            Render,
            (
                init_lighting_pipeline.in_set(RenderSystems::Prepare),
                prepare_gbuffer_textures.in_set(RenderSystems::PrepareResources),
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
