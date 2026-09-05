# Application icon

Revised with the built-in image-generation tool using the port's actual
`SHAPE_MYSHIP_4` player mesh as the shape/color reference. This is generated
icon artwork, not an exact renderer capture. The source PNG is preserved;
the 512-pixel PNG serves
desktop shells and the 256-pixel RGBA BMP is embedded in the executable so
SDL can decode it without another image library or runtime asset lookup.

Revision prompt (image 1: previous icon; image 2: `arwing-model-reference.png`):

> Use case: precise-object-edit. Revise the application icon in image 1. Image 1 is the EDIT TARGET, but its ship design is wrong. Image 2 is the PRIMARY SHIP REFERENCE, rendered directly from the actual SNES player mesh in this game's renderer. Replace the detailed spaceship in image 1 with the extremely simple low-poly Arwing in image 2. Match image 2's silhouette, proportions, pose and few large flat polygon faces closely: long pointed pale gray central fuselage with two shaded sides and nose pointing down; two thin long upright blue/cyan angular fins flanking the central body; small narrow swept triangular silver wings extending outward behind them. No canopy is visible in this pose. Keep the asymmetric light/dark face colors as in the reference. Clean crisp polygon edges, flat-shaded game geometry, absolutely no added panel lines, vents, insignia, red decals, cockpit glass, armor, bevels, extra appendages or photorealistic details. Enlarge this exact simple ship to occupy about 80 percent of the icon, centered with padding. Preserve the navy rounded-square badge concept and genuine transparency outside it from image 1, but soften its glow so the very simple game mesh is the focus. No text, no watermarks, no scene, no invented detail. This should look like the actual in-game polygon model placed on a desktop icon, not a modern redesign.

To reproduce the mesh reference from the repo root:

```sh
nix develop --command cargo run --manifest-path rust/Cargo.toml \
  -p sf-render --example arwing_icon_reference -- /tmp/arwing-reference.ppm
magick /tmp/arwing-reference.ppm -trim +repage -bordercolor black -border 20 \
  -resize 768x768 rust/sf-app/assets/arwing-model-reference.png
```

Final transparency-edit prompt, also using built-in image generation:

> Extract the blue rounded-square application badge from this image. Remove every black pixel of background outside the badge. Deliver an actual RGBA PNG cutout with alpha=0 in the four corners and all outside areas. Transparency is essential: do not paint a transparency checkerboard, white, or black background. The checkerboard preview should not be part of the image. Keep the blue badge and extremely simple flat-shaded polygon spacecraft unchanged. Do not redesign, reinterpret, redraw or add any detail. Only the exterior background must become genuinely transparent.

The previous icon and prompt remain available in Git commit `c63d7f8`.

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
