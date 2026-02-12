# v0.1.4
## Features
- Interior corners of text selections now support corner rounding.

## Fixes
- Scrolling an input is now slower and doesn't just near-instantly scroll to the end.

## Fixes
- Only text shows the IBeam cursor, instead of the entire bounds showing it.
- Fixed broken position of the accent menu in the text input.

# v0.1.3
## Fixes
- Fixed issue with text selections sometimes being slightly clipped on the right edge.
- Fixed issue with text wrapping breaking when the text's bounds are resized quickly.
- Text selection's width is now rounded to the nearest screen pixel which ensures it is sharp.
- Fixed issue with the chat input's text disappearing under specific conditions.
- Fixed issues with selection corner radius logic.
- Fixed issue where selecting text at the start of a wrap boundry would also select the end of the previous wrap boundry.

# v0.1.2
## Features
- Auto-update via GitHub releases with background downloads and one-click restart.
- Cross-platform update support for macOS, Linux, and Windows.

## Fixes
- The Geist font now works for users who don't have it installed on their system.
- Chat messages now use the Geist font.

# v0.1.1
## Fixes
- Fixed text selection being wrongly clipped in word wrap mode.
- Fixed words sometimes disappearing instead of wrapping to the next line.

# v0.0.1
Initial release.
