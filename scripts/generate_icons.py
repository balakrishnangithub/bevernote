"""Generate Bevernote's original beaver icon in the formats Tauri needs."""

from pathlib import Path

from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / "src-tauri" / "icons"
OUTPUT.mkdir(parents=True, exist_ok=True)
SCALE = 8
image = Image.new("RGBA", (128 * SCALE, 128 * SCALE), (0, 0, 0, 0))
draw = ImageDraw.Draw(image)


def rectangle(box, radius, fill):
    draw.rounded_rectangle(tuple(round(value * SCALE) for value in box), radius=radius * SCALE, fill=fill)


def ellipse(box, fill):
    draw.ellipse(tuple(round(value * SCALE) for value in box), fill=fill)


def line(points, fill, width):
    draw.line([(round(x * SCALE), round(y * SCALE)) for x, y in points], fill=fill, width=width * SCALE, joint="curve")


rectangle((0, 0, 127, 127), 30, "#256c52")
# Small, rounded ears frame a broad beaver face.
ellipse((22, 20, 52, 50), "#70432e")
ellipse((76, 20, 106, 50), "#70432e")
ellipse((29, 27, 45, 43), "#b9855e")
ellipse((83, 27, 99, 43), "#b9855e")
ellipse((23, 22, 105, 111), "#a96f47")
# Eyes stay large enough to survive a 16-pixel taskbar icon.
ellipse((39, 47, 49, 59), "#27241f")
ellipse((79, 47, 89, 59), "#27241f")
ellipse((41, 48, 44, 51), "#fff8e8")
ellipse((81, 48, 84, 51), "#fff8e8")
ellipse((32, 61, 66, 91), "#f1d5aa")
ellipse((62, 61, 96, 91), "#f1d5aa")
rectangle((51, 60, 77, 73), 7, "#302820")
line([(64, 72), (64, 81)], "#70432e", 2)
line([(50, 80), (57, 83), (64, 81), (71, 83), (78, 80)], "#70432e", 2)
rectangle((53, 81, 64, 101), 2, "#fff8e8")
rectangle((65, 81, 76, 101), 2, "#fff8e8")
line([(64.5, 84), (64.5, 98)], "#d9bd94", 1)
image.save(OUTPUT / "icon.png")
image.save(
    OUTPUT / "icon.ico",
    sizes=[(16, 16), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)],
)
for side in (32, 128):
    image.resize((side, side), Image.Resampling.LANCZOS).save(OUTPUT / f"{side}x{side}.png")
print(OUTPUT / "icon.png")
