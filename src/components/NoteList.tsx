import { useVirtualizer } from '@tanstack/react-virtual';
import { Copy, FolderInput, Menu, Paperclip, Plus, SlidersHorizontal, Star, Tags, X } from 'lucide-react';
import { useRef, useState } from 'react';

import { useLibraryStore } from '../store/useLibraryStore';
import type { NoteOrganizationAction } from '../types';
import { FilterPanel } from './FilterPanel';
import { OrganizationDialog } from './OrganizationDialog';

export function NoteList({
  onOpenSidebar,
  onOpenNote,
}: {
  onOpenSidebar: () => void;
  onOpenNote: () => void;
}) {
  const { notes, selectedNote, selectNote, filter, setFilter, createNote, snapshot } = useLibraryStore();
  const [filtersOpen, setFiltersOpen] = useState(false);
  const [checkedIds, setCheckedIds] = useState<Set<string>>(new Set());
  const [organizationAction, setOrganizationAction] = useState<NoteOrganizationAction | null>(null);
  const [organizationMessage, setOrganizationMessage] = useState<string | null>(null);
  const scrollRef = useRef<HTMLDivElement>(null);
  // TanStack Virtual intentionally exposes a mutable virtualizer instance.
  // eslint-disable-next-line react-hooks/incompatible-library
  const virtualizer = useVirtualizer({
    count: notes.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => 126,
    overscan: 8,
  });
  const title = filter.trashOnly
    ? 'Trash'
    : filter.notebookId
      ? snapshot.notebooks.find((item) => item.id === filter.notebookId)?.name
      : filter.stackId
        ? snapshot.stacks.find((item) => item.id === filter.stackId)?.name
        : filter.tagIds.length
          ? snapshot.tags
              .filter((tag) => filter.tagIds.includes(tag.id))
              .map((tag) => `#${tag.name}`)
              .join(' + ')
          : 'All notes';
  const filterCount =
    [filter.contains, filter.createdFrom, filter.createdTo, filter.updatedFrom, filter.updatedTo].filter(
      Boolean,
    ).length + filter.tagIds.length;
  const selectedNoteIds = notes.filter((note) => checkedIds.has(note.id)).map((note) => note.id);
  const allVisibleSelected = notes.length > 0 && selectedNoteIds.length === notes.length;
  const toggleChecked = (id: string, checked: boolean) => {
    setCheckedIds((current) => {
      const next = new Set(current);
      if (checked) next.add(id);
      else next.delete(id);
      return next;
    });
    setOrganizationMessage(null);
  };
  const clearSelection = () => setCheckedIds(new Set());

  return (
    <section className="notes-panel">
      <header className="notes-header">
        <button className="icon-button mobile-menu" onClick={onOpenSidebar} aria-label="Open navigation">
          <Menu />
        </button>
        <div>
          <p className="eyebrow">Library</p>
          <h1>
            {title ?? 'Notes'} <span>{notes.length}</span>
          </h1>
        </div>
        <button
          className="icon-button mobile-new"
          onClick={() => void createNote().then(onOpenNote)}
          aria-label="New note"
        >
          <Plus />
        </button>
      </header>
      <div className="search-wrap">
        <span>⌕</span>
        <input
          aria-label="Search notes"
          placeholder="Search title, content, tags…"
          value={filter.query}
          onChange={(event) =>
            void setFilter({
              query: event.target.value,
              sort: event.target.value ? 'relevance' : 'updatedDesc',
            })
          }
        />
        <kbd>Ctrl K</kbd>
      </div>
      <div className="notes-tools">
        <button
          className={filtersOpen || filterCount ? 'active' : ''}
          onClick={() => setFiltersOpen((value) => !value)}
        >
          <SlidersHorizontal />
          Filters{filterCount > 0 && <span>{filterCount}</span>}
        </button>
        <select
          aria-label="Sort notes"
          value={filter.sort}
          onChange={(event) => void setFilter({ sort: event.target.value as typeof filter.sort })}
        >
          <option value="updatedDesc">Recently updated</option>
          <option value="updatedAsc">Least recently updated</option>
          <option value="createdDesc">Newest created</option>
          <option value="createdAsc">Oldest created</option>
          <option value="titleAsc">Title A–Z</option>
          <option value="titleDesc">Title Z–A</option>
          {filter.query && <option value="relevance">Relevance</option>}
        </select>
      </div>
      {selectedNoteIds.length > 0 && (
        <div className="bulk-toolbar" aria-label="Selected note actions">
          <label title="Select or clear all visible notes">
            <input
              type="checkbox"
              aria-label="Select all visible notes"
              checked={allVisibleSelected}
              onChange={(event) => {
                setCheckedIds(event.target.checked ? new Set(notes.map((note) => note.id)) : new Set());
                setOrganizationMessage(null);
              }}
            />
            <span>{selectedNoteIds.length} selected</span>
          </label>
          <button onClick={() => setOrganizationAction('move')} title="Move selected notes">
            <FolderInput />
            Move
          </button>
          <button onClick={() => setOrganizationAction('copy')} title="Copy selected notes">
            <Copy />
            Copy
          </button>
          <button onClick={() => setOrganizationAction('addTags')} title="Add tags to selected notes">
            <Tags />
            Tags
          </button>
          <button className="icon-button" onClick={clearSelection} aria-label="Clear note selection">
            <X />
          </button>
        </div>
      )}
      {organizationMessage && (
        <p className="organization-status" role="status">
          {organizationMessage}
        </p>
      )}
      {filtersOpen && <FilterPanel onClose={() => setFiltersOpen(false)} />}
      {organizationAction && (
        <OrganizationDialog
          action={organizationAction}
          noteIds={selectedNoteIds}
          onClose={() => setOrganizationAction(null)}
          onComplete={(message) => {
            clearSelection();
            setOrganizationMessage(message);
          }}
        />
      )}
      <div className="note-list" ref={scrollRef} tabIndex={0} aria-label="Notes">
        {notes.length === 0 ? (
          <div className="empty-list">
            <span>⌁</span>
            <h2>No notes here</h2>
            <p>
              {filter.query
                ? 'Try another search or clear a filter.'
                : 'Create a note and make this space yours.'}
            </p>
            {!filter.query && (
              <button onClick={() => void createNote().then(onOpenNote)}>
                <Plus />
                New note
              </button>
            )}
          </div>
        ) : (
          <div style={{ height: virtualizer.getTotalSize(), position: 'relative' }}>
            {virtualizer.getVirtualItems().map((row) => {
              const note = notes[row.index]!;
              return (
                <div
                  key={note.id}
                  className={`note-card ${selectedNote?.id === note.id ? 'selected' : ''} ${
                    checkedIds.has(note.id) ? 'checked' : ''
                  }`}
                  style={{
                    position: 'absolute',
                    top: 0,
                    left: 0,
                    width: '100%',
                    transform: `translateY(${row.start}px)`,
                  }}
                >
                  <label className="note-select" title={`Select ${note.title || 'Untitled'}`}>
                    <input
                      type="checkbox"
                      aria-label={`Select ${note.title || 'Untitled'}`}
                      checked={checkedIds.has(note.id)}
                      onChange={(event) => toggleChecked(note.id, event.target.checked)}
                    />
                  </label>
                  <button
                    className="note-open"
                    aria-label={`Open note ${note.title || 'Untitled'}`}
                    onClick={() => {
                      void selectNote(note.id);
                      onOpenNote();
                    }}
                  >
                    <div className="note-card-top">
                      <h2>{note.title || 'Untitled'}</h2>
                      {note.isShortcut && <Star className="shortcut-star" />}
                    </div>
                    <p>
                      {note.snippet.replaceAll('<mark>', '').replaceAll('</mark>', '') ||
                        'No additional text'}
                    </p>
                    <div className="note-card-meta">
                      <time>{formatDate(note.updatedAt)}</time>
                      <span>{note.notebookName}</span>
                      {note.hasAttachments && <Paperclip />}
                    </div>
                    {note.tags.length > 0 && (
                      <div className="note-card-tags">
                        {note.tags.slice(0, 3).map((tag) => (
                          <span key={tag}>#{tag}</span>
                        ))}
                      </div>
                    )}
                  </button>
                </div>
              );
            })}
          </div>
        )}
      </div>
    </section>
  );
}

function formatDate(timestamp: number) {
  const date = new Date(timestamp);
  const today = new Date();
  if (date.toDateString() === today.toDateString())
    return date.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
  return date.toLocaleDateString([], {
    month: 'short',
    day: 'numeric',
    year: date.getFullYear() === today.getFullYear() ? undefined : 'numeric',
  });
}
