#!/usr/bin/env python3
"""
Visualize large-radius polar grids - ZOOMED IN version.

Clean visualization without cluttered text, blown up to see cell shapes clearly.
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


def create_zoomed_ring_svg(
    r_min: int, 
    r_depth: int, 
    theta_divisions: int,
    cells_to_show: int = 8,  # How many cells wide to show
) -> str:
    """
    Create a ZOOMED view of a section of the ring.
    Shows only a few cells, blown up large so you can see the shapes clearly.
    No text labels - just the cells with colors.
    """
    theta_step = 2 * math.pi / theta_divisions
    
    # Calculate view bounds - show just a few cells
    view_theta_start = 0
    view_theta_end = cells_to_show * theta_step
    
    # Large scale factor for zoomed view
    scale = 80.0  # Much larger scale
    margin = 40
    
    # Calculate visible area
    r_max = r_min + r_depth
    
    corners = []
    for r in [r_min, r_max]:
        for theta in [view_theta_start, view_theta_end]:
            corners.append(polar_to_cartesian(r, theta))
        for t in range(20):
            theta = view_theta_start + (view_theta_end - view_theta_start) * t / 19
            corners.append(polar_to_cartesian(r, theta))
    
    xs = [c[0] for c in corners]
    ys = [c[1] for c in corners]
    x_min, x_max = min(xs), max(xs)
    y_min, y_max = min(ys), max(ys)
    
    width = (x_max - x_min) * scale + 2 * margin
    height = (y_max - y_min) * scale + 2 * margin
    
    cx = margin - x_min * scale
    cy = height - margin + y_min * scale
    
    svg_parts = [
        '<?xml version="1.0" encoding="UTF-8"?>',
        f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {width:.0f} {height:.0f}" '
        f'width="{width:.0f}" height="{height:.0f}">',
        '',
        '  <rect width="100%" height="100%" fill="#fff"/>',
        '',
        '  <!-- Grid cells -->',
        '  <g id="cells">',
    ]
    
    # Draw cells
    for r_idx in range(r_depth):
        r_inner = r_min + r_idx
        r_outer = r_min + r_idx + 1
        
        for theta_idx in range(min(cells_to_show + 1, theta_divisions)):
            theta_start = theta_idx * theta_step
            theta_end = (theta_idx + 1) * theta_step
            
            if theta_end < view_theta_start or theta_start > view_theta_end:
                continue
            
            path = create_wedge_path(r_inner, r_outer, theta_start, theta_end, cx, cy, scale)
            
            # Color by theta index (hue varies with theta)
            hue = (theta_idx / cells_to_show) * 300  # Use 300 degrees of hue range
            color = f"hsl({hue:.0f}, 70%, 65%)"
            
            svg_parts.append(
                f'    <path d="{path}" fill="{color}" stroke="#333" stroke-width="2"/>'
            )
    
    svg_parts.append('  </g>')
    svg_parts.append('')
    svg_parts.append('</svg>')
    
    return '\n'.join(svg_parts)


def create_zoomed_neighbors_svg(
    r_min: int,
    r_depth: int, 
    theta_divisions: int,
    highlight_r: int,
    highlight_theta: int,
) -> str:
    """
    Create a ZOOMED view showing a cell and its 3x3 neighborhood.
    Large cells, clear colors, minimal text.
    """
    theta_step = 2 * math.pi / theta_divisions
    
    # Show 5 cells in theta direction, all r_depth cells in r direction
    cells_margin = 2  # cells on each side of highlight
    
    view_theta_start = (highlight_theta - cells_margin) * theta_step
    view_theta_end = (highlight_theta + cells_margin + 1) * theta_step
    
    scale = 100.0  # Very large scale
    margin = 60
    
    # Calculate visible area
    r_view_min = r_min
    r_view_max = r_min + r_depth
    
    corners = []
    for r in [r_view_min, r_view_max]:
        for t in range(20):
            theta = view_theta_start + (view_theta_end - view_theta_start) * t / 19
            corners.append(polar_to_cartesian(r, theta))
    
    xs = [c[0] for c in corners]
    ys = [c[1] for c in corners]
    x_min, x_max = min(xs), max(xs)
    y_min, y_max = min(ys), max(ys)
    
    grid_width = (x_max - x_min) * scale + 2 * margin
    grid_height = (y_max - y_min) * scale + 2 * margin
    
    # Add space for legend on the right
    legend_width = 200
    total_width = grid_width + legend_width
    
    cx = margin - x_min * scale
    cy = grid_height - margin + y_min * scale
    
    # Calculate neighbors
    theta_minus = (highlight_theta - 1 + theta_divisions) % theta_divisions
    theta_plus = (highlight_theta + 1) % theta_divisions
    
    neighbors = {}
    neighbors['θ-'] = (highlight_r, theta_minus)
    neighbors['θ+'] = (highlight_r, theta_plus)
    
    if highlight_r > 0:
        neighbors['r-'] = (highlight_r - 1, highlight_theta)
        neighbors['θ-,r-'] = (highlight_r - 1, theta_minus)
        neighbors['θ+,r-'] = (highlight_r - 1, theta_plus)
    
    if highlight_r < r_depth - 1:
        neighbors['r+'] = (highlight_r + 1, highlight_theta)
        neighbors['θ-,r+'] = (highlight_r + 1, theta_minus)
        neighbors['θ+,r+'] = (highlight_r + 1, theta_plus)
    
    # Colors
    neighbor_colors = {
        'θ-': '#4dabf7',
        'θ+': '#69db7c',
        'r-': '#ffd43b',
        'r+': '#da77f2',
        'θ-,r-': '#ff8787',
        'θ+,r-': '#ffa94d',
        'θ-,r+': '#63e6be',
        'θ+,r+': '#b197fc',
    }
    
    svg_parts = [
        '<?xml version="1.0" encoding="UTF-8"?>',
        f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {total_width:.0f} {grid_height:.0f}" '
        f'width="{total_width:.0f}" height="{grid_height:.0f}">',
        '',
        '  <rect width="100%" height="100%" fill="#fff"/>',
        '',
        '  <!-- Background cells (dimmed) -->',
        '  <g id="cells-bg" opacity="0.25">',
    ]
    
    # Draw all visible cells dimmed
    for r_idx in range(r_depth):
        r_inner = r_min + r_idx
        r_outer = r_min + r_idx + 1
        
        for theta_idx in range(highlight_theta - cells_margin, highlight_theta + cells_margin + 1):
            actual_theta = theta_idx % theta_divisions
            theta_start = theta_idx * theta_step
            theta_end = (theta_idx + 1) * theta_step
            
            path = create_wedge_path(r_inner, r_outer, theta_start, theta_end, cx, cy, scale)
            svg_parts.append(f'    <path d="{path}" fill="#bbb" stroke="#888" stroke-width="1"/>')
    
    svg_parts.append('  </g>')
    
    # Draw neighbor cells
    svg_parts.append('')
    svg_parts.append('  <!-- Neighbor cells -->')
    
    for name, (nr, nt) in neighbors.items():
        if nr < 0 or nr >= r_depth:
            continue
        
        r_inner = r_min + nr
        r_outer = r_min + nr + 1
        
        # Handle theta wrapping for display
        display_theta = nt
        if nt == theta_minus and highlight_theta == 0:
            display_theta = -1  # Show to the left
        elif nt == theta_plus and highlight_theta == theta_divisions - 1:
            display_theta = theta_divisions  # Show to the right
        
        # Adjust theta for cells near the highlight
        if abs(display_theta - highlight_theta) > cells_margin:
            if nt < highlight_theta:
                display_theta = highlight_theta - 1
            else:
                display_theta = highlight_theta + 1
        
        theta_start = display_theta * theta_step
        theta_end = (display_theta + 1) * theta_step
        
        path = create_wedge_path(r_inner, r_outer, theta_start, theta_end, cx, cy, scale)
        color = neighbor_colors.get(name, '#aaa')
        
        is_diagonal = ',' in name
        stroke_width = 2 if is_diagonal else 3
        
        svg_parts.append(
            f'  <path d="{path}" fill="{color}" stroke="#333" stroke-width="{stroke_width}"/>'
        )
    
    # Draw center cell
    svg_parts.append('')
    svg_parts.append('  <!-- Center cell -->')
    r_inner = r_min + highlight_r
    r_outer = r_min + highlight_r + 1
    theta_start = highlight_theta * theta_step
    theta_end = (highlight_theta + 1) * theta_step
    path = create_wedge_path(r_inner, r_outer, theta_start, theta_end, cx, cy, scale)
    svg_parts.append(f'  <path d="{path}" fill="#ff6b6b" stroke="#a00" stroke-width="4"/>')
    
    # Legend on the right side
    svg_parts.append('')
    svg_parts.append('  <!-- Legend -->')
    svg_parts.append(f'  <g transform="translate({grid_width + 10}, 30)">')
    
    # Title
    svg_parts.append(f'    <text font-family="sans-serif" font-size="16" font-weight="bold">3×3 Neighbors</text>')
    
    # Center
    y = 35
    svg_parts.append(f'    <rect x="0" y="{y}" width="24" height="20" fill="#ff6b6b" stroke="#a00" stroke-width="2"/>')
    svg_parts.append(f'    <text x="32" y="{y + 15}" font-family="sans-serif" font-size="13">Center</text>')
    
    # Direct neighbors
    y += 35
    svg_parts.append(f'    <text x="0" y="{y}" font-family="sans-serif" font-size="14" font-weight="bold">Direct (4):</text>')
    
    for name in ['θ-', 'θ+', 'r-', 'r+']:
        if name in neighbors:
            y += 26
            color = neighbor_colors[name]
            svg_parts.append(f'    <rect x="0" y="{y}" width="24" height="20" fill="{color}" stroke="#333" stroke-width="2"/>')
            svg_parts.append(f'    <text x="32" y="{y + 15}" font-family="sans-serif" font-size="13">{name}</text>')
    
    # Diagonal neighbors
    y += 35
    svg_parts.append(f'    <text x="0" y="{y}" font-family="sans-serif" font-size="14" font-weight="bold">Diagonal (4):</text>')
    
    for name in ['θ-,r-', 'θ+,r-', 'θ-,r+', 'θ+,r+']:
        if name in neighbors:
            y += 26
            color = neighbor_colors[name]
            svg_parts.append(f'    <rect x="0" y="{y}" width="24" height="20" fill="{color}" stroke="#333" stroke-width="2"/>')
            svg_parts.append(f'    <text x="32" y="{y + 15}" font-family="sans-serif" font-size="13">{name}</text>')
    
    # 3x3 grid diagram
    y += 45
    svg_parts.append(f'    <text x="0" y="{y}" font-family="sans-serif" font-size="14" font-weight="bold">Layout:</text>')
    
    cell_size = 36
    grid_x = 20
    grid_y = y + 10
    
    grid_layout = [
        [('θ-,r+', neighbor_colors.get('θ-,r+', '#ccc')), ('r+', neighbor_colors.get('r+', '#ccc')), ('θ+,r+', neighbor_colors.get('θ+,r+', '#ccc'))],
        [('θ-', neighbor_colors['θ-']), ('C', '#ff6b6b'), ('θ+', neighbor_colors['θ+'])],
        [('θ-,r-', neighbor_colors.get('θ-,r-', '#ccc')), ('r-', neighbor_colors.get('r-', '#ccc')), ('θ+,r-', neighbor_colors.get('θ+,r-', '#ccc'))],
    ]
    
    for row_idx, row in enumerate(grid_layout):
        for col_idx, (label, color) in enumerate(row):
            x = grid_x + col_idx * cell_size
            gy = grid_y + row_idx * cell_size
            exists = label == 'C' or label in neighbors
            opacity = '1' if exists else '0.3'
            
            svg_parts.append(
                f'    <rect x="{x}" y="{gy}" width="{cell_size-3}" height="{cell_size-3}" '
                f'fill="{color}" stroke="#333" stroke-width="1" opacity="{opacity}"/>'
            )
    
    svg_parts.append('  </g>')
    
    svg_parts.append('')
    svg_parts.append('</svg>')
    
    return '\n'.join(svg_parts)


def main():
    output_dir = Path(__file__).parent.parent / 'screenshots' / 'polar_grid'
    output_dir.mkdir(parents=True, exist_ok=True)
    
    target_arc = 1.0
    
    # R=512 configuration (512 to 523 = 12 rings)
    r_min_512 = 512
    r_depth_512 = 12
    theta_div_512 = calculate_theta_divisions(r_min_512 + r_depth_512 / 2, target_arc)
    
    print(f"R=512 ring configuration (512-523):")
    print(f"  r_min = {r_min_512}")
    print(f"  r_depth = {r_depth_512}")
    print(f"  theta_divisions = {theta_div_512}")
    
    theta_step = 2 * math.pi / theta_div_512
    inner_arc = (r_min_512 + 0.5) * theta_step
    outer_arc = (r_min_512 + r_depth_512 - 0.5) * theta_step
    print(f"  Inner arc: {inner_arc:.4f}")
    print(f"  Outer arc: {outer_arc:.4f}")
    print(f"  Ratio: {outer_arc/inner_arc:.4f}")
    print()
    
    # Zoomed ring view
    svg1 = create_zoomed_ring_svg(
        r_min=r_min_512,
        r_depth=r_depth_512,
        theta_divisions=theta_div_512,
        cells_to_show=12,
    )
    path1 = output_dir / f'ring_r{r_min_512}_zoomed.svg'
    with open(path1, 'w') as f:
        f.write(svg1)
    print(f"Written: {path1}")
    
    # Neighbor highlight view (middle of the 12 rings)
    svg2 = create_zoomed_neighbors_svg(
        r_min=r_min_512,
        r_depth=r_depth_512,
        theta_divisions=theta_div_512,
        highlight_r=6,  # Middle of 12 rings
        highlight_theta=5,
    )
    path2 = output_dir / f'ring_r{r_min_512}_neighbors.svg'
    with open(path2, 'w') as f:
        f.write(svg2)
    print(f"Written: {path2}")
    
    # R=256 configuration
    r_min_256 = 256
    r_depth_256 = 5
    theta_div_256 = calculate_theta_divisions(r_min_256 + r_depth_256 / 2, target_arc)
    
    print(f"\nR=256 ring configuration:")
    print(f"  r_depth = {r_depth_256}")
    print(f"  theta_divisions = {theta_div_256}")
    
    svg3 = create_zoomed_ring_svg(
        r_min=r_min_256,
        r_depth=r_depth_256,
        theta_divisions=theta_div_256,
        cells_to_show=10,
    )
    path3 = output_dir / f'ring_r{r_min_256}_zoomed.svg'
    with open(path3, 'w') as f:
        f.write(svg3)
    print(f"Written: {path3}")
    
    svg4 = create_zoomed_neighbors_svg(
        r_min=r_min_256,
        r_depth=r_depth_256,
        theta_divisions=theta_div_256,
        highlight_r=2,
        highlight_theta=5,
    )
    path4 = output_dir / f'ring_r{r_min_256}_neighbors.svg'
    with open(path4, 'w') as f:
        f.write(svg4)
    print(f"Written: {path4}")
    
    # R=128 configuration
    r_min_128 = 128
    r_depth_128 = 5
    theta_div_128 = calculate_theta_divisions(r_min_128 + r_depth_128 / 2, target_arc)
    
    print(f"\nR=128 ring configuration:")
    print(f"  theta_divisions = {theta_div_128}")
    
    svg5 = create_zoomed_ring_svg(
        r_min=r_min_128,
        r_depth=r_depth_128,
        theta_divisions=theta_div_128,
        cells_to_show=10,
    )
    path5 = output_dir / f'ring_r{r_min_128}_zoomed.svg'
    with open(path5, 'w') as f:
        f.write(svg5)
    print(f"Written: {path5}")
    
    svg6 = create_zoomed_neighbors_svg(
        r_min=r_min_128,
        r_depth=r_depth_128,
        theta_divisions=theta_div_128,
        highlight_r=2,
        highlight_theta=5,
    )
    path6 = output_dir / f'ring_r{r_min_128}_neighbors.svg'
    with open(path6, 'w') as f:
        f.write(svg6)
    print(f"Written: {path6}")
    
    print("\nDone!")


if __name__ == '__main__':
    main()
