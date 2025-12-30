# GTAO Debug Plan - Main Pass Audit

**Created:** 2024-12-30
**Status:** Ready to Execute
**Problem:** GTAO output shows excessive noise despite all implementation phases complete

---

## Problem Statement

The GTAO implementation is architecturally complete (Phases 1-6 done), but visual output shows **excessive noise**. The denoiser has been audited and verified correct (items 46-54). The main GTAO pass (items 1-45) has **NOT been audited** line-by-line against XeGTAO.hlsli.

**Root cause hypothesis:** One or more calculations in the main pass differ from XeGTAO reference, causing incorrect visibility values that manifest as noise.

---

## Conceptual Framework (from GTAO Paper)

The GTAO algorithm computes radiometrically-correct ambient occlusion:

### Core Algorithm

1. **Slice-based horizon search** - For each pixel, trace rays in N directions (slices)
2. **Per-slice processing:**
   - Project surface normal onto slice plane
   - Search for horizon angles in positive and negative directions
   - Integrate visibility using the analytic formula
3. **Accumulation** - Average visibility across all slices

### Key Formula (from paper)

Per-slice visibility integration:
```
visibility = (projNormalLen / 4) * (cos(n) + 2h*sin(n) - cos(2h - n))
```

Where:
- `n` = angle of projected normal in slice plane
- `h` = horizon angle (clamped to hemisphere)
- `projNormalLen` = length of normal projected onto slice plane

### XeGTAO Enhancements

- **Depth MIP chain** - Efficient multi-scale sampling
- **Spatio-temporal noise** - Hilbert curve + R2 sequence for TAA
- **Edge-aware denoising** - Clean up noise while preserving depth edges
- **Thin occluder compensation** - Handle thin geometry correctly
- **FP16 precision adjustment** - `viewspaceZ *= 0.99920`

---

## Debug Phases (SMART Tasks)

### Phase 0: Visual Diagnosis

**Outcome:** Identify which GTAO layer is broken based on debug screenshots

| ID | Task | Specific Action | Measurable Result | Verification |
|----|------|-----------------|-------------------|--------------|
| 0.1 | Capture screenshots | Run `cargo run --example p20_gtao_test` | 8 PNG files in `screenshots/gtao_test/` | Files exist |
| 0.2 | Check depth layer | Examine `gtao_depth.png` (mode 11) | Smooth gradient, near=dark, far=bright | No noise/banding |
| 0.3 | Check normal layer | Examine `gtao_normal.png` (mode 20) | Camera-facing=bright, sides=gray | Correct orientation |
| 0.4 | Check radius layer | Examine `gtao_radius.png` (mode 30) | Near=bright, far=dark | Inverse depth relationship |
| 0.5 | Check edge layer | Examine `gtao_edges.png` (mode 40) | Discontinuities=dark, smooth=bright | Edge detection working |
| 0.6 | Check final AO | Examine `ao_only.png` | Flat=white, corners=dark | No patchy noise |
| 0.7 | Document findings | Write diagnosis with broken layer(s) | Hypothesis for root cause | Written in this doc |

**Exit Criteria:** Broken layer identified, hypothesis documented

---

### Phase 1: Line-by-Line Audit

**Outcome:** All 45 checklist items verified against XeGTAO.hlsli L281-558

**Reference file:** `XeGTAO/Source/Rendering/Shaders/XeGTAO.hlsli`
**Our file:** `assets/shaders/gtao.wgsl`

#### Group A: Setup & Precision (Items 1-5)

| ID | XeGTAO Line | Component | What to Verify | Status |
|----|-------------|-----------|----------------|--------|
| 1 | L281-284 | viewspaceZ precision | `viewspaceZ *= 0.99920` for FP16 | |
| 2 | L315 | falloffMul | `falloffMul = -1.0 / falloffRange` | |
| 3 | L316 | falloffAdd | `falloffAdd = 1.0 - falloffMul * effectRadius` | |
| 4 | L335 | pixelTooCloseThreshold | Value = 1.3 | |
| 5 | L342-343 | Small radius fade | Fade when radius < minRadius | |

