#!/usr/bin/env python3
"""
Detailed analysis visualization of polar grid properties.

This script creates SVGs that highlight:
1. Cell size variation across radii
2. Arc length comparison
3. The "problem" at inner rings (cells become very thin wedges)
4. What neighbor relationships look like at different radii

Usage:
    python scripts/visualize_polar_analysis.py
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


def create_arc_length_comparison_svg(
    r_min: int, r_depth: int, theta_divisions: int
) -> str:
    """
    Create an SVG showing how arc length varies by radius.
    
    This visualization highlights the core issue:
    - All rings have the same number of theta divisions
    - But inner rings have much smaller arc lengths
    - Cells become increasingly thin wedges toward the center
    """
    r_max = r_min + r_depth
    scale = 50.0
    margin = 100
    svg_size = 2 * r_max * scale + 2 * margin
    cx = svg_size / 2
    cy = svg_size / 2
    
    theta_step = 2 * math.pi / theta_divisions
    
    # Calculate arc lengths at each radius
    arc_lengths = []
    for r_idx in range(r_depth):
        r = r_min + r_idx + 0.5  # Middle of ring
        arc_length = r * theta_step  # Arc length = r * theta
        arc_lengths.append((r_idx, r, arc_length))
    
    svg_parts = [
        '<?xml version="1.0" encoding="UTF-8"?>',
        f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {svg_size:.0f} {svg_size + 200:.0f}" '
        f'width="{svg_size:.0f}" height="{svg_size + 200:.0f}">',
        f'  <title>Arc Length Analysis: r_min={r_min}, theta_div={theta_divisions}</title>',
        '',
        '  <rect width="100%" height="100%" fill="#fff"/>',
        '',
        '  <!-- Grid with color gradient showing arc length -->',
        '  <g id="cells">',
    ]
    
    # Color cells by arc length (red = small, green = large)
    min_arc = arc_lengths[0][2]
    max_arc = arc_lengths[-1][2]
    
    for r_idx in range(r_depth):
        r_inner = r_min + r_idx
        r_outer = r_min + r_idx + 1
        arc = arc_lengths[r_idx][2]
        
        # Normalize to 0-1 range
        normalized = (arc - min_arc) / (max_arc - min_arc) if max_arc > min_arc else 0.5
        
        # Color: red (bad/small) to green (good/large)
        r_color = int(255 * (1 - normalized))
        g_color = int(255 * normalized)
        color = f"#{r_color:02x}{g_color:02x}66"
        
        for theta_idx in range(theta_divisions):
            theta_start = theta_idx * theta_step
            theta_end = (theta_idx + 1) * theta_step
            path = create_wedge_path(r_inner, r_outer, theta_start, theta_end, cx, cy, scale)
            svg_parts.append(f'    <path d="{path}" fill="{color}" stroke="#333" stroke-width="1"/>')
    
    svg_parts.append('  </g>')
    
    # Add arc length annotations
    svg_parts.append('')
    svg_parts.append('  <!-- Arc length annotations -->')
    svg_parts.append('  <g font-family="monospace" font-size="11">')
    
    for r_idx, r, arc in arc_lengths:
        # Position annotation to the right of the grid
        x = cx + (r_min + r_idx + 1) * scale + 10
        y = cy
        svg_parts.append(
            f'    <text x="{x:.0f}" y="{y:.0f}" fill="#333">'
            f'r={r_idx}: arc={arc:.2f}</text>'
        )
    
    svg_parts.append('  </g>')
    
    # Add bar chart showing arc lengths
    chart_y = svg_size + 20
    chart_height = 150
    bar_width = (svg_size - 2 * margin) / r_depth
    
    svg_parts.append('')
    svg_parts.append('  <!-- Arc length bar chart -->')
    svg_parts.append(f'  <g transform="translate({margin}, {chart_y})">')
    svg_parts.append(f'    <text font-family="sans-serif" font-size="14" font-weight="bold">Arc Length by Ring (same θ divisions everywhere)</text>')
    
    for r_idx, r, arc in arc_lengths:
        bar_height = (arc / max_arc) * (chart_height - 30)
        bar_x = r_idx * bar_width
        bar_y = chart_height - bar_height
        
        normalized = (arc - min_arc) / (max_arc - min_arc) if max_arc > min_arc else 0.5
        r_color = int(255 * (1 - normalized))
        g_color = int(255 * normalized)
        color = f"#{r_color:02x}{g_color:02x}66"
        
        svg_parts.append(
            f'    <rect x="{bar_x + 5:.0f}" y="{bar_y + 20:.0f}" '
            f'width="{bar_width - 10:.0f}" height="{bar_height:.0f}" fill="{color}" stroke="#333"/>'
        )
        svg_parts.append(
            f'    <text x="{bar_x + bar_width/2:.0f}" y="{chart_height + 15:.0f}" '
            f'text-anchor="middle" font-family="monospace" font-size="10">r={r_idx}</text>'
        )
        svg_parts.append(
            f'    <text x="{bar_x + bar_width/2:.0f}" y="{bar_y + 15:.0f}" '
            f'text-anchor="middle" font-family="monospace" font-size="9">{arc:.2f}</text>'
        )
    
    svg_parts.append('  </g>')
    
    # Add explanation
    svg_parts.append('')
    svg_parts.append('  <!-- Explanation -->')
    svg_parts.append(f'  <g transform="translate(10, 20)">')
    svg_parts.append(f'    <text font-family="sans-serif" font-size="14" font-weight="bold">The Arc Length Problem</text>')
    svg_parts.append(f'    <text font-family="sans-serif" font-size="11" y="18">With {theta_divisions} theta divisions at ALL radii:</text>')
    svg_parts.append(f'    <text font-family="monospace" font-size="10" y="33">• Inner ring (r={r_min}): arc = {arc_lengths[0][2]:.2f}</text>')
    svg_parts.append(f'    <text font-family="monospace" font-size="10" y="46">• Outer ring (r={r_min + r_depth - 1}): arc = {arc_lengths[-1][2]:.2f}</text>')
    svg_parts.append(f'    <text font-family="monospace" font-size="10" y="59">• Ratio: {arc_lengths[-1][2] / arc_lengths[0][2]:.1f}x difference!</text>')
    svg_parts.append(f'    <text font-family="sans-serif" font-size="10" y="76" fill="#c00">Inner cells are {arc_lengths[-1][2] / arc_lengths[0][2]:.1f}x narrower than outer cells.</text>')
    svg_parts.append('  </g>')
    
    svg_parts.append('')
    svg_parts.append('</svg>')
    
    return '\n'.join(svg_parts)


def create_neighbor_problem_svg(
    r_min: int, r_depth: int, theta_divisions: int
) -> str:
    """
    Create an SVG showing the neighbor relationship "problem" (or non-problem).
    
    Key insight: Even though cells are different sizes, the NEIGHBOR INDICES
    are still consistent. Cell [i] at ring r always has:
    - θ- neighbor at index (theta - 1 + theta_div) % theta_div + r * theta_div
    - θ+ neighbor at index (theta + 1) % theta_div + r * theta_div
    - r- neighbor at index theta + (r-1) * theta_div (if r > 0)
    - r+ neighbor at index theta + (r+1) * theta_div (if r < r_depth-1)
    
    The formulas are IDENTICAL regardless of radius!
    """
    r_max = r_min + r_depth
    scale = 60.0
    margin = 80
    svg_size = 2 * r_max * scale + 2 * margin
    cx = svg_size / 2
    cy = svg_size / 2
    
    theta_step = 2 * math.pi / theta_divisions
    
    svg_parts = [
        '<?xml version="1.0" encoding="UTF-8"?>',
        f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {svg_size:.0f} {svg_size:.0f}" '
        f'width="{svg_size:.0f}" height="{svg_size:.0f}">',
        f'  <title>Neighbor Index Consistency</title>',
        '',
        '  <rect width="100%" height="100%" fill="#fff"/>',
        '',
        '  <!-- All cells dimmed -->',
        '  <g id="cells" opacity="0.3">',
    ]
    
    for r_idx in range(r_depth):
        r_inner = r_min + r_idx
        r_outer = r_min + r_idx + 1
        for theta_idx in range(theta_divisions):
            theta_start = theta_idx * theta_step
            theta_end = (theta_idx + 1) * theta_step
            path = create_wedge_path(r_inner, r_outer, theta_start, theta_end, cx, cy, scale)
            svg_parts.append(f'    <path d="{path}" fill="#ddd" stroke="#999" stroke-width="1"/>')
    
    svg_parts.append('  </g>')
    
    # Highlight cells at different radii with same theta, showing their neighbors
    highlight_theta = 2  # Pick a theta index
    colors = ['#ff6b6b', '#4dabf7', '#69db7c', '#ffd43b']  # Different color per ring
    
    svg_parts.append('')
    svg_parts.append('  <!-- Highlighted cells at same theta, different r -->')
    
    for r_idx in range(r_depth):
        color = colors[r_idx % len(colors)]
        r_inner = r_min + r_idx
        r_outer = r_min + r_idx + 1
        
        # Draw the cell
        theta_start = highlight_theta * theta_step
        theta_end = (highlight_theta + 1) * theta_step
        path = create_wedge_path(r_inner, r_outer, theta_start, theta_end, cx, cy, scale)
        svg_parts.append(f'  <path d="{path}" fill="{color}" stroke="#333" stroke-width="2"/>')
        
        # Draw its θ+ neighbor with lighter shade
        theta_plus = (highlight_theta + 1) % theta_divisions
        theta_start_n = theta_plus * theta_step
        theta_end_n = (theta_plus + 1) * theta_step
        path_n = create_wedge_path(r_inner, r_outer, theta_start_n, theta_end_n, cx, cy, scale)
        svg_parts.append(f'  <path d="{path_n}" fill="{color}" fill-opacity="0.4" stroke="#333" stroke-width="1"/>')
    
    # Add index formulas
    svg_parts.append('')
    svg_parts.append('  <!-- Index formulas -->')
    svg_parts.append(f'  <g transform="translate(10, 20)">')
    svg_parts.append(f'    <text font-family="sans-serif" font-size="14" font-weight="bold">Neighbor Index Formulas (SAME at all radii!)</text>')
    svg_parts.append(f'    <text font-family="monospace" font-size="11" y="20">For cell at (r, θ) with flat index = θ + r × θ_div:</text>')
    svg_parts.append(f'    <text font-family="monospace" font-size="10" y="38">θ- neighbor: (θ-1+θ_div) % θ_div + r × θ_div</text>')
    svg_parts.append(f'    <text font-family="monospace" font-size="10" y="52">θ+ neighbor: (θ+1) % θ_div + r × θ_div</text>')
    svg_parts.append(f'    <text font-family="monospace" font-size="10" y="66">r- neighbor: θ + (r-1) × θ_div  [if r > 0]</text>')
    svg_parts.append(f'    <text font-family="monospace" font-size="10" y="80">r+ neighbor: θ + (r+1) × θ_div  [if r &lt; r_depth-1]</text>')
    svg_parts.append(f'    <text font-family="sans-serif" font-size="11" y="100" fill="#060">✓ The lookup is IDENTICAL to a regular pixel grid!</text>')
    svg_parts.append(f'    <text font-family="sans-serif" font-size="11" y="116" fill="#060">✓ Just with (θ, r) instead of (x, y)</text>')
    svg_parts.append('  </g>')
    
    # Show concrete examples
    svg_parts.append('')
    svg_parts.append(f'  <g transform="translate(10, {svg_size - 120})">')
    svg_parts.append(f'    <text font-family="sans-serif" font-size="12" font-weight="bold">Concrete Examples (θ={highlight_theta}, θ_div={theta_divisions}):</text>')
    
    y = 18
    for r_idx in range(r_depth):
        flat_idx = highlight_theta + r_idx * theta_divisions
        theta_plus_idx = ((highlight_theta + 1) % theta_divisions) + r_idx * theta_divisions
        color = colors[r_idx % len(colors)]
        svg_parts.append(
            f'    <rect x="0" y="{y-10}" width="12" height="12" fill="{color}"/>'
        )
        svg_parts.append(
            f'    <text font-family="monospace" font-size="10" x="18" y="{y}">'
            f'r={r_idx}: idx={flat_idx}, θ+ neighbor={theta_plus_idx}</text>'
        )
        y += 16
    
    svg_parts.append('  </g>')
    
    svg_parts.append('')
    svg_parts.append('</svg>')
    
    return '\n'.join(svg_parts)


def create_actual_vs_uniform_comparison(
    r_min: int, r_depth: int, base_theta_divisions: int
) -> str:
    """
    Side-by-side comparison:
    - Left: Current approach (same theta_div everywhere)
    - Right: What "uniform cells" would look like (more divisions at outer rings)
    """
    r_max = r_min + r_depth
    scale = 40.0
    margin = 60
    grid_size = 2 * r_max * scale + margin
    svg_width = grid_size * 2 + 100
    svg_height = grid_size + 150
    
    theta_step = 2 * math.pi / base_theta_divisions
    
    svg_parts = [
        '<?xml version="1.0" encoding="UTF-8"?>',
        f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {svg_width:.0f} {svg_height:.0f}" '
        f'width="{svg_width:.0f}" height="{svg_height:.0f}">',
        f'  <title>Uniform θ divisions vs Adaptive divisions</title>',
        '',
        '  <rect width="100%" height="100%" fill="#fff"/>',
    ]
    
    # LEFT: Current approach (uniform theta divisions)
    cx1 = grid_size / 2
    cy1 = grid_size / 2
    
    svg_parts.append('')
    svg_parts.append('  <!-- LEFT: Uniform theta divisions (current approach) -->')
    svg_parts.append(f'  <text x="{cx1}" y="20" text-anchor="middle" font-family="sans-serif" font-size="14" font-weight="bold">Current: {base_theta_divisions} θ divisions everywhere</text>')
    svg_parts.append('  <g id="uniform">')
    
    for r_idx in range(r_depth):
        r_inner = r_min + r_idx
        r_outer = r_min + r_idx + 1
        for theta_idx in range(base_theta_divisions):
            theta_start = theta_idx * theta_step
            theta_end = (theta_idx + 1) * theta_step
            path = create_wedge_path(r_inner, r_outer, theta_start, theta_end, cx1, cy1, scale)
            # Color by ring
            hue = r_idx / r_depth
            color = f"hsl({int(hue * 360)}, 70%, 70%)"
            svg_parts.append(f'    <path d="{path}" fill="{color}" stroke="#333" stroke-width="1"/>')
    
    svg_parts.append('  </g>')
    
    # RIGHT: Adaptive divisions (more at outer rings to maintain uniform arc length)
    cx2 = grid_size + 50 + grid_size / 2
    cy2 = grid_size / 2
    
    svg_parts.append('')
    svg_parts.append('  <!-- RIGHT: Adaptive theta divisions (uniform arc length) -->')
    svg_parts.append(f'  <text x="{cx2}" y="20" text-anchor="middle" font-family="sans-serif" font-size="14" font-weight="bold">Alternative: Adaptive θ divisions</text>')
    svg_parts.append('  <g id="adaptive">')
    
    # Base arc length from inner ring
    base_arc = (r_min + 0.5) * theta_step
    
    for r_idx in range(r_depth):
        r_inner = r_min + r_idx
        r_outer = r_min + r_idx + 1
        r_mid = r_min + r_idx + 0.5
        
        # Calculate theta divisions to maintain similar arc length
        adaptive_theta_div = max(6, int(2 * math.pi * r_mid / base_arc))
        adaptive_theta_step = 2 * math.pi / adaptive_theta_div
        
        for theta_idx in range(adaptive_theta_div):
            theta_start = theta_idx * adaptive_theta_step
            theta_end = (theta_idx + 1) * adaptive_theta_step
            path = create_wedge_path(r_inner, r_outer, theta_start, theta_end, cx2, cy2, scale)
            hue = r_idx / r_depth
            color = f"hsl({int(hue * 360)}, 70%, 70%)"
            svg_parts.append(f'    <path d="{path}" fill="{color}" stroke="#333" stroke-width="1"/>')
    
    svg_parts.append('  </g>')
    
    # Add comparison notes
    svg_parts.append('')
    svg_parts.append(f'  <g transform="translate(20, {grid_size + 20})">')
    svg_parts.append(f'    <text font-family="sans-serif" font-size="12" font-weight="bold">Comparison:</text>')
    svg_parts.append(f'    <text font-family="sans-serif" font-size="11" y="18">LEFT (Current):</text>')
    svg_parts.append(f'    <text font-family="monospace" font-size="10" y="32">• Same θ divisions at all radii</text>')
    svg_parts.append(f'    <text font-family="monospace" font-size="10" y="44">• Simple neighbor formulas</text>')
    svg_parts.append(f'    <text font-family="monospace" font-size="10" y="56">• Inner cells are thin wedges</text>')
    svg_parts.append(f'    <text font-family="monospace" font-size="10" y="68" fill="#060">• Pattern matching: center + 4 neighbors</text>')
    svg_parts.append('  </g>')
    
    svg_parts.append(f'  <g transform="translate({grid_size + 70}, {grid_size + 20})">')
    svg_parts.append(f'    <text font-family="sans-serif" font-size="11">RIGHT (Adaptive):</text>')
    svg_parts.append(f'    <text font-family="monospace" font-size="10" y="14">• More θ divisions at outer radii</text>')
    svg_parts.append(f'    <text font-family="monospace" font-size="10" y="26">• More uniform cell sizes</text>')
    svg_parts.append(f'    <text font-family="monospace" font-size="10" y="38">• Complex neighbor mapping</text>')
    svg_parts.append(f'    <text font-family="monospace" font-size="10" y="50" fill="#c00">• Pattern matching: ???</text>')
    svg_parts.append('  </g>')
    
    svg_parts.append('')
    svg_parts.append('</svg>')
    
    return '\n'.join(svg_parts)


def main():
    output_dir = Path(__file__).parent.parent / 'screenshots' / 'polar_grid'
    output_dir.mkdir(parents=True, exist_ok=True)
    
    # Generate arc length analysis
    svg1 = create_arc_length_comparison_svg(r_min=2, r_depth=5, theta_divisions=12)
    path1 = output_dir / 'polar_arc_length_analysis.svg'
    with open(path1, 'w') as f:
        f.write(svg1)
    print(f"Written: {path1}")
    
    # Generate neighbor consistency analysis
    svg2 = create_neighbor_problem_svg(r_min=2, r_depth=4, theta_divisions=8)
    path2 = output_dir / 'polar_neighbor_consistency.svg'
    with open(path2, 'w') as f:
        f.write(svg2)
    print(f"Written: {path2}")
    
    # Generate comparison
    svg3 = create_actual_vs_uniform_comparison(r_min=2, r_depth=5, base_theta_divisions=8)
    path3 = output_dir / 'polar_uniform_vs_adaptive.svg'
    with open(path3, 'w') as f:
        f.write(svg3)
    print(f"Written: {path3}")
    
    print("\nAll SVGs generated. Open in a browser or SVG viewer to examine.")


if __name__ == '__main__':
    main()
