import type { Editor } from '@tiptap/core';
import {
  AlignCenter,
  AlignLeft,
  AlignRight,
  Bold,
  CheckSquare,
  Code2,
  Indent,
  Italic,
  Link2,
  List,
  ListOrdered,
  Outdent,
  Paperclip,
  Redo2,
  Strikethrough,
  Subscript,
  Superscript,
  Table2,
  Underline,
  Undo2,
} from 'lucide-react';
import type { ReactNode } from 'react';

import { ColorPalette } from './ColorPalette';

interface Props {
  editor: Editor;
  onAttach: () => void;
  onInternalLink: () => void;
}

export function EditorToolbar({ editor, onAttach, onInternalLink }: Props) {
  const heading = [1, 2, 3, 4].find((level) => editor.isActive('heading', { level })) ?? 0;
  const textStyle = editor.getAttributes('textStyle');
  return (
    <div className="editor-toolbar" role="toolbar" aria-label="Note formatting">
      <div className="toolbar-group">
        <Button label="Undo" onClick={() => editor.chain().focus().undo().run()}>
          <Undo2 />
        </Button>
        <Button label="Redo" onClick={() => editor.chain().focus().redo().run()}>
          <Redo2 />
        </Button>
      </div>
      <div className="toolbar-group">
        <select
          aria-label="Text style"
          value={heading}
          onChange={(event) => {
            const level = Number(event.target.value);
            const chain = editor.chain().focus();
            if (level === 0) chain.setParagraph().run();
            else chain.toggleHeading({ level: level as 1 | 2 | 3 | 4 }).run();
          }}
        >
          <option value={0}>Normal</option>
          <option value={1}>Heading 1</option>
          <option value={2}>Heading 2</option>
          <option value={3}>Heading 3</option>
          <option value={4}>Heading 4</option>
        </select>
        <select
          aria-label="Font family"
          value={String(textStyle.fontFamily ?? '')}
          onChange={(event) =>
            event.target.value
              ? editor.chain().focus().setFontFamily(event.target.value).run()
              : editor.chain().focus().unsetFontFamily().run()
          }
        >
          <option value="">Default font</option>
          <option value="Arial">Arial</option>
          <option value="Georgia">Georgia</option>
          <option value="Times New Roman">Times New Roman</option>
          <option value="Courier New">Courier New</option>
          <option value="Verdana">Verdana</option>
          <option value="Trebuchet MS">Trebuchet MS</option>
        </select>
        <select
          aria-label="Font size"
          className="font-size-select"
          value={String(textStyle.fontSize ?? '')}
          onChange={(event) =>
            event.target.value
              ? editor.chain().focus().setMark('textStyle', { fontSize: event.target.value }).run()
              : editor.chain().focus().setMark('textStyle', { fontSize: null }).run()
          }
        >
          <option value="">Size</option>
          {[10, 12, 14, 16, 18, 20, 24, 30, 36, 48].map((size) => (
            <option key={size} value={`${size}px`}>
              {size}
            </option>
          ))}
        </select>
      </div>
      <div className="toolbar-group">
        <Button
          label="Bold"
          active={editor.isActive('bold')}
          onClick={() => editor.chain().focus().toggleBold().run()}
        >
          <Bold />
        </Button>
        <Button
          label="Italic"
          active={editor.isActive('italic')}
          onClick={() => editor.chain().focus().toggleItalic().run()}
        >
          <Italic />
        </Button>
        <Button
          label="Underline"
          active={editor.isActive('underline')}
          onClick={() => editor.chain().focus().toggleUnderline().run()}
        >
          <Underline />
        </Button>
        <Button
          label="Strikethrough"
          active={editor.isActive('strike')}
          onClick={() => editor.chain().focus().toggleStrike().run()}
        >
          <Strikethrough />
        </Button>
        <ColorPalette editor={editor} kind="text" />
        <ColorPalette editor={editor} kind="highlight" />
      </div>
      <div className="toolbar-group">
        <Button
          label="Bulleted list"
          active={editor.isActive('bulletList')}
          onClick={() => editor.chain().focus().toggleBulletList().run()}
        >
          <List />
        </Button>
        <Button
          label="Numbered list"
          active={editor.isActive('orderedList')}
          onClick={() => editor.chain().focus().toggleOrderedList().run()}
        >
          <ListOrdered />
        </Button>
        <Button
          label="Checklist"
          active={editor.isActive('taskList')}
          onClick={() => editor.chain().focus().toggleTaskList().run()}
        >
          <CheckSquare />
        </Button>
        <Button
          label="Code block"
          active={editor.isActive('codeBlock')}
          onClick={() => editor.chain().focus().toggleCodeBlock().run()}
        >
          <Code2 />
        </Button>
        <Button
          label="Superscript"
          active={editor.isActive('superscript')}
          onClick={() => editor.chain().focus().toggleSuperscript().run()}
        >
          <Superscript />
        </Button>
        <Button
          label="Subscript"
          active={editor.isActive('subscript')}
          onClick={() => editor.chain().focus().toggleSubscript().run()}
        >
          <Subscript />
        </Button>
      </div>
      <div className="toolbar-group">
        <Button
          label="Align left"
          active={editor.isActive({ textAlign: 'left' })}
          onClick={() => editor.chain().focus().setTextAlign('left').run()}
        >
          <AlignLeft />
        </Button>
        <Button
          label="Align center"
          active={editor.isActive({ textAlign: 'center' })}
          onClick={() => editor.chain().focus().setTextAlign('center').run()}
        >
          <AlignCenter />
        </Button>
        <Button
          label="Align right"
          active={editor.isActive({ textAlign: 'right' })}
          onClick={() => editor.chain().focus().setTextAlign('right').run()}
        >
          <AlignRight />
        </Button>
        <Button label="Outdent" onClick={() => changeIndent(editor, -1)}>
          <Outdent />
        </Button>
        <Button label="Indent" onClick={() => changeIndent(editor, 1)}>
          <Indent />
        </Button>
      </div>
      <div className="toolbar-group">
        <Button
          label="Insert table"
          onClick={() => editor.chain().focus().insertTable({ rows: 3, cols: 3, withHeaderRow: true }).run()}
        >
          <Table2 />
        </Button>
        <Button
          label="External link"
          active={editor.isActive('link')}
          onClick={() => setExternalLink(editor)}
        >
          <Link2 />
        </Button>
        <Button label="Internal note link" onClick={onInternalLink}>
          <span className="internal-link-icon">@</span>
        </Button>
        <Button label="Attach a file" onClick={onAttach}>
          <Paperclip />
        </Button>
      </div>
    </div>
  );
}

