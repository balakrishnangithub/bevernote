"""Regenerate the tiny, public OCR test fixture used by the Rust integration test."""
from pathlib import Path
from PIL import Image, ImageDraw, ImageFont

root = Path(__file__).resolve().parents[1]
target = root / "crates" / "bevernote-core" / "tests" / "fixtures" / "ocr-invoice.png"
target.parent.mkdir(parents=True, exist_ok=True)
image = Image.new("RGB", (1100, 260), "white")
draw = ImageDraw.Draw(image)
font = ImageFont.load_default(size=64)
draw.text((55, 55), "INVOICE 2048", fill="black", font=font)
draw.text((55, 155), "Total 42 dollars", fill="black", font=font)
image.save(target, optimize=True)
print(target)
