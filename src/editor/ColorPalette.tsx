import type { Editor } from '@tiptap/core';
import { Check, Highlighter } from 'lucide-react';
import { useEffect, useId, useLayoutEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';

const TEXT_COLORS = [
  ['Red', '#dc2626'],
  ['Orange', '#c2410c'],
  ['Gold', '#a16207'],
  ['Green', '#15803d'],
  ['Teal', '#0f766e'],
  ['Blue', '#2563eb'],
  ['Purple', '#7c3aed'],
  ['Pink', '#be185d'],
  ['Gray', '#4b5563'],
] as const;

const HIGHLIGHT_COLORS = [
  ['Yellow', '#fde68a'],
  ['Peach', '#fed7aa'],
  ['Rose', '#fecaca'],
  ['Mint', '#bbf7d0'],
  ['Sky', '#bae6fd'],
  ['Lavender', '#ddd6fe'],
  ['Gray', '#d1d5db'],
] as const;

interface Position {
  left: number;
  top: number;
}

export function ColorPalette({ editor, kind }: { editor: Editor; kind: 'text' | 'highlight' }) {
  const [open, setOpen] = useState(false);
  const [position, setPosition] = useState<Position | null>(null);
  const trigger = useRef<HTMLButtonElement>(null);
  const popover = useRef<HTMLSpanElement>(null);
  const id = useId();
  const label = kind === 'text' ? 'Text color' : 'Highlight color';
  const textColor = String(editor.getAttributes('textStyle').color ?? '');
  const currentColor = kind === 'text' ? textColor : String(editor.getAttributes('highlight').color ?? '');

  useLayoutEffect(() => {
    if (!open) return;
    const updatePosition = () => {
      const anchor = trigger.current?.getBoundingClientRect();
      const menu = popover.current?.getBoundingClientRect();
      if (!anchor || !menu) return;
      const margin = 8;
      const gap = 6;
      const left = Math.min(
        Math.max(margin, anchor.left + anchor.width / 2 - menu.width / 2),
        window.innerWidth - menu.width - margin,
      );
      const below = anchor.bottom + gap;
      const top =
        below + menu.height <= window.innerHeight - margin
          ? below
          : Math.max(margin, anchor.top - menu.height - gap);
      setPosition({ left, top });
    };
    updatePosition();
    window.addEventListener('resize', updatePosition);
    window.addEventListener('scroll', updatePosition, true);
    return () => {
      window.removeEventListener('resize', updatePosition);
      window.removeEventListener('scroll', updatePosition, true);
    };
  }, [open]);

  useEffect(() => {
    if (!open) return;
    const close = (event: PointerEvent) => {
      const target = event.target as Node;
      if (!trigger.current?.contains(target) && !popover.current?.contains(target)) setOpen(false);
    };
    window.addEventListener('pointerdown', close);
    return () => window.removeEventListener('pointerdown', close);
  }, [open]);

  const apply = (color: string | null) => {
    const chain = editor.chain().focus();
    if (kind === 'text') {
      if (color) chain.setColor(color).run();
      else chain.unsetColor().run();
    } else if (color) {
      chain.setColor('#1f2937').setHighlight({ color }).run();
    } else {
      chain.unsetHighlight().run();
    }
    setOpen(false);
  };

  return (
    <span className="color-palette">
      <button
        ref={trigger}
        type="button"
        className={`toolbar-button ${open ? 'active' : ''}`}
        aria-label={label}
        aria-expanded={open}
        aria-controls={open ? id : undefined}
        aria-haspopup="dialog"
        title={label}
        onMouseDown={(event) => event.preventDefault()}
        onClick={() => setOpen((value) => !value)}
      >
        {kind === 'text' ? (
          <span className="text-color-indicator" style={{ background: textColor || 'var(--ink)' }}>
            A
          </span>
        ) : (
          <Highlighter />
        )}
      </button>
      {open &&
        createPortal(
          <span
            ref={popover}
            id={id}
            className="palette-popover"
            role="dialog"
            aria-label={label}
            style={{ left: position?.left ?? 8, top: position?.top ?? 8 }}
            onKeyDown={(event) => {
              if (event.key === 'Escape') {
                event.stopPropagation();
                setOpen(false);
                trigger.current?.focus({ preventScroll: true });
              }
            }}
          >
            <strong>{label}</strong>
            <span className="palette-swatches">
              {(kind === 'text' ? TEXT_COLORS : HIGHLIGHT_COLORS).map(([name, color]) => (
                <button
                  type="button"
                  key={color}
                  className="palette-swatch"
                  title={name}
                  aria-label={`${label}: ${name}`}
                  aria-pressed={currentColor.toLowerCase() === color}
                  style={{ background: color }}
                  onMouseDown={(event) => event.preventDefault()}
                  onClick={() => apply(color)}
                >
                  {currentColor.toLowerCase() === color && <Check />}
                </button>
              ))}
            </span>
            <button
              type="button"
              className="palette-default"
              onMouseDown={(event) => event.preventDefault()}
              onClick={() => apply(null)}
            >
              {kind === 'text' ? 'Automatic color' : 'Remove highlight'}
            </button>
            <label className="palette-custom">
              Custom color
              <input
                type="color"
                aria-label={`Custom ${label.toLowerCase()}`}
                value={
                  /^#[0-9a-f]{6}$/i.test(currentColor)
                    ? currentColor
                    : kind === 'text'
                      ? '#2563eb'
                      : '#fde68a'
                }
                onChange={(event) => apply(event.target.value)}
              />
            </label>
          </span>,
          document.body,
        )}
    </span>
  );
}
