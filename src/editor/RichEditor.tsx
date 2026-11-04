import { open } from '@tauri-apps/plugin-dialog';
import { openUrl } from '@tauri-apps/plugin-opener';
import { EditorContent, useEditor } from '@tiptap/react';
import { ChevronDown, ChevronUp, X } from 'lucide-react';
import { useEffect, useState } from 'react';

import { isNative } from '../lib/api';
import { useLibraryStore } from '../store/useLibraryStore';
import type { Note } from '../types';
import { EditorToolbar } from './EditorToolbar';
import { createExtensions } from './extensions';
import { noteFindState, updateNoteFind } from './noteFind';

interface Props {
  note: Note;
  onChange: (html: string) => void;
  onAttach: (path: string) => Promise<Note | null>;
}

export function RichEditor({ note, onChange, onAttach }: Props) {
  const selectNote = useLibraryStore((state) => state.selectNote);
  const [, setVersion] = useState(0);
  const [linkDialog, setLinkDialog] = useState(false);
  const [findOpen, setFindOpen] = useState(false);
  const [findQuery, setFindQuery] = useState('');
  const [findCount, setFindCount] = useState(0);
  const [findActive, setFindActive] = useState(0);
  const editor = useEditor(
    {
      extensions: createExtensions(),
      content: note.contentHtml,
      immediatelyRender: false,
      editorProps: {
        attributes: { class: 'note-prose', 'aria-label': 'Note content', spellcheck: 'true' },
        handleClick: (_view, _pos, event) => {
          const anchor = (event.target as HTMLElement).closest('a');
          const href = anchor?.getAttribute('href');
          if (!href) return false;
          if (href.startsWith('bevernote://note/')) {
            void selectNote(href.slice('bevernote://note/'.length));
          } else if (isNative) {
            void openUrl(href);
          } else {
            window.open(href, '_blank', 'noopener,noreferrer');
          }
          return true;
        },
      },
      onUpdate: ({ editor: instance }) => onChange(instance.getHTML()),
      onSelectionUpdate: () => setVersion((value) => value + 1),
      onTransaction: () => setVersion((value) => value + 1),
    },
    [note.id],
  );

  useEffect(() => {
    if (!editor) return;
    const openFind = () => {
      const { from, to } = editor.state.selection;
      const selected = editor.state.doc.textBetween(from, to, ' ').trim();
      if (selected && selected.length < 200) setFindQuery(selected);
      setFindOpen(true);
    };
    window.addEventListener('bevernote:find', openFind);
    return () => window.removeEventListener('bevernote:find', openFind);
  }, [editor]);

  useEffect(() => {
    if (!editor || !findOpen) return;
    const timer = window.setTimeout(() => {
      const state = updateNoteFind(editor, findQuery, 0);
      setFindCount(state?.matches.length ?? 0);
      setFindActive(state?.active ?? 0);
    }, 120);
    return () => window.clearTimeout(timer);
  }, [editor, findOpen, findQuery]);

  if (!editor) return <div className="editor-loading">Loading editor…</div>;

  const closeFind = () => {
    setFindOpen(false);
    updateNoteFind(editor, '');
    editor.commands.focus();
  };
  const moveFind = (direction: number) => {
    const current = noteFindState(editor);
    if (!current?.matches.length) return;
    const state = updateNoteFind(editor, findQuery, current.active + direction);
    setFindActive(state?.active ?? 0);
  };
  const attach = async () => {
    if (!isNative) return;
    const path = await open({ multiple: false, title: 'Attach a file' });
    if (!path || Array.isArray(path)) return;
    const updated = await onAttach(path);
    if (updated) {
      editor.commands.setContent(updated.contentHtml);
      onChange(updated.contentHtml);
    }
  };

  return (
    <div className="rich-editor">
      <EditorToolbar
        editor={editor}
        onAttach={() => void attach()}
        onInternalLink={() => setLinkDialog(true)}
      />
      {findOpen && (
        <div className="note-find" role="dialog" aria-label="Find in note">
          <input
            autoFocus
            aria-label="Find in note content"
            placeholder="Find in this note…"
            value={findQuery}
            onChange={(event) => setFindQuery(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === 'Enter') {
                event.preventDefault();
                moveFind(event.shiftKey ? -1 : 1);
              }
              if (event.key === 'Escape') {
                event.preventDefault();
                event.stopPropagation();
                closeFind();
              }
            }}
          />
          <span aria-live="polite">
            {findQuery ? (findCount ? `${findActive + 1} / ${findCount}` : 'No matches') : ''}
          </span>
          <button className="icon-button" onClick={() => moveFind(-1)} aria-label="Previous match">
            <ChevronUp />
          </button>
          <button className="icon-button" onClick={() => moveFind(1)} aria-label="Next match">
            <ChevronDown />
          </button>
          <button className="icon-button" onClick={closeFind} aria-label="Close find">
            <X />
          </button>
        </div>
      )}
      <EditorContent editor={editor} className="editor-content" />
      {linkDialog && <InternalLinkDialog editor={editor} onClose={() => setLinkDialog(false)} />}
    </div>
  );
}

function InternalLinkDialog({
  editor,
  onClose,
}: {
  editor: NonNullable<ReturnType<typeof useEditor>>;
  onClose: () => void;
}) {
  const notes = useLibraryStore((state) => state.notes);
  const [query, setQuery] = useState('');
  const matches = notes.filter((note) => note.title.toLowerCase().includes(query.toLowerCase())).slice(0, 12);
  const insert = (id: string, title: string) => {
    const href = `bevernote://note/${id}`;
    const chain = editor.chain().focus();
    if (editor.state.selection.empty) {
      chain.insertContent({ type: 'text', text: title, marks: [{ type: 'link', attrs: { href } }] }).run();
    } else {
      chain.extendMarkRange('link').setLink({ href }).run();
    }
    onClose();
  };
  return (
    <div className="modal-backdrop" role="presentation" onMouseDown={onClose}>
      <section
        className="modal link-modal"
        role="dialog"
        aria-modal="true"
        aria-label="Link to a note"
        onMouseDown={(event) => event.stopPropagation()}
        onKeyDown={(event) => {
          if (event.key === 'Escape') {
            event.preventDefault();
            event.stopPropagation();
            onClose();
          }
        }}
      >
        <header>
          <div>
            <p className="eyebrow">Internal link</p>
            <h2>Link to a note</h2>
          </div>
          <button className="icon-button" onClick={onClose} aria-label="Close">
            ×
          </button>
        </header>
        <input
          autoFocus
          placeholder="Search your notes…"
          value={query}
          onChange={(event) => setQuery(event.target.value)}
        />
        <div className="link-results">
          {matches.map((note) => (
            <button key={note.id} onClick={() => insert(note.id, note.title || 'Untitled')}>
              <strong>{note.title || 'Untitled'}</strong>
              <span>{note.notebookName}</span>
            </button>
          ))}
          {matches.length === 0 && <p>No matching notes.</p>}
        </div>
      </section>
    </div>
  );
}