function Button({
  label,
  active = false,
  onClick,
  children,
}: {
  label: string;
  active?: boolean;
  onClick: () => void;
  children: ReactNode;
}) {
  return (
    <button
      type="button"
      className={`toolbar-button ${active ? 'active' : ''}`}
      title={label}
      aria-label={label}
      aria-pressed={active}
      onMouseDown={(event) => event.preventDefault()}
      onClick={onClick}
    >
      {children}
    </button>
  );
}

function setExternalLink(editor: Editor) {
  if (editor.isActive('link')) {
    editor.chain().focus().unsetLink().run();
    return;
  }
  const href = window.prompt('Link URL (https://…)');
  if (href) editor.chain().focus().extendMarkRange('link').setLink({ href }).run();
}

function changeIndent(editor: Editor, delta: number) {
  if (editor.isActive('listItem')) {
    const command = delta > 0 ? 'sinkListItem' : 'liftListItem';
    editor.chain().focus()[command]('listItem').run();
    return;
  }
  if (editor.isActive('taskItem')) {
    const command = delta > 0 ? 'sinkListItem' : 'liftListItem';
    editor.chain().focus()[command]('taskItem').run();
    return;
  }
  const type = editor.isActive('heading') ? 'heading' : 'paragraph';
  const current = Number(editor.getAttributes(type).indent ?? 0);
  editor
    .chain()
    .focus()
    .updateAttributes(type, { indent: Math.max(0, Math.min(8, current + delta)) })
    .run();
}
