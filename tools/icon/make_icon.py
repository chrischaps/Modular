"""Draws Modular's app icon: one glowing cable between two jacks, the way
the patch canvas draws them, on the canvas's dark grid. Blue is audio, the
signal leaving a source; purple is the output it arrives at.

    python tools/icon/make_icon.py

Writes assets/icon/: modular.png (1024 px), icon-256.png (the window icon),
modular.ico (Windows: the exe, the Start Menu shortcut, the installer) and
modular.icns (the macOS app bundle).
"""

import math
from pathlib import Path

from PIL import Image, ImageChops, ImageDraw, ImageFilter

SIZE = 1024
SS = 2  # drawn at twice the size, then scaled down, for smooth edges
OUT = Path(__file__).resolve().parents[2] / "assets" / "icon"

# The theme's colours (src/app/theme.rs)
BG_TOP = (38, 38, 62)
BG_BOTTOM = (20, 20, 38)
GRID = (46, 46, 74)
BLUE = (66, 165, 245)  # accent::PRIMARY, the audio cable
PURPLE = (155, 105, 230)  # the Output module's header
METAL_LIGHT = (176, 184, 198)
METAL_DARK = (58, 62, 78)
HOLE = (14, 14, 24)


def lerp(a, b, t):
    return tuple(round(x + (y - x) * t) for x, y in zip(a, b))


def rounded_mask(size, radius):
    mask = Image.new("L", (size, size), 0)
    ImageDraw.Draw(mask).rounded_rectangle([0, 0, size - 1, size - 1], radius, fill=255)
    return mask


def bezier(p0, p1, p2, p3, steps=400):
    for i in range(steps + 1):
        t = i / steps
        u = 1 - t
        x = u**3 * p0[0] + 3 * u * u * t * p1[0] + 3 * u * t * t * p2[0] + t**3 * p3[0]
        y = u**3 * p0[1] + 3 * u * u * t * p1[1] + 3 * u * t * t * p2[1] + t**3 * p3[1]
        yield t, (x, y)


def stroke(draw, points, width, color_at):
    """A thick stroke along `points`, coloured by position, as round dabs."""
    r = width / 2
    for t, (x, y) in points:
        draw.ellipse([x - r, y - r, x + r, y + r], fill=color_at(t))


