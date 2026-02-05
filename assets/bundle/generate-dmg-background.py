from PIL import Image, ImageDraw, ImageFont

# Create image with background color #110F15
img = Image.new('RGB', (600, 400), color=(17, 15, 21))
draw = ImageDraw.Draw(img)

import os

script_dir = os.path.dirname(os.path.abspath(__file__))
fonts_dir = os.path.join(script_dir, '..', 'app', 'fonts', 'geist_1.7.0', 'fonts')

font = ImageFont.truetype(os.path.join(fonts_dir, 'Geist', 'ttf', 'Geist-Medium.ttf'), 14)
mono_font = ImageFont.truetype(os.path.join(fonts_dir, 'GeistMono', 'ttf', 'GeistMono-Regular.ttf'), 13)

text_color = (235, 232, 254)
code_bg = (35, 32, 42)

# Golden ratio line height: 12 * 1.618 ≈ 19
line_height = int(12 * 1.618)
y_start = 400 - 95

# Line 1: 'You may need to run [command]'
prefix = 'You may need to run '
command = 'xattr -cr /Applications/Astrum.app'

bbox_prefix = draw.textbbox((0, 0), prefix, font=font)
prefix_width = bbox_prefix[2] - bbox_prefix[0]

bbox_cmd = draw.textbbox((0, 0), command, font=mono_font)
cmd_width = bbox_cmd[2] - bbox_cmd[0]
cmd_height = bbox_cmd[3] - bbox_cmd[1]

gap = 6
total_width = prefix_width + gap + cmd_width
x_start = (600 - total_width) // 2
y1 = y_start

# Draw prefix
draw.text((x_start, y1), prefix, fill=text_color, font=font)

# Draw command with background
cmd_x = x_start + prefix_width + gap
h_padding = 4
v_padding = 3
draw.rounded_rectangle(
    [cmd_x - h_padding, y1 - v_padding + 3, cmd_x + cmd_width + h_padding, y1 + cmd_height + v_padding + 3],
    radius=3,
    fill=code_bg
)
draw.text((cmd_x, y1), command, fill=text_color, font=mono_font)

# Line 2: 'in order to unquarantine the app.'
line2 = 'to unquarantine the app.'
bbox2 = draw.textbbox((0, 0), line2, font=font)
x2 = (600 - (bbox2[2] - bbox2[0])) // 2
y2 = y1 + line_height
draw.text((x2, y2), line2, fill=text_color, font=font)

# Line 3 (with gap)
line3 = 'Alternatively, run the included unquarantine.sh script.'
bbox3 = draw.textbbox((0, 0), line3, font=font)
x3 = (600 - (bbox3[2] - bbox3[0])) // 2
y3 = y2 + line_height + 8
draw.text((x3, y3), line3, fill=text_color, font=font)

img.save('/Volumes/T7/astrum_chat_main/astrum/assets/bundle/dmg-background.png')
print('Created dmg-background.png')
