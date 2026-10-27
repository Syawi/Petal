"""Compose README visuals from frames exported by Petal's native renderer."""
from pathlib import Path
import os
from PIL import Image, ImageDraw, ImageFont

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "docs/media"
FRAMES = ROOT / "target/documentation-frames"
SNAPSHOTS = ROOT / "target/visual-checks"
FONTS = Path(os.environ.get("WINDIR", r"C:\Windows")) / "Fonts"
OUT.mkdir(parents=True, exist_ok=True)

def text(draw, xy, value, size=20, color="#A7B3C3", bold=False):
    face = "segoeuib.ttf" if bold else "segoeui.ttf"
    draw.text(xy, value, fill=color, font=ImageFont.truetype(str(FONTS / face), size))

def native(path):
    return Image.open(path).convert("RGBA")

hero = Image.new("RGBA", (1440, 640), "#0C1118")
hero.alpha_composite(native(FRAMES / "frame-035.png"), (136, 120))
hero.alpha_composite(native(ROOT / "assets/petal.png").resize((56, 56), Image.Resampling.LANCZOS), (704, 138))
draw = ImageDraw.Draw(hero)
text(draw, (782, 130), "Petal", 62, "#F7FAFF", True)
text(draw, (706, 246), "Convert at a click.", 40, "#F7FAFF", True)
text(draw, (708, 318), "Images, media and documents.", 24)
text(draw, (708, 355), "One native menu, right where you work.", 24)
text(draw, (708, 464), "WINDOWS 11   /   RUST   /   MIT", 17, "#7FAEFF", True)
hero.convert("RGB").save(OUT / "hero.png", optimize=True)

themes = Image.new("RGBA", (1120, 560), "#F1F3F6")
themes.paste("#0C1118", (560, 0, 1120, 560))
themes.alpha_composite(native(SNAPSHOTS / "light.png"), (80, 100))
themes.alpha_composite(native(SNAPSHOTS / "dark.png"), (640, 100))
draw = ImageDraw.Draw(themes)
text(draw, (48, 30), "Light", 26, "#172130", True)
text(draw, (608, 30), "Dark", 26, "#F7FAFF", True)
themes.convert("RGB").save(OUT / "themes.png", optimize=True)

frames = []
for i in range(91):
    frame = Image.new("RGBA", (1040, 520), "#0C1118")
    frame.alpha_composite(native(FRAMES / f"frame-{i:03}.png"), (76, 60))
    draw = ImageDraw.Draw(frame)
    title = "Open the wheel" if i < 24 else "Choose a format" if i < 40 else "Converting..." if i < 64 else "Conversion complete"
    note = "Right-click a supported file." if i < 24 else "Move to a petal and click." if i < 40 else "Petal checks the result before replacement." if i < 64 else "A check and an optional completion sound."
    text(draw, (542, 154), "PETAL", 17, "#7FAEFF", True)
    text(draw, (540, 196), title, 32, "#F7FAFF", True)
    text(draw, (542, 258), note, 19)
    text(draw, (542, 402), "Preview from the native renderer", 15, "#738299")
    frames.append(frame.convert("RGB"))
# One global palette prevents quantization flicker between animation frames.
atlas = Image.new("RGB", (1040, 520 * 4))
for row, index in enumerate([15, 35, 50, 78]):
    atlas.paste(frames[index], (0, row * 520))
palette = atlas.quantize(colors=256)
indexed = [f.quantize(palette=palette, dither=Image.Dither.NONE) for f in frames]
indexed[0].save(OUT / "demo.gif", save_all=True, append_images=indexed[1:], duration=40, loop=0, optimize=True, disposal=2)
print("Exported hero.png, themes.png and demo.gif from native frames.")
