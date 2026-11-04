import { ChevronLeft, ChevronRight, LoaderCircle, X } from 'lucide-react';
import { useEffect, useRef, useState } from 'react';
import type { PDFDocumentProxy, RenderTask } from 'pdfjs-dist';

interface Props {
  url: string;
  name: string;
  onClose: () => void;
}

/** Local, one-page-at-a-time renderer; no document data leaves the device. */
export function PdfPreview({ url, name, onClose }: Props) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [document, setDocument] = useState<PDFDocumentProxy | null>(null);
  const [pageNumber, setPageNumber] = useState(1);
  const [pageCount, setPageCount] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;
    let loaded: PDFDocumentProxy | undefined;
    void (async () => {
      try {
        const pdfjs = await import('pdfjs-dist');
        pdfjs.GlobalWorkerOptions.workerSrc = new URL(
          'pdfjs-dist/build/pdf.worker.min.mjs',
          import.meta.url,
        ).toString();
        const task = pdfjs.getDocument({ url, withCredentials: false });
        loaded = await task.promise;
        if (cancelled) {
          void task.destroy();
          return;
        }
        setPageCount(loaded.numPages);
        setDocument(loaded);
      } catch {
        if (!cancelled) {
          setError('This PDF could not be previewed. Use Open or Save As to access the original file.');
          setLoading(false);
        }
      }
    })();
    return () => {
      cancelled = true;
      if (loaded) void loaded.loadingTask.destroy();
    };
  }, [url]);

  useEffect(() => {
    if (!document) return;
    let cancelled = false;
    let renderTask: RenderTask | undefined;
    void (async () => {
      try {
        const page = await document.getPage(pageNumber);
        const canvas = canvasRef.current;
        const context = canvas?.getContext('2d', { alpha: false });
        if (cancelled) return;
        if (!canvas || !context) throw new Error('PDF canvas is unavailable.');
        const availableWidth = Math.min(canvas.parentElement?.clientWidth ?? 800, 1000);
        const base = page.getViewport({ scale: 1 });
        const scale = Math.max(0.5, Math.min(2, (availableWidth - 32) / base.width));
        const viewport = page.getViewport({ scale });
        const ratio = Math.min(window.devicePixelRatio || 1, 2);
        canvas.width = Math.floor(viewport.width * ratio);
        canvas.height = Math.floor(viewport.height * ratio);
        canvas.style.width = `${Math.floor(viewport.width)}px`;
        canvas.style.height = `${Math.floor(viewport.height)}px`;
        renderTask = page.render({
          canvas,
          canvasContext: context,
          viewport,
          transform: ratio === 1 ? undefined : [ratio, 0, 0, ratio, 0, 0],
        });
        await renderTask.promise;
        if (!cancelled) {
          canvas.dataset.renderedPage = String(pageNumber);
          setLoading(false);
        }
      } catch (reason) {
        if (!cancelled && !String(reason).includes('Rendering cancelled')) {
          setError('This PDF could not be previewed. Use Open or Save As to access the original file.');
          setLoading(false);
        }
      }
    })();
    return () => {
      cancelled = true;
      renderTask?.cancel();
    };
  }, [document, pageNumber]);

  const goToPage = (nextPage: number) => {
    setLoading(true);
    setPageNumber(nextPage);
  };

  return (
    <div className="modal-backdrop pdf-backdrop" onMouseDown={onClose}>
      <section
        className="modal pdf-modal"
        role="dialog"
        aria-modal="true"
        aria-label={`Preview ${name}`}
        onMouseDown={(event) => event.stopPropagation()}
        onKeyDown={(event) => {
          if (event.key === 'Escape') {
            event.stopPropagation();
            onClose();
          }
        }}
      >
        <header>
          <strong>{name}</strong>
          <div className="pdf-controls">
            <button
              className="icon-button"
              disabled={loading || pageNumber <= 1}
              onClick={() => goToPage(pageNumber - 1)}
              aria-label="Previous PDF page"
            >
              <ChevronLeft />
            </button>
            <span>
              {pageNumber} / {pageCount || '…'}
            </span>
            <button
              className="icon-button"
              disabled={loading || !pageCount || pageNumber >= pageCount}
              onClick={() => goToPage(pageNumber + 1)}
              aria-label="Next PDF page"
            >
              <ChevronRight />
            </button>
            <button className="icon-button" onClick={onClose} aria-label="Close PDF preview">
              <X />
            </button>
          </div>
        </header>
        <div className="pdf-canvas-wrap" aria-busy={loading}>
          {loading && <LoaderCircle className="spin" aria-label="Loading PDF" />}
          {error && <p role="alert">{error}</p>}
          <canvas ref={canvasRef} aria-label={`${name}, page ${pageNumber}`} />
        </div>
      </section>
    </div>
  );
}
