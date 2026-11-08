#!/usr/bin/env python3
"""Draws assets/AppIcon.icns from the same mark the sidebar paints.

The sidebar's logo() is a 30pt rounded square: yellow fill, 2.5 border, a
2,3 offset shadow and three bars of height 8/14/10. Everything here is that
mark scaled up, so the two stay the same drawing.

One thing differs on purpose. In the dark theme the sidebar's border and
shadow are cream (--ink is light there), which is invisible on a pale
desktop. A launcher icon has no theme to sit in, so the border, bars and
shadow are all the dark ink: the only combination that reads on any
background, and the same one the kit's light theme would give the border.

Run: python3 tools/make-icon.py      (needs Pillow; iconutil ships with macOS)
"""
import pathlib
import shutil
import subprocess
import tempfile

from PIL import Image, ImageDraw

HERE = pathlib.Path(__file__).resolve().parent.parent
YELLOW = (0xF2, 0xC3, 0x1A, 255)   # --y2
INK = (0x14, 0x14, 0x0F, 255)      # --ink, light theme
SS = 4                             # supersample, for clean curves

# The mark's own proportions, in the logo's 30 units.
U_SIZE, U_RADIUS, U_BORDER = 30.0, 9.0, 2.5
U_OFFSET = (2.0, 3.0)
U_BAR_W, U_BAR_R, U_BAR_GAP = 3.0, 1.5, 5.0
U_BARS = (8.0, 14.0, 10.0)


def draw(px: int) -> Image.Image:
    """The mark on a transparent square of `px`, shadow and all."""
    n = px * SS
    # Mark plus its offset shadow, fitted to the canvas and centred as one
    # shape - centring the mark alone would hang the shadow off the edge.
    mark = n * 0.742
    u = mark / U_SIZE
    off = (U_OFFSET[0] * u, U_OFFSET[1] * u)
    x0 = (n - (mark + off[0])) / 2
    y0 = (n - (mark + off[1])) / 2

    im = Image.new("RGBA", (n, n), (0, 0, 0, 0))
    d = ImageDraw.Draw(im)
    r = U_RADIUS * u
    d.rounded_rectangle([x0 + off[0], y0 + off[1], x0 + off[0] + mark, y0 + off[1] + mark], r, fill=INK)
    d.rounded_rectangle([x0, y0, x0 + mark, y0 + mark], r, fill=YELLOW,
                        outline=INK, width=round(U_BORDER * u))

    cx, cy = x0 + mark / 2, y0 + mark / 2
    bw, br = U_BAR_W * u, U_BAR_R * u
    for i, bh in enumerate(U_BARS):
        x = cx + (i - 1) * U_BAR_GAP * u
        h = bh * u
        d.rounded_rectangle([x - bw / 2, cy - h / 2, x + bw / 2, cy + h / 2], br, fill=INK)
    return im.resize((px, px), Image.LANCZOS)


def main() -> None:
    out = HERE / "assets/AppIcon.icns"
    with tempfile.TemporaryDirectory() as tmp:
        iconset = pathlib.Path(tmp) / "AppIcon.iconset"
        iconset.mkdir()
        for base in (16, 32, 128, 256, 512):
            draw(base).save(iconset / f"icon_{base}x{base}.png")
            draw(base * 2).save(iconset / f"icon_{base}x{base}@2x.png")
        subprocess.run(["iconutil", "-c", "icns", str(iconset), "-o", str(out)], check=True)
        # A 1024 PNG beside it, for anywhere that cannot read an .icns.
        draw(1024).save(HERE / "assets/AppIcon.png")
    print(f"{out}  {out.stat().st_size} bytes")
    if shutil.which("iconutil") is None:  # pragma: no cover
        print("iconutil not found")


if __name__ == "__main__":
    main()
