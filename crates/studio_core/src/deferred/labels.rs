//! Render graph labels for the deferred rendering pipeline.

use bevy::render::render_graph::RenderLabel;

/// Labels for deferred rendering nodes in the render graph.
#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
pub enum DeferredLabel {
    /// G-Buffer pass: renders geometry to MRT (color, normal, position)
    GBufferPass,
    /// Lighting pass: fullscreen quad that computes lighting from G-buffer
    LightingPass,
    /// Bloom pass: post-process bloom effect on HDR output
    BloomPass,
}