def draw():
    s = SIZE * SS
    k = SS

    # The canvas: a soft vertical gradient with the patch grid over it
    bg = Image.new("RGB", (s, s))
    px = ImageDraw.Draw(bg)
    for y in range(s):
        px.line([(0, y), (s, y)], fill=lerp(BG_TOP, BG_BOTTOM, y / s))
    grid = ImageDraw.Draw(bg)
    step = 64 * k
    for i in range(0, s, step):
        grid.line([(i, 0), (i, s)], fill=lerp(GRID, BG_BOTTOM, 0.25), width=2 * k)
        grid.line([(0, i), (s, i)], fill=lerp(GRID, BG_BOTTOM, 0.25), width=2 * k)
    # A vignette, so the grid fades toward the corners
    vignette = Image.new("L", (s, s), 0)
    ImageDraw.Draw(vignette).ellipse([-s * 0.2, -s * 0.2, s * 1.2, s * 1.2], fill=255)
    vignette = vignette.filter(ImageFilter.GaussianBlur(160 * k))
    dark = Image.new("RGB", (s, s), BG_BOTTOM)
    bg = Image.composite(bg, dark, vignette)

    # The cable, from a jack low on the left to one high on the right
    a = (262 * k, 668 * k)
    b = (762 * k, 356 * k)
    pull = 300 * k
    points = list(bezier(a, (a[0] + pull, a[1]), (b[0] - pull, b[1]), b))
    color = lambda t: lerp(BLUE, PURPLE, t)

    glow = Image.new("RGB", (s, s), (0, 0, 0))
    stroke(ImageDraw.Draw(glow), points, 120 * k, color)
    glow = glow.filter(ImageFilter.GaussianBlur(70 * k))
    bg = ImageChops.add(bg, ImageChops.multiply(glow, Image.new("RGB", (s, s), (150, 150, 150))))

    cable = Image.new("RGBA", (s, s), (0, 0, 0, 0))
    cd = ImageDraw.Draw(cable)
    stroke(cd, points, 80 * k, lambda t: lerp(color(t), (10, 10, 20), 0.45) + (255,))  # its shadowed edge
    stroke(cd, points, 66 * k, lambda t: color(t) + (255,))
    # A soft highlight along its top, as on the canvas
    sheen = Image.new("RGBA", (s, s), (0, 0, 0, 0))
    lifted = [(t, (x - 5 * k, y - 12 * k)) for t, (x, y) in points]
    stroke(ImageDraw.Draw(sheen), lifted, 16 * k, lambda t: lerp(color(t), (255, 255, 255), 0.45) + (120,))
    cable = Image.alpha_composite(cable, sheen.filter(ImageFilter.GaussianBlur(5 * k)))
    bg = Image.alpha_composite(bg.convert("RGBA"), cable)

    # The jacks: brushed steel lit from the top left, a ring of the
    # cable's colour, and the hole the plug sits in
    for centre, tint in ((a, BLUE), (b, PURPLE)):
        cx, cy = centre
        r = 96 * k
        box = [cx - r, cy - r, cx + r, cy + r]
        shadow = Image.new("RGBA", (s, s), (0, 0, 0, 0))
        ImageDraw.Draw(shadow).ellipse([box[0] - 6 * k, box[1] + 4 * k, box[2] + 6 * k, box[3] + 22 * k], fill=(4, 4, 12, 190))
        bg = Image.alpha_composite(bg, shadow.filter(ImageFilter.GaussianBlur(14 * k)))

        steel = Image.new("RGBA", (2 * r, 2 * r))
        sd = ImageDraw.Draw(steel)
        for i in range(4 * r):
            sd.line([(i, 0), (0, i)], fill=lerp(METAL_LIGHT, METAL_DARK, i / (4 * r)) + (255,))
        disc = Image.new("L", (2 * r, 2 * r), 0)
        ImageDraw.Draw(disc).ellipse([0, 0, 2 * r - 1, 2 * r - 1], fill=255)
        bg.paste(steel, (cx - r, cy - r), disc)

        jd = ImageDraw.Draw(bg)
        bevel = 80 * k
        jd.ellipse([cx - bevel, cy - bevel, cx + bevel, cy + bevel], fill=lerp(METAL_DARK, (20, 20, 32), 0.5))
        ring = 66 * k
        jd.ellipse([cx - ring, cy - ring, cx + ring, cy + ring], fill=tint)
        hole = 46 * k
        jd.ellipse([cx - hole, cy - hole, cx + hole, cy + hole], fill=HOLE)
        # A thin crescent of light on the rim
        jd.arc([cx - r + 3 * k, cy - r + 3 * k, cx + r - 3 * k, cy + r - 3 * k], 190, 280, fill=(235, 240, 248), width=5 * k)

    # Cut to the rounded square, with a hairline edge
    icon = bg.resize((SIZE, SIZE), Image.LANCZOS)
    edge = ImageDraw.Draw(icon)
    edge.rounded_rectangle([1, 1, SIZE - 2, SIZE - 2], 224, outline=(80, 80, 120, 255), width=3)
    icon.putalpha(rounded_mask(SIZE, 224))
    return icon


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    icon = draw()
    icon.save(OUT / "modular.png")
    icon.resize((256, 256), Image.LANCZOS).save(OUT / "icon-256.png", optimize=True)
    icon.save(OUT / "modular.ico", sizes=[(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)])
    icon.save(OUT / "modular.icns")
    for f in sorted(OUT.iterdir()):
        print(f"{f.name}: {f.stat().st_size // 1024} KB")


if __name__ == "__main__":
    main()
