The title bar, footer, palette, font setup and caption icon geometry are adapted
from Craft Launcher by ArtCraft and its contributors, under its MIT license.

Source: https://github.com/storytold/craft-launcher
Revision: 0027613a909edfaa80eefd0b4027168dd3d5c876
Files: crates/ui-egui/src/{theme.rs,widgets.rs,views.rs,bar.rs}
License: LICENSE-Craft-Launcher-MIT.txt in this directory.

Sevi keeps its own name and supplied platform icons. No ArtCraft trademark,
application logos, launcher services or forked UI dependencies are included.
The fonts are distributed under their accompanying SIL Open Font Licenses.

To reduce the executable size, the embedded fonts are fixed instances of the
launcher variable fonts: Inter 400/500 at optical size 14 and Geist Mono
400/500/700. Archivo is no longer embedded, and the title bar has no wordmark.
Sevi uses tighter tracking and content-fitted settings. Every source glyph and
Unicode mapping is kept.
`font-instances.json` records source hashes, exact coordinates and coverage.
`generate_fonts.py` regenerates them with fonttools 4.68.0; Cargo needs no font
tools or network access. No language coverage is removed to save space.
