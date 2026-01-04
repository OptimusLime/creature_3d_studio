//! Render graph labels for the deferred rendering pipeline.

use bevy::render::render_graph::RenderLabel;

/// Labels for deferred rendering nodes in the render graph.
#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
pub enum DeferredLabel {
    /// Moon 1 (purple) shadow pass: directional shadow for first moon
    Moon1ShadowPass,
    /// Moon 2 (orange) shadow pass: directional shadow for second moon
    Moon2ShadowPass,
    /// Point light shadow pass: renders scene to cube shadow maps for point lights
    PointShadowPass,
    /// G-Buffer pass: renders geometry to MRT (color, normal, position)
    GBufferPass,
    /// GTAO depth prefilter: generates 5-level depth MIP pyramid for GTAO
    GtaoDepthPrefilter,
    /// GTAO pass: computes ground-truth ambient occlusion from G-buffer
    GtaoPass,
    /// GTAO denoise: XeGTAO edge-aware spatial denoiser
    GtaoDenoise,
    /// Lighting pass: fullscreen quad that computes lighting from G-buffer
    LightingPass,
    /// Bloom pass: post-process bloom effect on HDR output
    BloomPass,
    /// GPU collision compute: runs voxel collision on GPU
    CollisionCompute,
}
