#!/usr/bin/env python3
"""Generate DMG background image for Astrum macOS installer."""

import argparse
import math
import os

from PIL import Image, ImageDraw, ImageFont


def generate_squircle_points(cx, cy, width, height, radius, num_points=200):
    """Generate points for a squircle (rounded superellipse).

    This creates a rectangle with smoothly rounded corners using the
    squircle formula, not a pure superellipse.
    """
    points = []
    half_w = width / 2
    half_h = height / 2

    # Clamp radius
    radius = min(radius, half_w, half_h)

    def corner_point(angle, corner_cx, corner_cy, r):
        """Generate a point on a squircle corner."""
        # Use superellipse formula with n=5 for smoother corners
        n = 5
        cos_a = math.cos(angle)
        sin_a = math.sin(angle)
        x = corner_cx + r * abs(cos_a) ** (2/n) * (1 if cos_a >= 0 else -1)
        y = corner_cy + r * abs(sin_a) ** (2/n) * (1 if sin_a >= 0 else -1)
        return (x, y)

    steps_per_corner = num_points // 4

    # Top right corner
    corner_cx = cx + half_w - radius
    corner_cy = cy - half_h + radius
    for i in range(steps_per_corner):
        angle = -math.pi/2 + (math.pi/2) * i / steps_per_corner
        points.append(corner_point(angle, corner_cx, corner_cy, radius))

    # Bottom right corner
    corner_cx = cx + half_w - radius
    corner_cy = cy + half_h - radius
    for i in range(steps_per_corner):
        angle = 0 + (math.pi/2) * i / steps_per_corner
        points.append(corner_point(angle, corner_cx, corner_cy, radius))

    # Bottom left corner
    corner_cx = cx - half_w + radius
    corner_cy = cy + half_h - radius
    for i in range(steps_per_corner):
        angle = math.pi/2 + (math.pi/2) * i / steps_per_corner
        points.append(corner_point(angle, corner_cx, corner_cy, radius))

    # Top left corner
    corner_cx = cx - half_w + radius
    corner_cy = cy - half_h + radius
    for i in range(steps_per_corner):
        angle = math.pi + (math.pi/2) * i / steps_per_corner
        points.append(corner_point(angle, corner_cx, corner_cy, radius))

    return points


def draw_squircle(draw, bbox, fill, radius, num_points=200):
    """Draw a squircle on the given ImageDraw."""
    x1, y1, x2, y2 = bbox
    cx = (x1 + x2) / 2
    cy = (y1 + y2) / 2
    width = x2 - x1
    height = y2 - y1
    points = generate_squircle_points(cx, cy, width, height, radius, num_points)
    draw.polygon(points, fill=fill)


def generate_background(output_path: str, fonts_dir: str):
    """Generate the DMG background image."""
    # Scale factors
    SCALE = 1  # Final output scale
    AA_SCALE = 8  # Anti-aliasing scale (render at this multiple, then downscale)
    TOTAL_SCALE = SCALE * AA_SCALE

    # Create image at high resolution for anti-aliasing
    img = Image.new('RGB', (600 * TOTAL_SCALE, 580 * TOTAL_SCALE), color=(17, 15, 21))
    draw = ImageDraw.Draw(img)

    font = ImageFont.truetype(os.path.join(fonts_dir, 'Geist.ttf'), 14 * TOTAL_SCALE)
    mono_font = ImageFont.truetype(os.path.join(fonts_dir, 'GeistMono.ttf'), 13 * TOTAL_SCALE)

    text_color = (235, 232, 254)
    code_bg = (35, 32, 42)

    # Golden ratio line height: 12 * 1.618 ≈ 19
    line_height = int(12 * 1.618) * TOTAL_SCALE
    # Center text vertically - total text block is about 70px tall
    y_start = ((500 - 70) // 2) * TOTAL_SCALE

    # Line 1: 'You may need to run [command]'
    prefix = 'You may need to run '
    command = 'xattr -cr /Applications/Astrum.app'

    bbox_prefix = draw.textbbox((0, 0), prefix, font=font)
    prefix_width = bbox_prefix[2] - bbox_prefix[0]

    bbox_cmd = draw.textbbox((0, 0), command, font=mono_font)
    cmd_width = bbox_cmd[2] - bbox_cmd[0]
    cmd_height = bbox_cmd[3] - bbox_cmd[1]

    gap = 6 * TOTAL_SCALE
    total_width = prefix_width + gap + cmd_width
    x_start = (600 * TOTAL_SCALE - total_width) // 2
    y1 = y_start

    # Draw prefix
    draw.text((x_start, y1), prefix, fill=text_color, font=font)

    # Draw command with squircle background
    cmd_x = x_start + prefix_width + gap
    h_padding = 4 * TOTAL_SCALE
    v_padding = 3 * TOTAL_SCALE
    box_y_offset = 3 * TOTAL_SCALE

    draw_squircle(
        draw,
        [cmd_x - h_padding, y1 - v_padding + box_y_offset, cmd_x + cmd_width + h_padding, y1 + cmd_height + v_padding + box_y_offset],
        fill=code_bg,
        radius=12 * TOTAL_SCALE
    )
    draw.text((cmd_x, y1), command, fill=text_color, font=mono_font)

    # Line 2: 'to unquarantine the app.'
    line2 = 'to unquarantine the app.'
    bbox2 = draw.textbbox((0, 0), line2, font=font)
    x2 = (600 * TOTAL_SCALE - (bbox2[2] - bbox2[0])) // 2
    y2 = y1 + line_height
    draw.text((x2, y2), line2, fill=text_color, font=font)

    # Line 3 (with gap)
    line3 = 'Alternatively, run the included unquarantine.command script below.'
    bbox3 = draw.textbbox((0, 0), line3, font=font)
    x3 = (600 * TOTAL_SCALE - (bbox3[2] - bbox3[0])) // 2
    y3 = y2 + line_height + 8 * TOTAL_SCALE
    draw.text((x3, y3), line3, fill=text_color, font=font)

    # Downscale for anti-aliasing
    img = img.resize((600 * SCALE, 580 * SCALE), Image.Resampling.LANCZOS)

    img.save(output_path)
    print(f'Created {output_path}')


def main():
    parser = argparse.ArgumentParser(description='Generate DMG background image')
    parser.add_argument('output', help='Output path for the PNG file')
    parser.add_argument('fonts_dir', help='Path to the fonts directory containing Geist and GeistMono')
    args = parser.parse_args()

    generate_background(args.output, args.fonts_dir)


if __name__ == '__main__':
    main()
