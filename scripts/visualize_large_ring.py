#!/usr/bin/env python3
"""
Visualize large-radius polar grids where cells are nearly uniform.

This script creates SVGs showing:
1. A large-radius ring (r_min=128) with tuned theta divisions
2. The same ring with neighbor highlighting
3. A 3x3 neighborhood pattern (the REAL neighbor structure)

Key insight: At large radii, cells are nearly rectangular, and we CAN
have a 3x3 neighborhood - it's just that the "diagonal" neighbors
require TWO hops (one in θ, one in r).
"""

import math
from pathlib import Path


def polar_to_cartesian(r: float, theta: float) -> tuple[float, float]:
    x = r * math.cos(theta)
    y = r * math.sin(theta)
    return x, y


def create_wedge_path(
    r_inner: float, r_outer: float, theta_start: float, theta_end: float,
    cx: float, cy: float, scale: float
) -> str:
    inner_start = polar_to_cartesian(r_inner, theta_start)
    inner_end = polar_to_cartesian(r_inner, theta_end)
    outer_start = polar_to_cartesian(r_outer, theta_start)
    outer_end = polar_to_cartesian(r_outer, theta_end)
    
    def transform(p):
        return (cx + p[0] * scale, cy - p[1] * scale)
    
    p1 = transform(inner_start)
    p2 = transform(inner_end)
    p3 = transform(outer_end)
    p4 = transform(outer_start)
    
    r_inner_svg = r_inner * scale
    r_outer_svg = r_outer * scale
    
    arc_angle = theta_end - theta_start
    large_arc = 1 if arc_angle > math.pi else 0
    
    path = (
        f"M {p1[0]:.2f} {p1[1]:.2f} "
        f"A {r_inner_svg:.2f} {r_inner_svg:.2f} 0 {large_arc} 0 {p2[0]:.2f} {p2[1]:.2f} "
        f"L {p3[0]:.2f} {p3[1]:.2f} "
        f"A {r_outer_svg:.2f} {r_outer_svg:.2f} 0 {large_arc} 1 {p4[0]:.2f} {p4[1]:.2f} "
        f"Z"
    )
    return path


def calculate_theta_divisions(r: float, target_arc: float) -> int:
    """Calculate theta divisions to achieve approximately square cells."""
    circumference = 2 * math.pi * r
    divisions = int(circumference / target_arc)
    return max(6, divisions)


