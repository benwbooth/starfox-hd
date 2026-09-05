# Application icon

Generated with the built-in image-generation tool. This is project artwork,
not a retail ROM asset. The source PNG is preserved; the 512-pixel PNG serves
desktop shells and the 256-pixel RGBA BMP is embedded in the executable so
SDL can decode it without another image library or runtime asset lookup.

Generation prompt:

> Use case: logo-brand. Asset type: desktop game application icon for Star Fox HD, a modern Rust port of the original SNES Star Fox games. Create one square 1024 by 1024 icon: a bold, recognizable silver-white and blue low-poly Arwing starfighter, seen from slightly above and in front, nose pointing down toward the viewer, broad swept triangular wings fully visible. Crisp angular SNES-inspired faceted geometry, a small blue canopy, restrained blue engine glow. Centered strong silhouette fills most of the canvas with safe padding. Deep midnight-navy rounded-square badge behind the ship; genuinely transparent outside the badge. Keep the design clean, high contrast and readable at 32 pixels. No text, letters, watermark, border inscription, extra ships, stars or scenery. Finished application icon, not a mockup.

Rebuild the derived sizes with ImageMagick from this directory:

```sh
magick starfox-hd-source.png -resize 512x512 starfox-hd.png
magick starfox-hd-source.png -resize 256x256 -define bmp:format=bmp4 starfox-hd.bmp
```

The window app ID and installed desktop filename must both remain
`io.github.benwbooth.starfox-hd`. Wayland shells use this association for the
taskbar icon, even when setting a window icon directly is unsupported.
The Nix package installs the desktop entry and PNG into the standard paths.
The development checkout's local launcher can use an absolute icon path,
`Path` set to the repo root, and `Exec=nix develop /path/to/starfox-hd --command
/path/to/starfox-hd/scripts/run.sh` (on one line). The package launcher runs
the installed binary; ROM/config/assets remain user-supplied.
