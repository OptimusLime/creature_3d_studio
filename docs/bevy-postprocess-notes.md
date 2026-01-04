# Bevy Post-Processing Notes

Research notes for Phase 6 (Bloom) and future post-processing work.

## Bevy's Built-in Bloom

Bevy 0.17 includes a built-in `Bloom` component in `bevy::post_process::bloom` that implements a high-quality bloom effect similar to Bonsai's approach.

### Usage

```rust
use bevy::post_process::bloom::{Bloom, BloomCompositeMode, BloomPrefilter};
use bevy::render::view::Hdr;

commands.spawn((
    Camera3d::default(),
    Hdr,  // Required for bloom to work properly
    Bloom {
        intensity: 0.3,
        low_frequency_boost: 0.7,
        low_frequency_boost_curvature: 0.95,
        high_pass_frequency: 1.0,
        prefilter: BloomPrefilter {
            threshold: 1.0,
            threshold_softness: 0.5,
        },
        composite_mode: BloomCompositeMode::Additive,
        ..default()
    },
));
```

### Bloom Settings

| Setting | Description | Our Value |
|---------|-------------|-----------|
| `intensity` | Overall bloom strength (0.0-1.0) | 0.3 |
| `low_frequency_boost` | Emphasize larger glow halos (0.0-1.0) | 0.7 |
| `low_frequency_boost_curvature` | Shape of the boost curve | 0.95 |
| `high_pass_frequency` | Filter out small details (0.0-1.0) | 1.0 |
| `prefilter.threshold` | Brightness threshold for bloom | 1.0 |
| `prefilter.threshold_softness` | Soft edge around threshold | 0.5 |
| `composite_mode` | How bloom is blended | `Additive` |
| `scale` | Horizontal/vertical bloom scale | default |

### Presets

Bevy provides built-in presets:
- `Bloom::NATURAL` - Subtle, realistic bloom
- `Bloom::OLD_SCHOOL` - More aggressive, retro-style bloom
- Custom - Configure each parameter individually

### Composite Modes

- `BloomCompositeMode::EnergyConserving` - Total energy preserved, bloom replaces some base color
- `BloomCompositeMode::Additive` - Bloom is simply added on top (matches Bonsai's approach)

Bonsai reference (`composite.fragmentshader:209`):
```glsl
if (UseLightingBloom) { TotalLight += 0.05f*Bloom; }
```

## Comparison to Bonsai

Bevy's bloom uses the same COD-style mip-chain approach as Bonsai:

### Bonsai's Implementation

**Downsample** (`bloom_downsample.fragmentshader`):
- 13-tap filter pattern
- Takes samples at +/- 2 texels and +/- 1 texel offsets
- Energy-preserving weighted distribution: `0.125*5 + 0.03125*4 + 0.0625*4 = 1.0`

**Upsample** (`bloom_upsample.fragmentshader`):
- 9-tap 3x3 tent filter
- Weights: center=4, edges=2, corners=1, divided by 16
- FilterRadius parameter for variable blur size

### Bevy's Implementation

Bevy implements essentially the same algorithm:
1. Extract bright pixels above threshold
2. Downsample chain (typically 6 levels)
3. Upsample chain with tent filter
4. Composite onto final image

The main differences are configuration-level - Bevy exposes more tuning parameters through the component.

## Requirements

1. **HDR Camera**: Bloom requires HDR to work properly. Add the `Hdr` marker component to the camera.

2. **High Brightness Values**: Bloom only affects pixels above the threshold. Our emission multiplier (`EMISSION_MULTIPLIER = 2.0`) produces HDR values > 1.0 which trigger bloom.

3. **Tonemapping**: Use a tonemapper that handles HDR well. `Tonemapping::TonyMcMapface` is recommended.

## Future Work

### Custom Post-Processing

If we need more control than Bevy's built-in bloom provides, we can create custom post-process passes:

1. **FullscreenShader**: Use `bevy::core_pipeline::FullscreenShader` for custom screen-space effects
2. **Render Nodes**: Create custom render graph nodes for multi-pass effects
3. **View Targets**: Access render targets via `ViewTarget` in render systems

### Planned Effects (Phase 7-8)

- **Distance Fog** (Phase 7): Port from `Lighting.fragmentshader:306-319`
- **Tone Mapping** (Phase 8): AgX tone mapping from `composite.fragmentshader:49-97`

## References

- Bevy bloom example: `examples/3d/bloom_3d.rs`
- Bonsai bloom: `bonsai/shaders/bloom_downsample.fragmentshader`, `bloom_upsample.fragmentshader`
- COD presentation: SIGGRAPH 2014 - "Next Generation Post Processing in Call of Duty: Advanced Warfare"
