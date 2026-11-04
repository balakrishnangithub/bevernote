import { X } from 'lucide-react';

import { useLibraryStore } from '../store/useLibraryStore';

export function FilterPanel({ onClose }: { onClose: () => void }) {
  const { filter, setFilter, snapshot } = useLibraryStore();
  const dateValue = (timestamp: number | null) => {
    if (!timestamp) return '';
    const date = new Date(timestamp);
    return new Date(date.getTime() - date.getTimezoneOffset() * 60_000).toISOString().slice(0, 10);
  };
  const timestamp = (value: string, end = false) =>
    value ? new Date(`${value}T${end ? '23:59:59.999' : '00:00:00.000'}`).getTime() : null;
  const toggleTag = (tagId: string, selected: boolean) => {
    const tagIds = selected ? [...filter.tagIds, tagId] : filter.tagIds.filter((id) => id !== tagId);
    void setFilter({ tagIds });
  };
  return (
    <section className="filter-panel">
      <header>
        <strong>Filter notes</strong>
        <button className="icon-button" onClick={onClose} aria-label="Close filters">
          <X />
        </button>
      </header>
      <label>
        Contains
        <select
          value={filter.contains ?? ''}
          onChange={(event) => void setFilter({ contains: event.target.value || null })}
        >
          <option value="">Anything</option>
          <option value="images">Images</option>
          <option value="audio">Audio</option>
          <option value="video">Video</option>
          <option value="pdf">PDF</option>
          <option value="files">Attachments</option>
          <option value="checklist">Checklists</option>
          <option value="table">Tables</option>
          <option value="code">Code blocks</option>
          <option value="links">Links</option>
        </select>
      </label>
      <div className="filter-dates">
        <label>
          Created after
          <input
            type="date"
            value={dateValue(filter.createdFrom)}
            onChange={(event) => void setFilter({ createdFrom: timestamp(event.target.value) })}
          />
        </label>
        <label>
          Created before
          <input
            type="date"
            value={dateValue(filter.createdTo)}
            onChange={(event) => void setFilter({ createdTo: timestamp(event.target.value, true) })}
          />
        </label>
      </div>
      <div className="filter-dates">
        <label>
          Updated after
          <input
            type="date"
            value={dateValue(filter.updatedFrom)}
            onChange={(event) => void setFilter({ updatedFrom: timestamp(event.target.value) })}
          />
        </label>
        <label>
          Updated before
          <input
            type="date"
            value={dateValue(filter.updatedTo)}
            onChange={(event) => void setFilter({ updatedTo: timestamp(event.target.value, true) })}
          />
        </label>
      </div>
      <fieldset className="filter-tags">
        <legend>
          Tags <span>(all selected tags must match)</span>
        </legend>
        {snapshot.tags.length === 0 ? (
          <p>No tags yet. Add tags to a note to filter by them.</p>
        ) : (
          snapshot.tags.map((tag) => (
            <label className="checkbox-label" key={tag.id}>
              <input
                type="checkbox"
                checked={filter.tagIds.includes(tag.id)}
                onChange={(event) => toggleTag(tag.id, event.target.checked)}
              />
              <span>#{tag.name}</span>
            </label>
          ))
        )}
      </fieldset>
      <label className="checkbox-label">
        <input
          type="checkbox"
          checked={filter.includeTrash}
          onChange={(event) => void setFilter({ includeTrash: event.target.checked })}
        />
        Include trash
      </label>
      <button
        className="text-button"
        onClick={() =>
          void setFilter({
            contains: null,
            createdFrom: null,
            createdTo: null,
            updatedFrom: null,
            updatedTo: null,
            tagIds: [],
            includeTrash: false,
          })
        }
      >
        Clear filters
      </button>
    </section>
  );
}
