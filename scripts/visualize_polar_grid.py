#!/usr/bin/env python3
"""
Visualize a 2D Polar Grid as SVG.

This script renders the polar/spherical grid structure showing:
- Every cell as a distinct colored wedge/trapezoid
- Cell borders clearly visible
- Neighbor relationships visible through adjacency
- Labels showing cell indices and coordinates

Usage:
    python scripts/visualize_polar_grid.py [--r-min 4] [--r-depth 5] [--theta-divs 12] [--output grid.svg]
"""

import argparse
import math
import colorsys
from pathlib import Path


def generate_distinct_colors(n: int) -> list[str]:
    """Generate n visually distinct colors using HSL color space."""
    colors = []
    for i in range(n):
        # Use golden ratio to spread hues evenly
        hue = (i * 0.618033988749895) % 1.0
        # Vary saturation and lightness slightly for more distinction
        sat = 0.6 + (i % 3) * 0.15
        light = 0.45 + ((i // 3) % 3) * 0.15
        rgb = colorsys.hls_to_rgb(hue, light, sat)
        hex_color = "#{:02x}{:02x}{:02x}".format(
            int(rgb[0] * 255), int(rgb[1] * 255), int(rgb[2] * 255)
        )
        colors.append(hex_color)
    return colors


def polar_to_cartesian(r: float, theta: float) -> tuple[float, float]:
    """Convert polar coordinates to Cartesian (x, y)."""
    x = r * math.cos(theta)
    y = r * math.sin(theta)
    return x, y


def create_wedge_path(
    r_inner: float,
    r_outer: float,
    theta_start: float,
    theta_end: float,
    cx: float,
    cy: float,
    scale: float,
) -> str:
    """
    Create an SVG path for a wedge/trapezoid cell.
    
    The cell is bounded by:
    - Inner arc at r_inner from theta_start to theta_end
    - Outer arc at r_outer from theta_start to theta_end
    - Two radial edges connecting them
    """
    # Calculate corner points
    inner_start = polar_to_cartesian(r_inner, theta_start)
    inner_end = polar_to_cartesian(r_inner, theta_end)
    outer_start = polar_to_cartesian(r_outer, theta_start)
    outer_end = polar_to_cartesian(r_outer, theta_end)
    
    # Transform to SVG coordinates (flip y, scale, translate to center)
    def transform(p):
        return (cx + p[0] * scale, cy - p[1] * scale)
    
    p1 = transform(inner_start)  # Inner arc start
    p2 = transform(inner_end)    # Inner arc end
    p3 = transform(outer_end)    # Outer arc end
    p4 = transform(outer_start)  # Outer arc start
    
    # Arc radii in SVG coordinates
    r_inner_svg = r_inner * scale
    r_outer_svg = r_outer * scale
    
    # Determine if arcs are large (> 180 degrees)
    arc_angle = theta_end - theta_start
    large_arc = 1 if arc_angle > math.pi else 0
    
    # SVG path:
    # M = move to inner_start
    # A = arc to inner_end (inner arc, counterclockwise)
    # L = line to outer_end
    # A = arc to outer_start (outer arc, clockwise - so sweep=0)
    # Z = close path back to start
    
    path = (
        f"M {p1[0]:.2f} {p1[1]:.2f} "
        f"A {r_inner_svg:.2f} {r_inner_svg:.2f} 0 {large_arc} 0 {p2[0]:.2f} {p2[1]:.2f} "
        f"L {p3[0]:.2f} {p3[1]:.2f} "
        f"A {r_outer_svg:.2f} {r_outer_svg:.2f} 0 {large_arc} 1 {p4[0]:.2f} {p4[1]:.2f} "
        f"Z"
    )
    
    return path


def create_polar_grid_svg(
    r_min: int,
    r_depth: int,
    theta_divisions: int,
    show_labels: bool = True,
    show_indices: bool = True,
    cell_size: float = 50.0,
) -> str:
    """
    Create an SVG visualization of a 2D polar grid.
    
    Args:
        r_min: Minimum radius (inner edge)
        r_depth: Number of radial rings
        theta_divisions: Number of angular divisions
        show_labels: Whether to show (r, theta) labels
        show_indices: Whether to show flat indices
        cell_size: Approximate size of cells in pixels
    
    Returns:
        SVG content as a string
    """
    # Calculate total cells and generate colors
    total_cells = r_depth * theta_divisions
    colors = generate_distinct_colors(total_cells)
    
    # Calculate SVG dimensions
    r_max = r_min + r_depth
    scale = cell_size  # pixels per unit radius
    margin = 60
    svg_size = 2 * r_max * scale + 2 * margin
    cx = svg_size / 2  # center x
    cy = svg_size / 2  # center y
    
    # Start SVG
    svg_parts = [
        f'<?xml version="1.0" encoding="UTF-8"?>',
        f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {svg_size:.0f} {svg_size:.0f}" width="{svg_size:.0f}" height="{svg_size:.0f}">',
        f'  <title>Polar Grid: r_min={r_min}, r_depth={r_depth}, theta_divisions={theta_divisions}</title>',
        f'  <desc>Total cells: {total_cells}. Index formula: idx = theta + r * theta_divisions</desc>',
        '',
        '  <!-- Background -->',
        f'  <rect width="100%" height="100%" fill="#f5f5f5"/>',
        '',
        '  <!-- Grid cells -->',
        '  <g id="cells">',
    ]
    
    # Draw each cell
    theta_step = 2 * math.pi / theta_divisions
    
    for r_idx in range(r_depth):
        r_inner = r_min + r_idx
        r_outer = r_min + r_idx + 1
        
        for theta_idx in range(theta_divisions):
            # Flat index: idx = theta + r * theta_divisions
            flat_idx = theta_idx + r_idx * theta_divisions
            
            # Angular bounds
            theta_start = theta_idx * theta_step
            theta_end = (theta_idx + 1) * theta_step
            
            # Create wedge path
            path = create_wedge_path(
                r_inner, r_outer, theta_start, theta_end, cx, cy, scale
            )
            
            # Get color for this cell
            color = colors[flat_idx]
            
            svg_parts.append(
                f'    <path d="{path}" fill="{color}" stroke="#333" stroke-width="1" '
                f'data-idx="{flat_idx}" data-r="{r_idx}" data-theta="{theta_idx}"/>'
            )
    
    svg_parts.append('  </g>')
    
    # Add labels if requested
    if show_labels or show_indices:
        svg_parts.append('')
        svg_parts.append('  <!-- Cell labels -->')
        svg_parts.append('  <g id="labels" font-family="monospace" font-size="10" text-anchor="middle">')
        
        for r_idx in range(r_depth):
            r_center = r_min + r_idx + 0.5
            
            for theta_idx in range(theta_divisions):
                flat_idx = theta_idx + r_idx * theta_divisions
                
                # Calculate label position (center of cell)
                theta_center = (theta_idx + 0.5) * theta_step
                label_x, label_y = polar_to_cartesian(r_center, theta_center)
                label_x = cx + label_x * scale
                label_y = cy - label_y * scale
                
                # Build label text
                lines = []
                if show_indices:
                    lines.append(f"[{flat_idx}]")
                if show_labels:
                    lines.append(f"r={r_idx}")
                    lines.append(f"θ={theta_idx}")
                
                # Adjust font size based on cell size and number of lines
                cell_angular_width = r_center * theta_step * scale
                font_size = min(10, cell_angular_width / 4)
                if font_size < 6:
                    # Too small to show labels
                    continue
                
                for i, line in enumerate(lines):
                    y_offset = (i - len(lines)/2 + 0.5) * (font_size + 2)
                    svg_parts.append(
                        f'    <text x="{label_x:.1f}" y="{label_y + y_offset:.1f}" '
                        f'font-size="{font_size:.1f}" fill="#000">{line}</text>'
                    )
        
        svg_parts.append('  </g>')
    
    # Add axis indicators
    svg_parts.append('')
    svg_parts.append('  <!-- Axis indicators -->')
    svg_parts.append('  <g id="axes" stroke="#666" stroke-width="1" stroke-dasharray="5,5">')
    svg_parts.append(f'    <line x1="{cx}" y1="{margin}" x2="{cx}" y2="{svg_size - margin}" />')
    svg_parts.append(f'    <line x1="{margin}" y1="{cy}" x2="{svg_size - margin}" y2="{cy}" />')
    svg_parts.append('  </g>')
    
    # Add legend
    svg_parts.append('')
    svg_parts.append('  <!-- Legend -->')
    svg_parts.append(f'  <g id="legend" transform="translate(10, 20)">')
    svg_parts.append(f'    <text font-family="sans-serif" font-size="14" font-weight="bold">Polar Grid</text>')
    svg_parts.append(f'    <text font-family="monospace" font-size="11" y="18">r_min={r_min}, r_depth={r_depth}, θ_div={theta_divisions}</text>')
    svg_parts.append(f'    <text font-family="monospace" font-size="11" y="33">Total cells: {total_cells}</text>')
    svg_parts.append(f'    <text font-family="monospace" font-size="11" y="48">Index: idx = θ + r × θ_div</text>')
    svg_parts.append('  </g>')
    
    # Add neighbor explanation
    svg_parts.append('')
    svg_parts.append('  <!-- Neighbor info -->')
    svg_parts.append(f'  <g id="neighbor-info" transform="translate(10, {svg_size - 80})">')
    svg_parts.append(f'    <text font-family="sans-serif" font-size="12" font-weight="bold">Neighbors:</text>')
    svg_parts.append(f'    <text font-family="monospace" font-size="10" y="15">θ-: (θ-1) mod θ_div, same r</text>')
    svg_parts.append(f'    <text font-family="monospace" font-size="10" y="28">θ+: (θ+1) mod θ_div, same r</text>')
    svg_parts.append(f'    <text font-family="monospace" font-size="10" y="41">r-: same θ, r-1 (if r>0)</text>')
    svg_parts.append(f'    <text font-family="monospace" font-size="10" y="54">r+: same θ, r+1 (if r&lt;r_depth-1)</text>')
    svg_parts.append('  </g>')
    
    # Close SVG
    svg_parts.append('')
    svg_parts.append('</svg>')
    
    return '\n'.join(svg_parts)


def create_neighbor_highlight_svg(
    r_min: int,
    r_depth: int,
    theta_divisions: int,
    highlight_r: int,
    highlight_theta: int,
    cell_size: float = 50.0,
) -> str:
    """
    Create an SVG showing one cell and its neighbors highlighted.
    """
    total_cells = r_depth * theta_divisions
    
    # Calculate SVG dimensions
    r_max = r_min + r_depth
    scale = cell_size
    margin = 60
    svg_size = 2 * r_max * scale + 2 * margin
    cx = svg_size / 2
    cy = svg_size / 2
    
    # Compute neighbors
    center_idx = highlight_theta + highlight_r * theta_divisions
    
    neighbors = {}
    # θ- neighbor
    theta_minus = (highlight_theta - 1 + theta_divisions) % theta_divisions
    neighbors['θ-'] = (highlight_r, theta_minus, theta_minus + highlight_r * theta_divisions)
    
    # θ+ neighbor
    theta_plus = (highlight_theta + 1) % theta_divisions
    neighbors['θ+'] = (highlight_r, theta_plus, theta_plus + highlight_r * theta_divisions)
    
    # r- neighbor
    if highlight_r > 0:
        neighbors['r-'] = (highlight_r - 1, highlight_theta, highlight_theta + (highlight_r - 1) * theta_divisions)
    
    # r+ neighbor
    if highlight_r < r_depth - 1:
        neighbors['r+'] = (highlight_r + 1, highlight_theta, highlight_theta + (highlight_r + 1) * theta_divisions)
    
    # Start SVG
    svg_parts = [
        f'<?xml version="1.0" encoding="UTF-8"?>',
        f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {svg_size:.0f} {svg_size:.0f}" width="{svg_size:.0f}" height="{svg_size:.0f}">',
        f'  <title>Neighbor Highlight: Cell ({highlight_r}, {highlight_theta})</title>',
        '',
        '  <!-- Background -->',
        f'  <rect width="100%" height="100%" fill="#f5f5f5"/>',
        '',
        '  <!-- All cells (dimmed) -->',
        '  <g id="cells-dimmed" opacity="0.3">',
    ]
    
    theta_step = 2 * math.pi / theta_divisions
    
    # Draw all cells dimmed
    for r_idx in range(r_depth):
        r_inner = r_min + r_idx
        r_outer = r_min + r_idx + 1
        
        for theta_idx in range(theta_divisions):
            flat_idx = theta_idx + r_idx * theta_divisions
            theta_start = theta_idx * theta_step
            theta_end = (theta_idx + 1) * theta_step
            
            path = create_wedge_path(
                r_inner, r_outer, theta_start, theta_end, cx, cy, scale
            )
            
            svg_parts.append(
                f'    <path d="{path}" fill="#ccc" stroke="#999" stroke-width="1"/>'
            )
    
    svg_parts.append('  </g>')
    svg_parts.append('')
    
    # Draw highlighted center cell
    svg_parts.append('  <!-- Center cell (highlighted) -->')
    r_inner = r_min + highlight_r
    r_outer = r_min + highlight_r + 1
    theta_start = highlight_theta * theta_step
    theta_end = (highlight_theta + 1) * theta_step
    path = create_wedge_path(r_inner, r_outer, theta_start, theta_end, cx, cy, scale)
    svg_parts.append(f'  <path d="{path}" fill="#ff6b6b" stroke="#c92a2a" stroke-width="3"/>')
    
    # Draw neighbor cells
    svg_parts.append('')
    svg_parts.append('  <!-- Neighbor cells -->')
    neighbor_colors = {'θ-': '#4dabf7', 'θ+': '#69db7c', 'r-': '#ffd43b', 'r+': '#da77f2'}
    
    for name, (nr, nt, nidx) in neighbors.items():
        r_inner = r_min + nr
        r_outer = r_min + nr + 1
        theta_start = nt * theta_step
        theta_end = (nt + 1) * theta_step
        path = create_wedge_path(r_inner, r_outer, theta_start, theta_end, cx, cy, scale)
        color = neighbor_colors[name]
        svg_parts.append(f'  <path d="{path}" fill="{color}" stroke="#333" stroke-width="2"/>')
    
    # Add labels
    svg_parts.append('')
    svg_parts.append('  <!-- Labels -->')
    svg_parts.append('  <g font-family="monospace" font-size="12" font-weight="bold" text-anchor="middle">')
    
    # Center label
    r_center = r_min + highlight_r + 0.5
    theta_center = (highlight_theta + 0.5) * theta_step
    lx, ly = polar_to_cartesian(r_center, theta_center)
    lx = cx + lx * scale
    ly = cy - ly * scale
    svg_parts.append(f'  <text x="{lx:.1f}" y="{ly:.1f}" fill="#fff">CENTER</text>')
    svg_parts.append(f'  <text x="{lx:.1f}" y="{ly + 14:.1f}" fill="#fff" font-size="10">[{center_idx}]</text>')
    
    # Neighbor labels
    for name, (nr, nt, nidx) in neighbors.items():
        r_center = r_min + nr + 0.5
        theta_center = (nt + 0.5) * theta_step
        lx, ly = polar_to_cartesian(r_center, theta_center)
        lx = cx + lx * scale
        ly = cy - ly * scale
        svg_parts.append(f'  <text x="{lx:.1f}" y="{ly:.1f}" fill="#000">{name}</text>')
        svg_parts.append(f'  <text x="{lx:.1f}" y="{ly + 12:.1f}" fill="#000" font-size="10">[{nidx}]</text>')
    
    svg_parts.append('  </g>')
    
    # Add legend
    svg_parts.append('')
    svg_parts.append('  <!-- Legend -->')
    svg_parts.append(f'  <g id="legend" transform="translate(10, 20)">')
    svg_parts.append(f'    <text font-family="sans-serif" font-size="14" font-weight="bold">Neighbor Relationships</text>')
    svg_parts.append(f'    <text font-family="monospace" font-size="11" y="20">Center: r={highlight_r}, θ={highlight_theta}, idx={center_idx}</text>')
    svg_parts.append(f'    <rect x="0" y="30" width="15" height="15" fill="#ff6b6b"/>')
    svg_parts.append(f'    <text font-family="monospace" font-size="10" x="20" y="42">CENTER</text>')
    y = 50
    for name, (nr, nt, nidx) in neighbors.items():
        color = neighbor_colors[name]
        svg_parts.append(f'    <rect x="0" y="{y}" width="15" height="15" fill="{color}"/>')
        svg_parts.append(f'    <text font-family="monospace" font-size="10" x="20" y="{y+12}">{name}: r={nr}, θ={nt}, idx={nidx}</text>')
        y += 20
    svg_parts.append('  </g>')
    
    svg_parts.append('')
    svg_parts.append('</svg>')
    
    return '\n'.join(svg_parts)


def main():
    parser = argparse.ArgumentParser(description='Visualize a 2D Polar Grid as SVG')
    parser.add_argument('--r-min', type=int, default=4, help='Minimum radius (inner edge)')
    parser.add_argument('--r-depth', type=int, default=5, help='Number of radial rings')
    parser.add_argument('--theta-divs', type=int, default=12, help='Number of angular divisions')
    parser.add_argument('--cell-size', type=float, default=50.0, help='Approximate cell size in pixels')
    parser.add_argument('--output', type=str, default=None, help='Output SVG file path')
    parser.add_argument('--no-labels', action='store_true', help='Hide coordinate labels')
    parser.add_argument('--no-indices', action='store_true', help='Hide flat indices')
    parser.add_argument('--highlight', type=str, default=None, 
                        help='Highlight a cell and its neighbors: "r,theta" (e.g., "2,5")')
    
    args = parser.parse_args()
    
    # Default output path
    if args.output is None:
        output_dir = Path(__file__).parent.parent / 'screenshots' / 'polar_grid'
        output_dir.mkdir(parents=True, exist_ok=True)
        if args.highlight:
            args.output = str(output_dir / f'polar_grid_r{args.r_min}_d{args.r_depth}_t{args.theta_divs}_highlight.svg')
        else:
            args.output = str(output_dir / f'polar_grid_r{args.r_min}_d{args.r_depth}_t{args.theta_divs}.svg')
    
    if args.highlight:
        # Parse highlight coordinates
        r, theta = map(int, args.highlight.split(','))
        svg_content = create_neighbor_highlight_svg(
            r_min=args.r_min,
            r_depth=args.r_depth,
            theta_divisions=args.theta_divs,
            highlight_r=r,
            highlight_theta=theta,
            cell_size=args.cell_size,
        )
    else:
        svg_content = create_polar_grid_svg(
            r_min=args.r_min,
            r_depth=args.r_depth,
            theta_divisions=args.theta_divs,
            show_labels=not args.no_labels,
            show_indices=not args.no_indices,
            cell_size=args.cell_size,
        )
    
    # Write output
    with open(args.output, 'w') as f:
        f.write(svg_content)
    
    print(f"SVG written to: {args.output}")
    print(f"Grid: r_min={args.r_min}, r_depth={args.r_depth}, theta_divisions={args.theta_divs}")
    print(f"Total cells: {args.r_depth * args.theta_divs}")


if __name__ == '__main__':
    main()