def create_large_ring_svg(
    r_min: int, 
    r_depth: int, 
    theta_divisions: int,
    show_indices: bool = True,
    zoom_section: tuple[float, float] | None = None,  # (theta_start, theta_end) in radians
) -> str:
    """
    Create an SVG of a large-radius polar ring.
    
    Colors cells by theta index to show angular consistency.
    """
    r_max = r_min + r_depth
    
    # For large radii, we need to zoom in to see detail
    if zoom_section:
        theta_start_view, theta_end_view = zoom_section
    else:
        # Default: show a 45-degree section
        theta_start_view = 0
        theta_end_view = math.pi / 4
    
    # Calculate SVG dimensions for the zoomed section
    # We'll transform coordinates to show just this section
    scale = 4.0  # pixels per unit radius
    margin = 80
    
    # Calculate the visible area in Cartesian coordinates
    # For a ring section from theta_start to theta_end
    corners = []
    for r in [r_min, r_max]:
        for theta in [theta_start_view, theta_end_view]:
            corners.append(polar_to_cartesian(r, theta))
    
    xs = [c[0] for c in corners]
    ys = [c[1] for c in corners]
    
    # Add some points along the arcs
    for r in [r_min, r_max]:
        for t in range(10):
            theta = theta_start_view + (theta_end_view - theta_start_view) * t / 9
            corners.append(polar_to_cartesian(r, theta))
    
    xs = [c[0] for c in corners]
    ys = [c[1] for c in corners]
    
    x_min, x_max = min(xs), max(xs)
    y_min, y_max = min(ys), max(ys)
    
    width = (x_max - x_min) * scale + 2 * margin
    height = (y_max - y_min) * scale + 2 * margin
    
    # Center offset
    cx = margin - x_min * scale
    cy = height - margin + y_min * scale  # Flip y
    
    theta_step = 2 * math.pi / theta_divisions
    
    svg_parts = [
        '<?xml version="1.0" encoding="UTF-8"?>',
        f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {width:.0f} {height:.0f}" '
        f'width="{width:.0f}" height="{height:.0f}">',
        f'  <title>Large Ring: r_min={r_min}, r_depth={r_depth}, θ_div={theta_divisions}</title>',
        '',
        '  <rect width="100%" height="100%" fill="#fff"/>',
        '',
        '  <!-- Grid cells -->',
        '  <g id="cells">',
    ]
    
    # Draw cells in the visible section
    for r_idx in range(r_depth):
        r_inner = r_min + r_idx
        r_outer = r_min + r_idx + 1
        
        for theta_idx in range(theta_divisions):
            theta_start = theta_idx * theta_step
            theta_end = (theta_idx + 1) * theta_step
            
            # Skip cells outside the view
            if theta_end < theta_start_view or theta_start > theta_end_view:
                continue
            
            # Clip to view
            theta_start_clip = max(theta_start, theta_start_view)
            theta_end_clip = min(theta_end, theta_end_view)
            
            path = create_wedge_path(r_inner, r_outer, theta_start_clip, theta_end_clip, cx, cy, scale)
            
            # Color by theta index (hue varies with theta)
            hue = (theta_idx / theta_divisions) * 360
            color = f"hsl({hue:.0f}, 70%, 65%)"
            
            flat_idx = theta_idx + r_idx * theta_divisions
            
            svg_parts.append(
                f'    <path d="{path}" fill="{color}" stroke="#333" stroke-width="1" '
                f'data-idx="{flat_idx}" data-r="{r_idx}" data-theta="{theta_idx}"/>'
            )
    
    svg_parts.append('  </g>')
    
    # Add cell labels if requested and cells are big enough
    if show_indices:
        svg_parts.append('')
        svg_parts.append('  <!-- Cell labels -->')
        svg_parts.append('  <g font-family="monospace" font-size="8" text-anchor="middle">')
        
        for r_idx in range(r_depth):
            r_center = r_min + r_idx + 0.5
            
            for theta_idx in range(theta_divisions):
                theta_start = theta_idx * theta_step
                theta_end = (theta_idx + 1) * theta_step
                
                if theta_end < theta_start_view or theta_start > theta_end_view:
                    continue
                
                theta_center = (theta_start + theta_end) / 2
                if theta_center < theta_start_view or theta_center > theta_end_view:
                    continue
                
                lx, ly = polar_to_cartesian(r_center, theta_center)
                lx = cx + lx * scale
                ly = cy - ly * scale
                
                flat_idx = theta_idx + r_idx * theta_divisions
                
                svg_parts.append(
                    f'    <text x="{lx:.1f}" y="{ly:.1f}" fill="#000">[{flat_idx}]</text>'
                )
                svg_parts.append(
                    f'    <text x="{lx:.1f}" y="{ly + 10:.1f}" fill="#333" font-size="7">r{r_idx}θ{theta_idx}</text>'
                )
        
        svg_parts.append('  </g>')
    
    # Add info
    svg_parts.append('')
    svg_parts.append('  <!-- Info -->')
    svg_parts.append(f'  <g transform="translate(10, 20)">')
    svg_parts.append(f'    <text font-family="sans-serif" font-size="14" font-weight="bold">Large Radius Ring</text>')
    svg_parts.append(f'    <text font-family="monospace" font-size="11" y="18">r_min={r_min}, r_depth={r_depth}, θ_div={theta_divisions}</text>')
    
    # Calculate and show arc lengths
    inner_arc = (r_min + 0.5) * theta_step
    outer_arc = (r_min + r_depth - 0.5) * theta_step
    ratio = outer_arc / inner_arc
    
    svg_parts.append(f'    <text font-family="monospace" font-size="10" y="34">Inner ring arc: {inner_arc:.2f}</text>')
    svg_parts.append(f'    <text font-family="monospace" font-size="10" y="46">Outer ring arc: {outer_arc:.2f}</text>')
    svg_parts.append(f'    <text font-family="monospace" font-size="10" y="58">Ratio: {ratio:.3f}x (ideal: 1.0)</text>')
    svg_parts.append(f'    <text font-family="monospace" font-size="10" y="70">Radial thickness: 1.0</text>')
    svg_parts.append('  </g>')
    
    svg_parts.append('')
    svg_parts.append('</svg>')
    
    return '\n'.join(svg_parts)


