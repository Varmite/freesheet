"""Makes the Android launcher icons and splash screens from build-resources/icon.png.

Run from anywhere:  python "Android Edition/scripts/make-icons.py"   (needs Pillow)
"""
from pathlib import Path
from PIL import Image, ImageDraw

HERE = Path(__file__).resolve().parent.parent
RES = HERE / "android" / "app" / "src" / "main" / "res"
SRC = Image.open(HERE.parent / "build-resources" / "icon.png").convert("RGBA")
DARK = (29, 29, 27, 255)  # the icon's tile colour
SHEET = SRC.crop((262, 180, 763, 844))  # the white page with its lines

DENSITIES = {"mdpi": 1, "hdpi": 1.5, "xhdpi": 2, "xxhdpi": 3, "xxxhdpi": 4}


def place_sheet(canvas, height_frac):
    w, h = canvas.size
    sh = int(h * height_frac)
    sw = int(SHEET.width * sh / SHEET.height)
    s = SHEET.resize((sw, sh), Image.LANCZOS)
    canvas.alpha_composite(s, ((w - sw) // 2, (h - sh) // 2))
    return canvas


def save(img, path):
    path.parent.mkdir(parents=True, exist_ok=True)
    img.save(path)


for name, d in DENSITIES.items():
    folder = RES / f"mipmap-{name}"
    legacy = int(48 * d)
    # Old launchers: the full tile.
    save(SRC.resize((legacy, legacy), Image.LANCZOS), folder / "ic_launcher.png")
    # Round launchers: a dark circle with the page.
    big = legacy * 4
    circle = Image.new("RGBA", (big, big), (0, 0, 0, 0))
    ImageDraw.Draw(circle).ellipse((0, 0, big - 1, big - 1), fill=DARK)
    place_sheet(circle, 0.56)
    save(circle.resize((legacy, legacy), Image.LANCZOS), folder / "ic_launcher_round.png")
    # Adaptive icon foreground: 108dp canvas, the page inside the 66dp safe zone.
    fg = int(108 * d)
    save(place_sheet(Image.new("RGBA", (fg, fg), (0, 0, 0, 0)), 0.5), folder / "ic_launcher_foreground.png")

# Splash screens (shown while the app starts): the page on the dark tile colour.
for png in RES.glob("drawable*/splash.png"):
    w, h = Image.open(png).size
    img = Image.new("RGBA", (w, h), DARK)
    side = min(w, h)
    sh = int(side * 0.28)
    sw = int(SHEET.width * sh / SHEET.height)
    img.alpha_composite(SHEET.resize((sw, sh), Image.LANCZOS), ((w - sw) // 2, (h - sh) // 2))
    img.convert("RGB").save(png)

(RES / "values" / "ic_launcher_background.xml").write_text(
    '<?xml version="1.0" encoding="utf-8"?>\n<resources>\n    <color name="ic_launcher_background">#1D1D1B</color>\n</resources>\n',
    encoding="utf-8",
)
print("icons and splash screens written to", RES)
