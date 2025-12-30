# XeGTAO Research & Analysis

## Overview
**XeGTAO** (Ground Truth Ambient Occlusion) is a screen-space ambient occlusion technique developed by Intel. It is an evolution of the GTAO (Ground Truth Ambient Occlusion) algorithm introduced by Activision.

**Key Paper:** "Practical Real-Time Strategies for Accurate Indirect Occlusion" (Jimenez et al.)

**License:** MIT License (Copyright (C) 2016-2021, Intel Corporation)

**Core Concept:** Unlike SSAO which samples a hemisphere around a point, GTAO works by searching for the "horizon" angle in multiple slices (directions) around the pixel. This provides a more physically accurate representation of occlusion and avoids many artifacts associated with traditional SSAO (like over-occlusion on flat surfaces or halos).

## Key Features
1.  **Horizon-Based Sampling:** Calculates the maximum elevation angle of occluders in several directions.
2.  **Physically Based:** Approximates the integral of the visibility function over the hemisphere.
3.  **Depth MIP-Mapping:** Uses a MIP chain of the depth buffer to sample efficiently at different radii, improving cache locality and performance for large radii.
4.  **Bent Normals:** Can optionally compute "bent normals" (the average unoccluded direction), which is useful for specular occlusion and GI.
5.  **Thickness Heuristic:** Includes a heuristic to handle thin objects (like grass or wires) to prevent them from occluding things far behind them incorrectly.
6.  **Spatial Denoising:** Includes an edge-aware spatial denoiser to clean up the noise resulting from low sample counts.

## Algorithm Pipeline

The XeGTAO implementation typically involves three passes:

### 1. Prefilter Depths (Pass 1)
*   **Goal:** Generate a MIP chain of viewspace linear depths.
*   **Input:** Raw Depth Buffer.
*   **Output:** 5 levels of MIP-mapped viewspace depth (Level 0 is the full res converted to viewspace linear Z).
*   **Details:** Uses a weighted average filter (`XeGTAO_DepthMIPFilter`) to preserve depth edges better than standard downsampling.

### 2. Main Pass (Pass 2)
*   **Goal:** Compute the AO term (and optional bent normal).
*   **Input:** Viewspace Depth MIPs, Normal Map (optional, can be generated from depth), Noise Texture.
*   **Output:** Noisy AO Term (Visibility), Edge Information (for denoiser).
*   **Logic:**
    *   Reconstructs Viewspace Position.
    *   Calculates/Loads Viewspace Normal.
    *   Iterates through `SliceCount` (directions) and `StepsPerSlice` (samples along the ray).
    *   For each direction, finds the maximum horizon angle by sampling the depth MIPs.
    *   Calculates the visibility integral based on these horizon angles.
    *   Applies "Thickness Heuristic" to mitigate occlusion from thin foreground objects.
    *   Outputs a noisy visibility value and "edges" (depth discontinuities) for the denoiser.

### 3. Denoise Pass (Pass 3)
*   **Goal:** Smooth the noisy AO result without blurring edges.
*   **Input:** Noisy AO Term, Edge Information.
*   **Output:** Final clean AO.
*   **Logic:**
    *   Performs a spatial blur.
    *   Uses the edge information generated in the main pass to prevent blurring across depth discontinuities.
    *   Often run in multiple passes (e.g., X and Y axis separately, or just one gather-based pass).

## Key File Analysis (Source/Rendering/Shaders/)

### `XeGTAO.h`
*   **Language:** C++ / HLSL Header.
*   **Purpose:** Shared definitions between CPU and GPU.
*   **Content:**
    *   `GTAOConstants` struct: Uniforms like ViewportSize, Projection info, EffectRadius, etc.
    *   `GTAOSettings` struct: User-facing settings (Radius, Quality Level, Denoise Passes).
    *   Constants: `XE_GTAO_DEPTH_MIP_LEVELS` (5), `XE_GTAO_NUMTHREADS_X/Y` (8).

### `vaGTAO.hlsl`
*   **Language:** HLSL.
*   **Purpose:** The entry point file for the demo engine's shaders.
*   **Content:**
    *   Defines Compute Shader entry points (`CSGTAOLow`, `CSGTAOHigh`, etc.).
    *   Handles resource binding (Textures, Samplers, Constant Buffers).
    *   Includes `XeGTAO.hlsli`.

### `XeGTAO.hlsli`
*   **Language:** HLSL.
*   **Purpose:** The core library containing the actual algorithm implementation.
*   **Key Functions:**
    *   `XeGTAO_PrefilterDepths16x16`: Implements the depth MIP generation.
    *   `XeGTAO_MainPass`: The heart of the algorithm. Implements the horizon search loop.
    *   `XeGTAO_Denoise`: The edge-aware denoiser.
    *   `XeGTAO_CalculateEdges`: Computes depth edges for the denoiser.
    *   `XeGTAO_ComputeViewspacePosition`: Helper to get Viewspace Pos from Screen Pos + Depth.

## Implementation Notes for Porting to WGSL/Bevy
*   **Coordinate System:** XeGTAO relies heavily on **Viewspace**. We need to ensure our depth unpacking and coordinate reconstruction match Bevy's view space conventions (Right-handed, Y-up? Check Bevy docs, usually -Z is forward).
*   **MIP Chain:** We will need to implement the Compute Shader dispatch to generate the depth MIPs. This is a prerequisite for the main pass.
*   **Uniforms:** We need to map `GTAOConstants` to a WGSL struct and populate it correctly from Bevy's camera and window data.
*   **Texture Formats:**
    *   Depth: Needs to be linearized viewspace Z (likely `R32Float` or `R16Float`).
    *   AO Output: `R8Uint` or `R8Unorm` is sufficient for visibility, but `R11G11B10Float` or `RGBA8` might be needed if bent normals are used.
*   **Denoiser:** The denoiser is crucial for quality. It relies on the "Edge" texture generated in the main pass.