#### Group B: Slice Setup (Items 6-18)

| ID | XeGTAO Line | Component | What to Verify | Status |
|----|-------------|-----------|----------------|--------|
| 6 | L367 | minS | `minS = pixelTooCloseThreshold / screenspaceRadius` | |
| 7 | L372 | sliceK with noise | `sliceK = (slice + noise) / sliceCount` | |
| 8 | L374 | phi | `phi = sliceK * PI` | |
| 9 | L377 | omega | `omega = (cos(phi), -sin(phi))` | |
| 10 | L383 | directionVec | `directionVec.xy = omega` | |
| 11 | L386 | orthoDirectionVec | `orthoDirectionVec = directionVec - dot(directionVec, viewVec) * viewVec` | |
| 12 | L390 | axisVec | `axisVec = cross(directionVec, viewVec)` | |
| 13 | L396 | projectedNormalVec | `projectedNormalVec = normal - dot(normal, axisVec) * axisVec` | |
| 14 | L399 | signNorm | `signNorm = sign(dot(orthoDirectionVec, projectedNormalVec))` | |
| 15 | L402 | projectedNormalVecLength | `length(projectedNormalVec)` | |
| 16 | L403 | cosNorm | `cosNorm = saturate(dot(projectedNormalVec, viewVec) / projectedNormalVecLength)` | |
| 17 | L406 | n angle | `n = signNorm * FastACos(cosNorm)` | |
| 18 | L409-414 | lowHorizonCos, horizonCos init | Both sides initialized correctly | |

#### Group C: Step Loop (Items 19-29)

| ID | XeGTAO Line | Component | What to Verify | Status |
|----|-------------|-----------|----------------|--------|
| 19 | L420 | R1 stepBaseNoise | Second noise value from R2 sequence | |
| 20 | L421 | stepNoise | `stepNoise = frac(stepBaseNoise + step * 0.6180339887)` | |
| 21 | L424 | s calculation | `s = (step + stepNoise) / stepsPerSlice` | |
| 22 | L427 | s power | `s = pow(s, sampleDistributionPower)` | |
| 23 | L430 | s += minS | Add minimum offset | |
| 24 | L433 | sampleOffset | `sampleOffset = s * screenspaceRadius * omega` | |
| 25 | L438 | MIP level | `mipLevel = clamp(log2(length(sampleOffset)) - depthMIPOffset, 0, 5)` | |
| 26 | L442 | Snap to pixel | Round to nearest pixel center | |
| 27 | L458-460 | Sample positive | `samplePos = pixelCenter + sampleOffset` | |
| 28 | L462-464 | Sample negative | `sampleNeg = pixelCenter - sampleOffset` | |
| 29 | L466-469 | sampleDelta, sampleDist | Delta from center, distance calculation | |

#### Group D: Horizon Update (Items 30-37)

| ID | XeGTAO Line | Component | What to Verify | Status |
|----|-------------|-----------|----------------|--------|
| 30 | L472-473 | sampleHorizonVec | `horizonVec = normalize(sampleDelta)` | |
| 31 | L477-478 | Falloff (no thin occ) | `weight = saturate(sampleDist * falloffMul + falloffAdd)` | |
| 32 | L481-484 | Thin occluder falloff | Modified falloff with thin occluder compensation | |
| 33 | L488-489 | shc calculation | `shc = dot(horizonVec, viewVec)` | |
| 34 | L492-493 | shc lerp | `shc = lerp(lowHorizonCos, shc, weight)` | |
| 35 | L505-506 | horizonCos update | `horizonCos = max(horizonCos, shc)` | |

#### Group E: Visibility Integration (Items 36-45)

