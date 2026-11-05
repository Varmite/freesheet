"""Draws the Windows installer artwork from build-resources/icon.png.

    python scripts/make-installer-art.py

Writes to build-resources/:
  icon.ico                 app, installer and uninstaller icon (16-256 px)
  installerSidebar.bmp     164x314 welcome/finish page image (also used by the uninstaller)
  installerHeader.bmp      150x57 header image on the inner pages
"""
import os
from PIL import Image, ImageDraw, ImageFont

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
RES = os.path.join(ROOT, "build-resources")
icon = Image.open(os.path.join(RES, "icon.png")).convert("RGBA")

BLUE = (31, 95, 204)
NAVY = (14, 34, 78)
WHITE = (255, 255, 255)
SOFT = (198, 214, 245)


def font(bold, size):
    name = "segoeuib.ttf" if bold else "segoeui.ttf"
    for p in (os.path.join(os.environ.get("WINDIR", "C:\\Windows"), "Fonts", name), name):
        try:
            return ImageFont.truetype(p, size)
        except OSError:
            pass
    return ImageFont.load_default()


def gradient(w, h, top, bottom):
    img = Image.new("RGB", (w, h))
    d = ImageDraw.Draw(img)
    for y in range(h):
        t = y / (h - 1)
        d.line([(0, y), (w, y)], fill=tuple(round(a + (b - a) * t) for a, b in zip(top, bottom)))
    return img


def centred(d, y, text, f, fill, w):
    tw = d.textlength(text, font=f)
    d.text(((w - tw) / 2, y), text, font=f, fill=fill)


# Draw at 3x and scale down for smooth edges.
S = 3

# ---- sidebar (164x314)
W, H = 164 * S, 314 * S
side = gradient(W, H, BLUE, NAVY)
d = ImageDraw.Draw(side)
# soft decorative circles
for cx, cy, r, a in ((150, 40, 70, 18), (20, 290, 90, 14), (140, 250, 40, 10)):
    layer = Image.new("RGBA", (W, H), (0, 0, 0, 0))
    ImageDraw.Draw(layer).ellipse([(cx - r) * S, (cy - r) * S, (cx + r) * S, (cy + r) * S], fill=(255, 255, 255, a))
    side = Image.alpha_composite(side.convert("RGBA"), layer).convert("RGB")
d = ImageDraw.Draw(side)
ic = icon.resize((76 * S, 76 * S), Image.LANCZOS)
side.paste(ic, ((W - ic.width) // 2, 58 * S), ic)
centred(d, 146 * S, "FreeSheet", font(True, 22 * S), WHITE, W)
centred(d, 178 * S, "Study notes, practice", font(False, 10 * S), SOFT, W)
centred(d, 193 * S, "and exams for", font(False, 10 * S), SOFT, W)
centred(d, 208 * S, "Grades 8 to 12", font(False, 10 * S), SOFT, W)
d.line([(52 * S, 238 * S), (112 * S, 238 * S)], fill=(255, 255, 255), width=S)
centred(d, 250 * S, "Works offline", font(True, 9 * S), WHITE, W)
centred(d, 264 * S, "CAPS curriculum", font(True, 9 * S), WHITE, W)
side = side.resize((164, 314), Image.LANCZOS)
side.save(os.path.join(RES, "installerSidebar.bmp"))

# ---- header (150x57), shown at the top right of white inner pages
W, H = 150 * S, 57 * S
head = Image.new("RGB", (W, H), WHITE)
d = ImageDraw.Draw(head)
ic = icon.resize((38 * S, 38 * S), Image.LANCZOS)
x0 = 14 * S
head.paste(ic, (x0, (H - ic.height) // 2), ic)
d.text((x0 + ic.width + 8 * S, 13 * S), "FreeSheet", font=font(True, 17 * S), fill=(29, 29, 27))
d.text((x0 + ic.width + 9 * S, 35 * S), "Grades 8 to 12", font=font(False, 8 * S), fill=BLUE)
head = head.resize((150, 57), Image.LANCZOS)
head.save(os.path.join(RES, "installerHeader.bmp"))

# ---- MSI (WiX) welcome/finish background, 493x312: artwork on the left 164 px, white text area on the right
dlg = Image.new("RGB", (493, 312), WHITE)
dlg.paste(side.crop((0, 0, 164, 312)), (0, 0))
dlg.save(os.path.join(RES, "msiDialog.bmp"))

# ---- MSI (WiX) banner, 493x58: WiX draws the page title on the left, so the logo goes on the right
ban = Image.new("RGB", (493 * S, 58 * S), WHITE)
d = ImageDraw.Draw(ban)
ic = icon.resize((36 * S, 36 * S), Image.LANCZOS)
fx = 493 * S - 14 * S - int(d.textlength("FreeSheet", font=font(True, 15 * S)))
ban.paste(ic, (fx - ic.width - 8 * S, (58 * S - ic.height) // 2), ic)
d.text((fx, 18 * S), "FreeSheet", font=font(True, 15 * S), fill=(29, 29, 27))
ban = ban.resize((493, 58), Image.LANCZOS)
ban.save(os.path.join(RES, "msiBanner.bmp"))

# ---- icon
icon.save(os.path.join(RES, "icon.ico"), sizes=[(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)])
print("Wrote icon.ico, installerSidebar.bmp, installerHeader.bmp")
