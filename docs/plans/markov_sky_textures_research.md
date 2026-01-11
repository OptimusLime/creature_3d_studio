# MarkovJunior Sky Texture Research

**Purpose:** Document proposed MJ model approaches for generating stylized sky textures.

**Referenced by:** `docs/plans/markov_cloud_sky_alternative.md`

---

## Sky Elements (in phase order)

We need to generate three types of sky textures:
1. **Clouds** (Phase 3-5) - Wispy/blobby cloud patterns with alpha
2. **Moons** (Phase 8-10) - Circular disc textures with surface detail  
3. **Stars** (Phase 11-13) - Sparse point field with varying brightness

---

## 1. Cloud Textures

**Goal:** Generate tileable cloud patterns with organic blob shapes and alpha transparency.

### Approach A: Cave-Style Cellular Automata (Recommended)

**Inspiration:** `Cave.xml`, `OpenCave.xml`, `ConnectedCaves.xml`

The cave models use convolution with Moore neighborhood to create organic blob shapes through cellular automata rules. The resulting patterns have:
- Connected regions with irregular edges
- Natural "cloud-like" blob shapes
- Controllable density via initial seed probability

```xml
<sequence values="BW">
  <!-- Initialize with random noise -->
  <prl in="B" out="W" p="0.45" steps="1"/>
  
  <!-- Cellular automata smoothing (Moore neighborhood) -->
  <!-- Rule: Cell becomes W if 5+ neighbors are W, else B -->
  <convolution neighborhood="Moore" periodic="True">
    <rule in="W" out="B" sum="0..4" values="W"/>
    <rule in="B" out="W" sum="5..8" values="W"/>
  </convolution>
</sequence>
```

**Why this works for clouds:**
- Creates organic, blobby shapes similar to cumulus clouds
- `periodic="True"` makes it tileable
- Density controlled by initial `p` value and CA threshold
- Multiple CA passes can smooth/refine edges

### Approach B: Voronoi-Style Growth

**Inspiration:** `Voronoi.xml`, `BiasedVoronoi.xml`

Sparse seeds that grow outward, competing for space:

```xml
<sequence values="BW">
  <!-- Sparse cloud nuclei -->
  <one in="B" out="W" steps="20"/>
  
  <!-- Grow outward -->
  <all in="WB" out="WW" steps="15"/>
</sequence>
```

**Pros:** Creates distinct cloud "puffs"
**Cons:** Less organic edges than CA approach

### Approach C: Partitioning with Contours

**Inspiration:** `BasicPartitioning.xml`, `CaveContour.xml`

Creates rectangular/boxy stylized clouds with clear edges:

```xml
<sequence values="BW">
  <all in="***/*B*/***" out="***/*W*/***"/>
  <markov>
    <one in="WWW/WBW" out="WBW/WBW"/>
    <one in="WWW/BBB" out="WBW/BBB"/>
  </markov>
</sequence>
```

**Why consider:** Creates a more stylized, geometric cloud look that could work well with bloom/glow post-processing.

---

## 2. Moon Textures

**Goal:** Generate circular moon disc textures with surface detail (craters, maria).

### Approach A: Masked Growth (Recommended)

Start with a circular mask, then grow detail patterns inside:

```xml
<sequence values="BWG">
  <!-- B=background, W=moon surface, G=crater/dark region -->
  
  <!-- First: create circular moon mask (done in code, not MJ) -->
  <!-- MJ operates only within the mask -->
  
  <!-- Add crater seeds -->
  <prl in="W" out="G" p="0.08" steps="1"/>
  
  <!-- Grow craters slightly -->
  <one in="GW" out="GG" steps="3"/>
</sequence>
```

**Implementation:** Generate circular mask in Rust code, run MJ only on pixels inside the circle, then composite.

### Approach B: Noise-Based Surface

**Inspiration:** `Noise.xml`, `StrangeNoise.xml`

Use competing growth to create varied surface regions:

```xml
<sequence values="WLG">
  <!-- W=bright surface, L=medium, G=dark maria -->
  <prl in="*/*" out="W/*" symmetry="(x)"/>
  <prl in="W" out="L" p="0.3" steps="1"/>
  <prl in="W" out="G" p="0.1" steps="1"/>
  <one>
    <rule in="LW" out="LL"/>
    <rule in="GW" out="GG"/>
  </one>
</sequence>
```

### Approach C: Pre-painted with MJ Enhancement

Create a base moon texture by hand or with simple gradients, then use MJ to add procedural detail/craters on top.

---

## 3. Star Field Textures

**Goal:** Generate sparse star points with varying brightness levels.

### Approach A: Blue Noise Distribution (Recommended)

**Inspiration:** `BlueNoise.xml`

Blue noise creates well-distributed points that don't clump:

```xml
<one values="BW" origin="True" temperature="0.3">
  <rule in="B" out="W"/>
  <field for="W" from="W" on="B" recompute="True"/>
</one>
```

**Post-processing:** After MJ generates star positions, vary brightness in the shader based on hash of position.

### Approach B: Simple Random Scatter

Very sparse random placement:

```xml
<prl values="BW" in="B" out="W" p="0.003" steps="1"/>
```

**Pros:** Simple, fast
**Cons:** May clump; no brightness variation from MJ

### Approach C: Multi-Layer Stars

Generate multiple star layers at different densities for depth:

```xml
<sequence values="BWYZ">
  <!-- W=bright stars (sparse) -->
  <prl in="B" out="W" p="0.001" steps="1"/>
  <!-- Y=medium stars -->
  <prl in="B" out="Y" p="0.003" steps="1"/>
  <!-- Z=dim stars (denser) -->
  <prl in="B" out="Z" p="0.01" steps="1"/>
</sequence>
```

Each layer rendered with different brightness in shader.

---

## Rendering Considerations

### Alpha Handling

For clouds and moons, we need alpha transparency:
- MJ outputs binary (0 or 1 per cell)
- Convert to alpha in `render_2d()` color palette
- Cloud edges can be softened with blur post-process or by running multiple CA passes

### Tileability

For clouds that scroll via UV flow:
- Use `periodic="True"` on convolution rules
- Verify seams by tiling 2x2 in test

### Size Recommendations

- **Clouds:** 256x256 or 512x512 (needs detail for scrolling)
- **Moons:** 128x128 (fixed size in sky, doesn't need to tile)
- **Stars:** 512x512 or 1024x1024 (sparse, but covers whole sky)

---

## Implementation Priority

Based on current phase plan:

1. **Clouds first** (Phase 3) - Use Cave-style CA approach
2. **Moons second** (Phase 8) - Masked growth for surface detail
3. **Stars last** (Phase 11) - Blue noise or simple scatter

---

## Next Steps

1. Implement Cave-style cloud model in `CloudTexture.xml`
2. Test output with `p33_mj_cloud_gen.rs`
3. Verify tileable by rendering 2x2 grid
4. If good, proceed to Phase 4 (texture loading into shader)
