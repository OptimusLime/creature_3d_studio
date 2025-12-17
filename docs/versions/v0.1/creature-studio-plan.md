# v0.1 Creature Studio - Dark Fantasy 80s Voxel Glow Aesthetic

## Vision

A cozy creature creation studio set in a perpetual twilight world. Players sculpt glowing voxel creatures that come alive and serve purposes in a hostile, fog-shrouded landscape lit only by mana glow and six colored moons.

**Aesthetic**: Tron meets Dark Crystal meets Hollow Knight. 80s synth palette - magenta, cyan, purple, orange glow on black void.

---

## Target Demo: "The Awakening"

Minimal slice proving the core creation fantasy:

1. Dark scene with voxel placement
2. Glow/emission shader system
3. Creature "awakens" (pulse animation, eyes light up)
4. Creature moves via Lua behavior script

**Success criteria**: A 15-second clip that feels magical - placing the last voxel, watching it come alive, seeing it move with purpose.

---

## Exploration Phase (Current)

Before detailed planning, we need to understand our voxel rendering options.

### Bonsai Repository Analysis

Goals:
- Understand SVO (Sparse Voxel Octree) architecture
- Evaluate shader approach (HLSL/GLSL)
- Assess greedy meshing implementation
- Identify reusable components vs. inspiration-only
- Understand performance characteristics

### Key Questions

1. **Rendering**: How does Bonsai handle voxel rendering? Ray marching? Mesh generation?
2. **Data structures**: What's the octree/SVO setup? Memory layout?
3. **Shaders**: What's the shader architecture? Can we adapt for Bevy/wgpu?
4. **Emission/Glow**: Any existing emission support, or do we add our own?
5. **Integration path**: Port directly? Rewrite inspired-by? Use as reference only?

---

## Rough Phase Outline (Subject to Change Post-Exploration)

### Phase 1: Voxel Foundation
- [ ] Analyze Bonsai architecture
- [ ] Decide: port vs. rewrite vs. reference
- [ ] Basic voxel data structure in Rust
- [ ] Simple voxel rendering (cubes or SVO)
- [ ] Camera controls for studio view

### Phase 2: Glow Aesthetic
- [ ] Emission property on voxels
- [ ] Bloom post-processing
- [ ] Dark background / void aesthetic
- [ ] Basic fog system
- [ ] Color palette (80s dark fantasy)

### Phase 3: Creature Construction
- [ ] Voxel placement/removal in studio mode
- [ ] Creature as voxel collection
- [ ] Save/load creature definitions
- [ ] "Awakening" animation system (glow pulse, eyes)

### Phase 4: Creature Behavior
- [ ] Lua script attached to creature
- [ ] Basic movement behaviors
- [ ] Hot-reload behavior scripts
- [ ] MCP agent integration (voice → Lua generation)

### Phase 5: World Integration
- [ ] Deploy creature from studio to world
- [ ] Basic mana system (glow = mana consumption)
- [ ] Heat map concept (glow attracts)
- [ ] Fog + distant silhouettes

---

## Aesthetic Reference

### Color Palette
- **Void**: Pure black (#000000)
- **Ambient**: Deep purple (#1a0a2e)
- **Mana glow**: Cyan (#00ffff), Magenta (#ff00ff), Orange (#ff6600)
- **Moon colors**: Red, Blue, Green, Purple, White, Gold

### Visual Pillars
| Element | Treatment |
|---------|-----------|
| Light source | Only mana glows - no sun, no ambient |
| Creatures | Bioluminescent veins, glowing cores/eyes |
| Fog | Thick, colored by moonlight, hides silhouettes |
| Studio | Warm amber safe space, cozy |
| World | Cold, dark, threatening |

---

## TikTok Demo Targets

### Demo 1: "It woke up"
- Sculpt small creature
- Place final voxel
- Glow pulses through like heartbeat
- Eyes flicker open
- It moves

### Demo 2: "It came back"
- Creature ventures into fog
- Returns with glowing mana
- Player's reserves brighten

### Demo 3: "Something followed it"
- Creature returning
- Massive silhouette behind it
- Tension, dread, scale

---

## Technical Stack

| Layer | Technology |
|-------|------------|
| Engine | Bevy 0.17 |
| Physics | Rapier3D |
| Scripting | mlua (Lua 5.4) + hot reload |
| UI | ImGui (bevy_mod_imgui) |
| Voxels | TBD (Bonsai analysis pending) |
| Shaders | wgpu/WGSL (Bevy native) |
| AI Integration | MCP/ACP agent server |

---

## Open Questions

1. Bonsai: Port, adapt, or reference-only?
2. Voxel rendering: SVO ray marching vs. greedy mesh vs. instanced cubes?
3. Glow: Per-voxel emission vs. creature-level aura?
4. Fog: Volumetric vs. distance-based?
5. Performance target: How many glowing voxels on screen?

---

## Next Steps

1. **Explore Bonsai** - Deep dive into architecture
2. **Spike: Basic glow** - Can we get one glowing cube in Bevy?
3. **Refine plan** - Update phases based on findings
4. **Build Phase 1** - Voxel foundation
