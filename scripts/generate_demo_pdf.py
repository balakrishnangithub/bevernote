"""Generate a two-page public PDF used only by the browser demonstration."""
from pathlib import Path
from reportlab.pdfgen import canvas

root = Path(__file__).resolve().parents[1]
target = root / "public" / "demo-attachments" / "demo-pdf"
target.parent.mkdir(parents=True, exist_ok=True)
pdf = canvas.Canvas(str(target))
for page in (1, 2):
    pdf.setTitle("Bevernote local PDF preview")
    pdf.setFont("Helvetica-Bold", 24)
    pdf.drawString(72, 730, "Bevernote local PDF preview")
    pdf.setFont("Helvetica", 14)
    pdf.drawString(72, 690, f"Page {page} of 2")
    pdf.drawString(72, 650, "This public sample is rendered entirely on your device.")
    pdf.showPage()
pdf.save()
print(target)
