# Practical Real-Time Strategies for Accurate Indirect Occlusion

**Technical Memo ATVI-TR-19-01**

Jorge Jimenez, Xian-Chun Wu, Angelo Pesce, Adrian Jarabo

Activision Blizzard, Universidad de Zaragoza, I3A

---

## Abstract

In this work we introduce a set of techniques for real-time ambient occlusion targeted to very tight budgets. We propose GTAO, a new formulation of screen-space ambient occlusion that allows the composited occluded illumination to match the ground truth reference in half a millisecond on current console hardware. This is done by using a radiometrically-correct formulation of the ambient occlusion equation, and an efficient implementation that distributes computation using spatio-temporal sampling.

As opposed to previous methods, our technique incorporates the energy lost by missing interreflections by using an efficient, accurate physically-based parametric form, avoiding the use of ad-hoc approximations of indirect illumination. Then, we extend GTAO to account for directionally-resolved illumination, by fastly projecting coupled visibility and foreshortening factors into spherical harmonics, and thoroughly analyze with previous work. Finally, we introduce a novel model for specular occlusion formulation that accounts for the coupling between visibility and BRDF, closely matching the ground truth specular illumination from probe-based lighting, and propose GTSO, an efficient implementation of this concept based on tabulation. Our techniques are practical real-time, give results close to the ray-traced ground truth, and have been integrated in recent AAA console titles.

---

## 1. Introduction

Ambient occlusion (AO) is an approximation of global illumination, that models the diffuse shadows produced by close, potentially small occluders in a tight budget. It allows to preserve high-frequency details and contrast in low-frequency precomputed indirect illumination via pre-baked illumination or light probes. Unfortunately, solving the ambient occlusion integral is still too expensive to be practical in certain scenarios (e.g. 1080p or 4K rendering at 60 fps), so approximations have been developed in the past to achieve the target performance budget.

We introduce a new set of screen-space occlusion techniques, that target practical real-time performance while matching ray-traced ground truth solutions. We propose a novel technique for ambient occlusion, that we call ground truth-based ambient occlusion (GTAO), that decouples ambient occlusion from the near-range indirect illumination. This allows us to solve efficiently the ambient occlusion integral by avoiding piecewise integration (as required when using obscurance estimators), while recovering the lost multiple scattered diffuse lighting by using an efficient physically-based functional approximation. This allows to match not only ground truth occlusion, but also illumination references.

Then, we extend our ambient occlusion model to directionally-resolved illumination from distant probes, that uses our accurate ambient occlusion term and our from-horizons bent normal calculations to derive an efficient expansion in spherical harmonics, that can be used to efficiently integrate ambient illumination. Finally, we generalize ambient occlusion for arbitrary specular materials and formulate it by using a novel split-integral formulation that couples the BRDF with the visibility. We propose an efficient implementation of this formulation, that we call ground truth-based specular occlusion (GTSO), to compute it in runtime by accessing a small precomputed table.

### Contributions

* **GTAO**: An efficient ambient occlusion technique that matches a radiometrically-correct ambient occlusion integral, and incorporates the lost energy due to close-range indirect illumination using a simple closed-form analytical expression.
* **Directional GTAO**: an extension that accounts for directionally-resolved distant illumination, which includes a ground truth derivation of horizon-based bent normals.
* **Specular occlusion (SO)**: A generalization of the standard ambient occlusion formulation for arbitrary specular BRDFs that couples visibility and reflectance for efficiently computing specular reflection from distant probes.
* **GTSO**: An efficient implementation of this specular formulation for microfacets-based BRDFs.

---

## 2. Related Work

### Screen-Space Ambient Occlusion

Ambient occlusion integrates the visibility from a point in the scene to modulate the ambient illumination term. Mittring [Mit07] proposed to move the visibility queries to screen-space. Bavoil et al. [BSD08] proposed Horizon Based AO (HBAO) which performs line integrals based on horizon angles. Timonen [Tim13b] proposed a radiometrically-correct estimator for ambient obscurance. Our work efficiently computes radiometrically-correct ambient occlusion based on visibility horizons and accounts for indirect illumination via a physically-based parametric formula.

### Directional Occlusion

Landis [Lan02] proposed using bent normals to fetch from the ambient probe. Ramamoorthi and Hanrahan [RH01] proposed to encode light probes into spherical harmonics. We build on these ideas, proposing an efficient projection into SH of the coupled visibility and foreshortening factor in run-time.

### Specular Occlusion

Gotanda [Got12] and Lagarde [Ld14] derived empirical specular occlusion. In contrast, we formally derive a specular occlusion term analogous to ambient occlusion that couples visibility and specular BRDF.

---

## 3. Background & Overview

The reflected radiance $L_r(x, \omega_o)$ from a point $x$ with normal $n$ towards a direction $\omega_o$ can be modeled as:

$$L_r(x, \omega_o) = \int_{H^2} L(x, \omega_i) f_r(x, \omega_i, \omega_o) \langle n, \omega_i \rangle_+ d\omega_i$$

where $H^2$ is the hemisphere centered in $x$ and having $n$ as its axis, $L(x, \omega_i)$ is the incoming radiance, $f_r$ is the BRDF, and $\langle n, \omega_i \rangle_+$ models foreshortening.

Ambient occlusion approximates this by assuming surfaces are purely absorbing, light comes from a uniform environment, and the surface is Lambertian:

$$A(x) = \frac{1}{\pi} \int_{H^2} V(x, \omega_i) \langle n, \omega_i \rangle_+ d\omega_i$$

