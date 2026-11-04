import { save } from '@tauri-apps/plugin-dialog';
import { NodeViewWrapper, type NodeViewProps } from '@tiptap/react';
import {
  Download,
  ExternalLink,
  Eye,
  File,
  FileAudio,
  FileVideo,
  LockKeyhole,
  MoreHorizontal,
  Paperclip,
} from 'lucide-react';
import { lazy, Suspense, useState } from 'react';

import { api, isNative, resourceUrl } from '../lib/api';
import { useLibraryStore } from '../store/useLibraryStore';

const PdfPreview = lazy(() => import('./PdfPreview').then((module) => ({ default: module.PdfPreview })));

export function AttachmentView({ node }: NodeViewProps) {
  const selectedNote = useLibraryStore((state) => state.selectedNote);
  const [menu, setMenu] = useState<{ x: number; y: number } | null>(null);
  const [preview, setPreview] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const hash = String(node.attrs.hash ?? '');
  const mime = String(node.attrs.type ?? 'application/octet-stream');
  const resource = selectedNote?.resources.find((item) => item.md5 === hash || item.sha256 === hash);
  const url = resourceUrl(resource?.sha256 || hash);
  const name = resource?.fileName || String(node.attrs.title ?? '') || 'Attachment';
  const width = node.attrs.width ? Number(node.attrs.width) : undefined;
  const height = node.attrs.height ? Number(node.attrs.height) : undefined;
  const isPdf = mime.toLowerCase() === 'application/pdf' || name.toLowerCase().endsWith('.pdf');

  const openAttachment = async () => {
    setMenu(null);
    if (!resource || !isNative) return;
    try {
      await api.openAttachment(resource.id);
    } catch (reason) {
      setError(String(reason));
    }
  };
  const saveAttachment = async () => {
    setMenu(null);
    if (!resource || !isNative) return;
    const destination = await save({ defaultPath: name, title: 'Save attachment' });
    if (!destination) return;
    try {
      await api.saveAttachment(resource.id, destination);
    } catch (reason) {
      setError(String(reason));
    }
  };
  const showMenu = (event: React.MouseEvent) => {
    event.preventDefault();
    event.stopPropagation();
    setMenu({
      x: Math.min(event.clientX, window.innerWidth - 190),
      y: Math.min(event.clientY, window.innerHeight - 165),
    });
  };

  let content = (
    <span className="attachment attachment-file">
      {isPdf ? <File size={18} /> : <Paperclip size={18} />}
      <span>{name}</span>
      <small>{resource ? formatBytes(resource.size) : mime}</small>
    </span>
  );
  if (mime.startsWith('image/')) {
    content = (
      <span className="attachment attachment-image" data-drag-handle>
        <img src={url} alt={String(node.attrs.alt ?? name)} width={width} height={height} loading="lazy" />
      </span>
    );
  } else if (mime.startsWith('audio/') || mime.startsWith('video/')) {
    content = (
      <span className="attachment attachment-media">
        <span className="attachment-label">
          {mime.startsWith('audio/') ? <FileAudio size={16} /> : <FileVideo size={16} />}
          {name}
        </span>
        {mime.startsWith('audio/') ? (
          <audio src={url} controls preload="metadata" />
        ) : (
          <video src={url} controls preload="metadata" width={width} height={height} />
        )}
      </span>
    );
  }

  return (
    <NodeViewWrapper as="span" className="attachment-shell" contentEditable={false} onContextMenu={showMenu}>
      {content}
      <button className="attachment-more icon-button" onClick={showMenu} aria-label={`Actions for ${name}`}>
        <MoreHorizontal />
      </button>
      {isPdf && (
        <button className="attachment-preview-button" onClick={() => setPreview(true)}>
          <Eye /> Preview PDF
        </button>
      )}
      {menu && (
        <>
          <button
            className="attachment-menu-scrim"
            aria-label="Close attachment actions"
            onClick={() => setMenu(null)}
          />
          <span className="attachment-menu" role="menu" style={{ left: menu.x, top: menu.y }}>
            {isPdf && (
              <button
                role="menuitem"
                onClick={() => {
                  setMenu(null);
                  setPreview(true);
                }}
              >
                <Eye /> Preview
              </button>
            )}
            <button role="menuitem" disabled={!resource || !isNative} onClick={() => void openAttachment()}>
              <ExternalLink /> Open with system app
            </button>
            <button role="menuitem" disabled={!resource || !isNative} onClick={() => void saveAttachment()}>
              <Download /> Save As…
            </button>
          </span>
        </>
      )}
      {error && (
        <span className="attachment-error" role="alert">
          {error}
        </span>
      )}
      {preview && (
        <Suspense fallback={<span className="attachment-error">Loading PDF preview…</span>}>
          <PdfPreview url={url} name={name} onClose={() => setPreview(false)} />
        </Suspense>
      )}
    </NodeViewWrapper>
  );
}

export function TodoView({ node, updateAttributes }: NodeViewProps) {
  const checked = String(node.attrs.checked) === 'true';
  return (
    <NodeViewWrapper as="span" className="en-todo" contentEditable={false}>
      <input
        type="checkbox"
        checked={checked}
        aria-label={checked ? 'Mark as incomplete' : 'Mark as complete'}
        onChange={(event) => updateAttributes({ checked: String(event.target.checked) })}
      />
    </NodeViewWrapper>
  );
}

export function EncryptedBlockView({ node }: NodeViewProps) {
  return (
    <NodeViewWrapper as="span" className="encrypted-block" contentEditable={false}>
      <LockKeyhole size={15} />
      <span>Encrypted Evernote content</span>
      {node.attrs.hint && <small>Hint: {String(node.attrs.hint)}</small>}
    </NodeViewWrapper>
  );
}

function formatBytes(bytes: number) {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 ** 2) return `${Math.round(bytes / 1024)} KB`;
  return `${(bytes / 1024 ** 2).toFixed(1)} MB`;
}