def create_large_ring_with_neighbors_svg(
    r_min: int,
    r_depth: int, 
    theta_divisions: int,
    highlight_r: int,
    highlight_theta: int,
    show_3x3: bool = True,  # Show full 3x3 neighborhood including diagonals
) -> str:
    """
    Create an SVG showing a cell and its FULL neighborhood (including diagonals).
    
    The key insight: we're not limited to 4 neighbors. We can have 8 neighbors
    just like a Cartesian grid:
    
        θ-,r+  |  θ,r+  |  θ+,r+
        -------+--------+-------
        θ-,r   | CENTER |  θ+,r
        -------+--------+-------
        θ-,r-  |  θ,r-  |  θ+,r-
    
    The "diagonal" neighbors are just cells that differ in BOTH θ and r.
    """
    r_max = r_min + r_depth
    
    # Zoom to show the highlighted cell and its neighborhood
    theta_step = 2 * math.pi / theta_divisions
    
    # Calculate view bounds (show 5 cells in each direction)
    view_theta_start = max(0, (highlight_theta - 3) * theta_step)
    view_theta_end = min(2 * math.pi, (highlight_theta + 4) * theta_step)
    
    scale = 8.0
    margin = 100
    
    # Calculate visible area
    r_view_min = max(r_min, r_min + highlight_r - 2)
    r_view_max = min(r_max, r_min + highlight_r + 3)
    
    corners = []
    for r in [r_view_min, r_view_max]:
        for theta in [view_theta_start, view_theta_end]:
            corners.append(polar_to_cartesian(r, theta))
        for t in range(10):
            theta = view_theta_start + (view_theta_end - view_theta_start) * t / 9
            corners.append(polar_to_cartesian(r, theta))
    
    xs = [c[0] for c in corners]
    ys = [c[1] for c in corners]
    x_min, x_max = min(xs), max(xs)
    y_min, y_max = min(ys), max(ys)
    
    width = (x_max - x_min) * scale + 2 * margin
    height = (y_max - y_min) * scale + 2 * margin + 200  # Extra space for legend
    
    cx = margin - x_min * scale
    cy = height - margin - 200 + y_min * scale
    
    # Calculate all 8 neighbors (plus center = 9 cells in 3x3)
    neighbors = {}
    
    # Direct neighbors (4)
    theta_minus = (highlight_theta - 1 + theta_divisions) % theta_divisions
    theta_plus = (highlight_theta + 1) % theta_divisions
    
    neighbors['θ-'] = (highlight_r, theta_minus)
    neighbors['θ+'] = (highlight_r, theta_plus)
    
    if highlight_r > 0:
        neighbors['r-'] = (highlight_r - 1, highlight_theta)
    if highlight_r < r_depth - 1:
        neighbors['r+'] = (highlight_r + 1, highlight_theta)
    
    # Diagonal neighbors (4) - if show_3x3
    if show_3x3:
        if highlight_r > 0:
            neighbors['θ-,r-'] = (highlight_r - 1, theta_minus)
            neighbors['θ+,r-'] = (highlight_r - 1, theta_plus)
        if highlight_r < r_depth - 1:
            neighbors['θ-,r+'] = (highlight_r + 1, theta_minus)
            neighbors['θ+,r+'] = (highlight_r + 1, theta_plus)
    
    # Colors for each neighbor type
    neighbor_colors = {
        'θ-': '#4dabf7',      # Blue
        'θ+': '#69db7c',      # Green
        'r-': '#ffd43b',      # Yellow
        'r+': '#da77f2',      # Purple
        'θ-,r-': '#ff8787',   # Light red (diagonal)
        'θ+,r-': '#ffa94d',   # Orange (diagonal)
        'θ-,r+': '#63e6be',   # Teal (diagonal)
        'θ+,r+': '#b197fc',   # Light purple (diagonal)
    }
    
    svg_parts = [
        '<?xml version="1.0" encoding="UTF-8"?>',
        f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {width:.0f} {height:.0f}" '
        f'width="{width:.0f}" height="{height:.0f}">',
        f'  <title>3x3 Neighborhood at r={highlight_r}, θ={highlight_theta}</title>',
        '',
        '  <rect width="100%" height="100%" fill="#fff"/>',
        '',
        '  <!-- All cells (dimmed) -->',
        '  <g id="cells-dimmed" opacity="0.2">',
    ]
    
    # Draw visible cells dimmed
    for r_idx in range(r_depth):
        r_inner = r_min + r_idx
        r_outer = r_min + r_idx + 1
        
        if r_outer < r_view_min or r_inner > r_view_max:
            continue
        
        for theta_idx in range(theta_divisions):
            theta_start = theta_idx * theta_step
            theta_end = (theta_idx + 1) * theta_step
            
            if theta_end < view_theta_start or theta_start > view_theta_end:
                continue
            
            path = create_wedge_path(r_inner, r_outer, theta_start, theta_end, cx, cy, scale)
            svg_parts.append(f'    <path d="{path}" fill="#ccc" stroke="#999" stroke-width="1"/>')
    
    svg_parts.append('  </g>')
    
    # Draw neighbor cells
    svg_parts.append('')
    svg_parts.append('  <!-- Neighbor cells -->')
    
    for name, (nr, nt) in neighbors.items():
        if nr < 0 or nr >= r_depth:
            continue
        
        r_inner = r_min + nr
        r_outer = r_min + nr + 1
        theta_start = nt * theta_step
        theta_end = (nt + 1) * theta_step
        
        path = create_wedge_path(r_inner, r_outer, theta_start, theta_end, cx, cy, scale)
        color = neighbor_colors.get(name, '#aaa')
        
        # Diagonals get dashed border
        stroke_dash = 'stroke-dasharray="4,2"' if ',' in name else ''
        
        svg_parts.append(
            f'  <path d="{path}" fill="{color}" stroke="#333" stroke-width="2" {stroke_dash}/>'
        )
    
    # Draw center cell last (on top)
    svg_parts.append('')
    svg_parts.append('  <!-- Center cell -->')
    r_inner = r_min + highlight_r
    r_outer = r_min + highlight_r + 1
    theta_start = highlight_theta * theta_step
    theta_end = (highlight_theta + 1) * theta_step
    path = create_wedge_path(r_inner, r_outer, theta_start, theta_end, cx, cy, scale)
    svg_parts.append(f'  <path d="{path}" fill="#ff6b6b" stroke="#c92a2a" stroke-width="3"/>')
    
    # Add labels on cells
    svg_parts.append('')
    svg_parts.append('  <!-- Cell labels -->')
    svg_parts.append('  <g font-family="monospace" font-size="10" font-weight="bold" text-anchor="middle">')
    
    # Center label
    r_center = r_min + highlight_r + 0.5
    theta_center = (highlight_theta + 0.5) * theta_step
    lx, ly = polar_to_cartesian(r_center, theta_center)
    lx = cx + lx * scale
    ly = cy - ly * scale
    center_idx = highlight_theta + highlight_r * theta_divisions
    svg_parts.append(f'  <text x="{lx:.1f}" y="{ly - 5:.1f}" fill="#fff">CENTER</text>')
    svg_parts.append(f'  <text x="{lx:.1f}" y="{ly + 8:.1f}" fill="#fff" font-size="9">[{center_idx}]</text>')
    
    # Neighbor labels
    for name, (nr, nt) in neighbors.items():
        if nr < 0 or nr >= r_depth:
            continue
        
        r_center = r_min + nr + 0.5
        theta_center = (nt + 0.5) * theta_step
        lx, ly = polar_to_cartesian(r_center, theta_center)
        lx = cx + lx * scale
        ly = cy - ly * scale
        
        nidx = nt + nr * theta_divisions
        svg_parts.append(f'  <text x="{lx:.1f}" y="{ly - 3:.1f}" fill="#000" font-size="9">{name}</text>')
        svg_parts.append(f'  <text x="{lx:.1f}" y="{ly + 8:.1f}" fill="#333" font-size="8">[{nidx}]</text>')
    
    svg_parts.append('  </g>')
    
    # Add legend
    legend_y = height - 180
    svg_parts.append('')
    svg_parts.append('  <!-- Legend -->')
    svg_parts.append(f'  <g transform="translate(20, {legend_y})">')
    svg_parts.append(f'    <text font-family="sans-serif" font-size="14" font-weight="bold">3×3 Neighborhood Pattern</text>')
    svg_parts.append(f'    <text font-family="monospace" font-size="11" y="18">Center: r={highlight_r}, θ={highlight_theta}, idx={center_idx}</text>')
    
    # Draw legend grid
    svg_parts.append('')
    svg_parts.append('    <!-- Legend color key -->')
    
    y = 40
    svg_parts.append(f'    <rect x="0" y="{y}" width="20" height="16" fill="#ff6b6b" stroke="#333"/>')
    svg_parts.append(f'    <text x="28" y="{y + 12}" font-family="monospace" font-size="10">CENTER</text>')
    
    y += 22
    svg_parts.append(f'    <text x="0" y="{y}" font-family="sans-serif" font-size="11" font-weight="bold">Direct (4):</text>')
    y += 16
    
    for name in ['θ-', 'θ+', 'r-', 'r+']:
        if name in neighbors:
            nr, nt = neighbors[name]
            nidx = nt + nr * theta_divisions
            color = neighbor_colors[name]
            svg_parts.append(f'    <rect x="0" y="{y}" width="20" height="14" fill="{color}" stroke="#333"/>')
            svg_parts.append(f'    <text x="28" y="{y + 11}" font-family="monospace" font-size="9">{name}: [{nidx}]</text>')
            y += 18
    
    if show_3x3:
        y += 4
        svg_parts.append(f'    <text x="0" y="{y}" font-family="sans-serif" font-size="11" font-weight="bold">Diagonal (4):</text>')
        y += 16
        
        for name in ['θ-,r-', 'θ+,r-', 'θ-,r+', 'θ+,r+']:
            if name in neighbors:
                nr, nt = neighbors[name]
                nidx = nt + nr * theta_divisions
                color = neighbor_colors[name]
                svg_parts.append(f'    <rect x="0" y="{y}" width="20" height="14" fill="{color}" stroke="#333" stroke-dasharray="3,1"/>')
                svg_parts.append(f'    <text x="28" y="{y + 11}" font-family="monospace" font-size="9">{name}: [{nidx}]</text>')
                y += 18
    
    svg_parts.append('  </g>')
    
    # Add 3x3 grid diagram
    svg_parts.append('')
    svg_parts.append('  <!-- 3x3 grid diagram -->')
    svg_parts.append(f'  <g transform="translate({width - 180}, {legend_y})">')
    svg_parts.append(f'    <text font-family="sans-serif" font-size="12" font-weight="bold">Grid Layout:</text>')
    
    # Draw the 3x3 grid
    cell_size = 40
    grid_x = 10
    grid_y = 20
    
    grid_layout = [
        [('θ-,r+', neighbor_colors.get('θ-,r+', '#ccc')), ('r+', neighbor_colors['r+']), ('θ+,r+', neighbor_colors.get('θ+,r+', '#ccc'))],
        [('θ-', neighbor_colors['θ-']), ('C', '#ff6b6b'), ('θ+', neighbor_colors['θ+'])],
        [('θ-,r-', neighbor_colors.get('θ-,r-', '#ccc')), ('r-', neighbor_colors.get('r-', '#ccc')), ('θ+,r-', neighbor_colors.get('θ+,r-', '#ccc'))],
    ]
    
    for row_idx, row in enumerate(grid_layout):
        for col_idx, (label, color) in enumerate(row):
            x = grid_x + col_idx * cell_size
            y = grid_y + row_idx * cell_size
            
            # Check if this neighbor exists
            exists = label == 'C' or label in neighbors
            opacity = '1' if exists else '0.3'
            
            svg_parts.append(
                f'    <rect x="{x}" y="{y}" width="{cell_size-2}" height="{cell_size-2}" '
                f'fill="{color}" stroke="#333" stroke-width="1" opacity="{opacity}"/>'
            )
            svg_parts.append(
                f'    <text x="{x + cell_size/2 - 1}" y="{y + cell_size/2 + 4}" '
                f'text-anchor="middle" font-family="monospace" font-size="9" opacity="{opacity}">{label}</text>'
            )
    
    # Axis labels
    svg_parts.append(f'    <text x="{grid_x + cell_size * 1.5}" y="{grid_y - 5}" text-anchor="middle" font-family="sans-serif" font-size="9">← θ+ | θ- →</text>')
    svg_parts.append(f'    <text x="{grid_x - 5}" y="{grid_y + cell_size * 1.5}" text-anchor="middle" font-family="sans-serif" font-size="9" transform="rotate(-90, {grid_x - 5}, {grid_y + cell_size * 1.5})">← r+ | r- →</text>')
    
    svg_parts.append('  </g>')
    
    # Add info
    svg_parts.append('')
    svg_parts.append(f'  <g transform="translate(10, 20)">')
    svg_parts.append(f'    <text font-family="sans-serif" font-size="14" font-weight="bold">Full 3×3 Neighborhood (8 neighbors)</text>')
    svg_parts.append(f'    <text font-family="monospace" font-size="11" y="18">r_min={r_min}, r_depth={r_depth}, θ_div={theta_divisions}</text>')
    svg_parts.append(f'    <text font-family="sans-serif" font-size="10" y="34" fill="#060">✓ Just like Cartesian: 4 direct + 4 diagonal = 8 neighbors</text>')
    svg_parts.append('  </g>')
    
    svg_parts.append('')
    svg_parts.append('</svg>')
    
    return '\n'.join(svg_parts)


