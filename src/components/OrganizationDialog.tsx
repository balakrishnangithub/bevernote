import { Copy, FolderInput, Tags, X } from 'lucide-react';
import { useState } from 'react';

import { useLibraryStore } from '../store/useLibraryStore';
import type { NoteOrganizationAction } from '../types';

interface Props {
  action: NoteOrganizationAction;
  noteIds: string[];
  onClose: () => void;
  onComplete: (message: string) => void;
}

function splitTags(value: string) {
  return value
    .split(',')
    .map((name) => name.trim().replace(/^#/, ''))
    .filter(Boolean);
}

export function OrganizationDialog({ action, noteIds, onClose, onComplete }: Props) {
  const { snapshot, organizeNotes } = useLibraryStore();
  const [notebookId, setNotebookId] = useState('');
  const [selectedTags, setSelectedTags] = useState<string[]>([]);
  const [tagInput, setTagInput] = useState('');
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const addingTags = action === 'addTags';
  const title = action === 'move' ? 'Move notes' : action === 'copy' ? 'Copy notes' : 'Add tags';
  const ActionIcon = action === 'move' ? FolderInput : action === 'copy' ? Copy : Tags;
  const tagQuery = tagInput.split(',').at(-1)?.trim().replace(/^#/, '').toLowerCase() ?? '';
  const suggestions = snapshot.tags
    .filter((tag) => !selectedTags.some((name) => name.toLowerCase() === tag.name.toLowerCase()))
    .filter((tag) => !tagQuery || tag.name.toLowerCase().includes(tagQuery))
    .sort((a, b) => a.name.localeCompare(b.name));

  const canonicalTag = (name: string) =>
    snapshot.tags.find((tag) => tag.name.toLowerCase() === name.toLowerCase())?.name ?? name;
  const addTag = (value: string) => {
    const name = canonicalTag(value.trim().replace(/^#/, ''));
    if (!name) return;
    setSelectedTags((current) =>
      current.some((tag) => tag.toLowerCase() === name.toLowerCase()) ? current : [...current, name],
    );
    setTagInput('');
    setError(null);
  };
  const addTagsFromInput = () => {
    const names = splitTags(tagInput).map(canonicalTag);
    if (names.length) {
      setSelectedTags((current) => {
        const merged = [...current];
        for (const name of names) {
          if (!merged.some((tag) => tag.toLowerCase() === name.toLowerCase())) merged.push(name);
        }
        return merged;
      });
    }
    setTagInput('');
    setError(null);
  };

  const submit = async () => {
    const names = addingTags
      ? [
          ...new Map(
            [...selectedTags, ...splitTags(tagInput).map(canonicalTag)].map((name) => [
              name.toLowerCase(),
              name,
            ]),
          ).values(),
        ]
      : [];
    if (!addingTags && !notebookId) {
      setError('Choose a destination notebook.');
      return;
    }
    if (addingTags && names.length === 0) {
      setError('Choose an existing tag or enter at least one new tag.');
      return;
    }
    setSaving(true);
    setError(null);
    try {
      const changed = await organizeNotes(noteIds, action, addingTags ? null : notebookId, names);
      const noun = changed === 1 ? 'note' : 'notes';
      const message = addingTags
        ? changed
          ? `Tags added to ${changed} ${noun}.`
          : 'No changes; the selected notes already have those tags.'
        : `${changed} ${noun} ${action === 'move' ? 'moved' : 'copied'}.`;
      onComplete(message);
      onClose();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="modal-backdrop" onMouseDown={onClose}>
      <section
        className="modal organization-dialog"
        role="dialog"
        aria-modal="true"
        aria-label={title}
        aria-describedby="organization-description"
        onMouseDown={(event) => event.stopPropagation()}
        onKeyDown={(event) => {
          if (event.key === 'Escape') {
            event.stopPropagation();
            onClose();
          }
        }}
      >
        <header>
          <div>
            <p className="eyebrow">Organize library</p>
            <h2>{title}</h2>
          </div>
          <button className="icon-button" onClick={onClose} aria-label={`Close ${title.toLowerCase()}`}>
            <X />
          </button>
        </header>
        <p id="organization-description">
          {noteIds.length} {noteIds.length === 1 ? 'note' : 'notes'} selected.
          {action === 'copy' ? ' Copies keep their content, tags, metadata, and attachments.' : ''}
        </p>
        <form
          onSubmit={(event) => {
            event.preventDefault();
            void submit();
          }}
        >
          {addingTags ? (
            <div className="organization-tag-field">
              <label htmlFor="organization-tags">Tags</label>
              {selectedTags.length > 0 && (
                <div className="organization-selected-tags" role="group" aria-label="Tags to add">
                  {selectedTags.map((tag) => (
                    <button
                      type="button"
                      key={tag}
                      aria-label={`Remove tag ${tag}`}
                      onClick={() => setSelectedTags((current) => current.filter((name) => name !== tag))}
                    >
                      #{tag}
                      <X />
                    </button>
                  ))}
                </div>
              )}
              <input
                id="organization-tags"
                autoFocus
                value={tagInput}
                onChange={(event) => setTagInput(event.target.value)}
                onKeyDown={(event) => {
                  if (event.key === 'Enter' || event.key === ',') {
                    event.preventDefault();
                    addTagsFromInput();
                  } else if (event.key === 'Backspace' && !tagInput && selectedTags.length) {
                    setSelectedTags((current) => current.slice(0, -1));
                  }
                }}
                placeholder="Type a new tag or search existing tags"
                aria-label="Tags"
                aria-describedby="organization-tags-hint"
              />
              <span id="organization-tags-hint">
                Click an existing tag or type a new one. Press Enter or comma to add several.
              </span>
              <div className="organization-tag-suggestions" role="group" aria-label="Existing tags">
                <span>Existing tags</span>
                <div>
                  {suggestions.map((tag) => (
                    <button
                      type="button"
                      key={tag.id}
                      aria-label={`Add existing tag ${tag.name}`}
                      onMouseDown={(event) => event.preventDefault()}
                      onClick={() => addTag(tag.name)}
                    >
                      #{tag.name}
                    </button>
                  ))}
                  {suggestions.length === 0 && <p>No matching existing tags.</p>}
                </div>
              </div>
            </div>
          ) : (
            <label>
              Destination notebook
              <select
                autoFocus
                required
                value={notebookId}
                onChange={(event) => setNotebookId(event.target.value)}
              >
                <option value="">Choose a notebook…</option>
                {snapshot.notebooks.map((notebook) => (
                  <option key={notebook.id} value={notebook.id}>
                    {notebook.name}
                  </option>
                ))}
              </select>
            </label>
          )}
          {error && (
            <p className="organization-error" role="alert">
              {error}
            </p>
          )}
          <div className="organization-actions">
            <button type="button" className="secondary-action" disabled={saving} onClick={onClose}>
              Cancel
            </button>
            <button
              type="submit"
              className="primary-action"
              disabled={
                saving ||
                noteIds.length === 0 ||
                (addingTags && selectedTags.length === 0 && !tagInput.trim())
              }
            >
              <ActionIcon />
              {saving ? 'Applying…' : title}
            </button>
          </div>
        </form>
      </section>
    </div>
  );
}