| ID | XeGTAO Line | Component | What to Verify | Status |
|----|-------------|-----------|----------------|--------|
| 36 | L532 | projNormalLen fudge | `projectedNormalVecLength = max(0.05, projectedNormalVecLength)` | |
| 37 | L536 | h0 angle | `h0 = -FastACos(horizonCos1)` | |
| 38 | L537 | h1 angle | `h1 = FastACos(horizonCos0)` | |
| 39 | L540 | h0 clamp | `h0 = n + clamp(h0 - n, -PI/2, PI/2)` | |
| 40 | L541 | h1 clamp | `h1 = n + clamp(h1 - n, -PI/2, PI/2)` | |
| 41 | L542-543 | iarc formula | `iarc = (cosNorm + 2*h*sinN - cos(2h-n)) / 4` | |
| 42 | L544 | localVisibility | `localVisibility = projectedNormalVecLength * iarc` | |
| 43 | L556 | visibility / sliceCount | Divide by slice count | |
| 44 | L557 | visibility pow | `visibility = pow(visibility, finalValuePower)` | |
| 45 | L558 | visibility clamp | `visibility = max(0.03, visibility)` | |

**Exit Criteria:** All 45 items marked with status, all discrepancies documented

---

### Phase 2: Fix Identified Defects

**Outcome:** All audit defects corrected to match XeGTAO exactly

For each defect found in Phase 1:

| ID | Defect Description | XeGTAO Reference | Fix Applied | Verified |
|----|-------------------|------------------|-------------|----------|
| 2.1 | (to be filled) | | | |
| 2.2 | (to be filled) | | | |
| ... | | | | |

**Exit Criteria:** 
- All defects fixed
- `cargo build` succeeds
- Code matches XeGTAO line-by-line

---

### Phase 3: Verification & Quality Gates

**Outcome:** GTAO output is production quality

| ID | Task | Measurable Result | Pass/Fail |
|----|------|-------------------|-----------|
| 3.1 | Re-capture screenshots | `cargo run --example p20_gtao_test` | |
| 3.2 | Check flat surface AO | Value > 0.95 (near white) | |
| 3.3 | Check corner AO | Value 0.3-0.6 (visible darkening) | |
| 3.4 | Check for noise | No patchy/splotchy artifacts | |
| 3.5 | Check for banding | No visible banding | |
| 3.6 | Production quality | "Would you ship this?" = YES | |

**Exit Criteria:** All quality gates pass

---

## Diagnostic Screenshots Reference

| File | GTAO Mode | Lighting Mode | Shows |
|------|-----------|---------------|-------|
| `render.png` | 0 | 0 | Final lit scene with GTAO |
| `ao_only.png` | 0 | 5 | Raw GTAO output |
| `gtao_depth.png` | 11 | 5 | Linear viewspace depth |
| `gtao_normal.png` | 20 | 5 | View-space normal.z |
| `gtao_edges.png` | 40 | 5 | Packed edges |
| `gtao_radius.png` | 30 | 5 | Screenspace radius |
| `gbuffer_normals.png` | 0 | 1 | World-space normals |
| `gbuffer_depth.png` | 0 | 2 | G-buffer depth |

---

## Quick Commands

```bash
# Capture debug screenshots
cargo run --example p20_gtao_test

# View screenshots
open screenshots/gtao_test/

# Build check
cargo build

# Reference file locations
# XeGTAO: XeGTAO/Source/Rendering/Shaders/XeGTAO.hlsli
# Ours:   assets/shaders/gtao.wgsl
```

---

## Process Reminders

From `docs/HOW_WE_WORK.md`:

1. **Hypothesis-driven debugging** - Observe, hypothesize, test, analyze
2. **Verify each layer** before proceeding to next
3. **Never abandon** because it's hard
4. **Never substitute** simpler approaches
5. **Be honest** about defects - no wishful thinking
6. **Quality gates** - "Does it work?" is not the same as "Is it good enough?"