def main():
    output_dir = Path(__file__).parent.parent / 'screenshots' / 'polar_grid'
    output_dir.mkdir(parents=True, exist_ok=True)
    
    # Large ring parameters
    r_min = 128
    r_depth = 5  # 5 rings: r=128 to r=132
    
    # Calculate theta divisions for approximately square cells
    # Target arc length = 1.0 (same as radial thickness)
    target_arc = 1.0
    theta_divisions = calculate_theta_divisions(r_min + r_depth / 2, target_arc)
    
    print(f"Large ring configuration:")
    print(f"  r_min = {r_min}")
    print(f"  r_depth = {r_depth} (r ranges from {r_min} to {r_min + r_depth - 1})")
    print(f"  theta_divisions = {theta_divisions} (for ~square cells)")
    print(f"  Total cells = {r_depth * theta_divisions}")
    
    # Calculate arc lengths at inner and outer rings
    theta_step = 2 * math.pi / theta_divisions
    inner_arc = (r_min + 0.5) * theta_step
    outer_arc = (r_min + r_depth - 0.5) * theta_step
    print(f"  Inner ring arc length: {inner_arc:.3f}")
    print(f"  Outer ring arc length: {outer_arc:.3f}")
    print(f"  Arc ratio (outer/inner): {outer_arc/inner_arc:.4f}")
    print()
    
    # 1. Large ring overview (zoomed to show a section)
    svg1 = create_large_ring_svg(
        r_min=r_min,
        r_depth=r_depth,
        theta_divisions=theta_divisions,
        zoom_section=(0, math.pi / 8),  # Show 22.5 degrees
    )
    path1 = output_dir / f'large_ring_r{r_min}_d{r_depth}_t{theta_divisions}.svg'
    with open(path1, 'w') as f:
        f.write(svg1)
    print(f"Written: {path1}")
    
    # 2. Large ring with 3x3 neighborhood highlight
    # Pick a cell in the middle ring
    highlight_r = r_depth // 2  # Middle ring
    highlight_theta = theta_divisions // 8  # Some angle
    
    svg2 = create_large_ring_with_neighbors_svg(
        r_min=r_min,
        r_depth=r_depth,
        theta_divisions=theta_divisions,
        highlight_r=highlight_r,
        highlight_theta=highlight_theta,
        show_3x3=True,
    )
    path2 = output_dir / f'large_ring_r{r_min}_neighbors_3x3.svg'
    with open(path2, 'w') as f:
        f.write(svg2)
    print(f"Written: {path2}")
    
    # 3. Also generate a comparison at smaller r_min to show the difference
    small_r_min = 4
    small_theta_div = calculate_theta_divisions(small_r_min + r_depth / 2, target_arc)
    
    print(f"\nSmall ring comparison:")
    print(f"  r_min = {small_r_min}")
    print(f"  theta_divisions = {small_theta_div}")
    
    svg3 = create_large_ring_with_neighbors_svg(
        r_min=small_r_min,
        r_depth=r_depth,
        theta_divisions=small_theta_div,
        highlight_r=r_depth // 2,
        highlight_theta=small_theta_div // 8,
        show_3x3=True,
    )
    path3 = output_dir / f'small_ring_r{small_r_min}_neighbors_3x3.svg'
    with open(path3, 'w') as f:
        f.write(svg3)
    print(f"Written: {path3}")
    
    print("\nDone! Open the SVGs to examine the 3x3 neighborhood structure.")
    print("\nKey insight: The polar grid CAN have 8 neighbors (3x3 pattern)!")
    print("- 4 direct: θ-, θ+, r-, r+")
    print("- 4 diagonal: (θ-,r-), (θ+,r-), (θ-,r+), (θ+,r+)")


if __name__ == '__main__':
    main()