---

## 4. Directional GTAO

We extend the ambient occlusion model to directionally-resolved illumination. We project the terms of the integral as their spherical harmonics expansion:

$$L_r(x) \approx \sum_j \hat{L}_j \hat{V}'_j$$

where $\hat{L}_j$ and $\hat{V}'_j$ are the j-th term of the SH expansion of $L(\omega_i)$ and $V'(x, \omega_i)$ respectively, with $V'(x, \omega_i) = V(x, \omega_i)\langle n, \omega_i \rangle_+$.

Assuming visibility $V(x, \omega_i)$ can be approximated by a cone centered at the bent normal $b$ with aperture angle $\alpha_v$, we can project both visibility and the dot product in zonal harmonics.

### Computing the bent normal $b$

We compute $b$ using a radiometric formulation weighted by the cosine:

$$b = \frac{\int_{H^2} V(x, \omega_i) \langle n, \omega_i \rangle_+ \omega_i \, d\omega_i}{\left\| \int_{H^2} V(x, \omega_i) \langle n, \omega_i \rangle_+ \omega_i \, d\omega_i \right\|}$$

Using the horizon-based approximation, the inner integral over $\theta$ can be solved analytically for each component of $b$:

$$t_0(\phi) = \frac{6\sin(h_0-n) - \sin(3h_0-n) + 6\sin(h_1-n) - \sin(3h_1-n) + 16\sin(n) - 3(\sin(h_0+n) + \sin(h_1+n))}{12}$$

$$t_1(\phi) = \frac{-\cos(3h_0-n) - \cos(3h_1-n) + 8\cos(n) - 3(\cos(h_0+n) + \cos(h_1+n))}{12}$$

---

## Algorithm Implementation

### Pseudocode for Bent Normals Calculation

```
1: ...
2: for slice ∈ [0, sliceCount) do
3:    ...
      // Equations (20) and (21)
4:    t[0] ← (6*sin(h[0]-n) - sin(3*h[0]-n) + 6*sin(h[1]-n) - sin(3*h[1]-n) 
              + 16*sin(n) - 3*(sin(h[0]+n) + sin(h[1]+n))) / 12
5:    t[1] ← (-cos(3*h[0]-n) - cos(3*h[1]-n) + 8*cos(n) 
              - 3*(cos(h[0]+n) + cos(h[1]+n))) / 12
6:    bentNormalL ← {ω[0]*t[0], ω[1]*t[0], -t[1]}   // Flip z due to change of handedness
7:    bentNormalV ← bentNormalV + MULT(bentNormalL, ROTFROMTOMATRIX({0,0,-1}, viewV)) * LEN(projNormalV)
8: end for
9: bentNormalV ← NORMALIZE(bentNormalV)
```

### Pseudocode for Horizon Search

```
4:  for slice ∈ [0, sliceCount) do
5:      φ ← (π/sliceCount) * slice
6:      ω ← {cos(φ), sin(φ)}
        ...
18:     cHorizonCos ← -1
19:     for sample ∈ [0, directionSampleCount) do
            ...
24:         cHorizonCos ← MAX(cHorizonCos, DOT(sHorizonV, viewV))
25:     end for
27:     h[side] ← n + CLAMP((-1+2*side)*arccos(cHorizonCos) - n, -π/2, π/2)
28:     visibility ← visibility + LEN(projNormalV) * (cos(n) + 2*h[side]*sin(n) - cos(2*h[side]-n)) / 4
29: end for
30: visibility ← visibility / sliceCount
```

---

## 5.1 Results

We compare our method for directional occlusion against standard non-directional AO approximation, the bent normal approximation [Lan02], and the triple product approximation [Sny06]. Our results show that our Directional GTAO technique matches ground truth references more accurately than previous methods, especially for high-frequency illumination, while remaining efficient for real-time applications.

**Performance**: The technique renders high-quality occlusion matching the ground truth, with the baseline GTAO + GI rendering in just **0.5 ms on a PS4 at 1080p** (for a standard halfres occlusion buffer).

---

## Key Formulas Summary

### Visibility Integration (per slice)

$$\text{visibility} = \frac{\|n'\|}{4} \left( \cos(n) + 2h \sin(n) - \cos(2h - n) \right)$$

Where:
- $n$ = angle of projected normal in slice plane
- $h$ = horizon angle (clamped to hemisphere)
- $n'$ = projected normal onto slice plane

### Horizon Angle Computation

$$h_{side} = n + \text{clamp}\left( (-1 + 2 \cdot side) \cdot \arccos(h_{cos}) - n, -\frac{\pi}{2}, \frac{\pi}{2} \right)$$

### Final Visibility

$$A(x) = \frac{1}{\text{sliceCount}} \sum_{\text{slices}} \text{visibility}_{\text{slice}}$$

---

## References

- [Mit07] Mittring, M. "Finding next gen: CryEngine 2"
- [BSD08] Bavoil, L., Sainz, M., Dimitrov, R. "Image-space horizon-based ambient occlusion" (HBAO)
- [Tim13b] Timonen, V. "Screen-space far-field ambient obscurance"
- [Lan02] Landis, H. "Production-ready global illumination"
- [RH01] Ramamoorthi, R., Hanrahan, P. "An efficient representation for irradiance environment maps"
- [Got12] Gotanda, Y. "Practical implementation of physically-based shading models at tri-Ace"
- [Ld14] Lagarde, S. "Moving Frostbite to PBR"
