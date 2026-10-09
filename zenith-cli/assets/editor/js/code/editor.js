// The code pane: one CodeMirror view over the document text.
//
// CodeMirror keeps no history of its own. Undo and redo go to the engine
// (`history.undo` / `history.redo`), which owns one history for typing and
// engine edits. Engine changes arrive as one transaction marked `remote`,
// so the sync layer never sends them back.

import {
  Annotation,
  Decoration,
  EditorSelection,
  EditorState,
  EditorView,
  Prec,
  StateEffect,
  StateField,
  bracketMatching,
  defaultKeymap,
  drawSelection,
  dropCursor,
  highlightActiveLine,
  highlightActiveLineGutter,
  highlightSelectionMatches,
  highlightSpecialChars,
  indentOnInput,
  indentUnit,
  indentWithTab,
  keymap,
  lineNumbers,
  lintGutter,
  lintKeymap,
  search,
  searchKeymap,
  setDiagnostics,
} from "../../vendor/codemirror.js";
import { kdlSupport } from "./language.js";
import { codeTheme } from "./theme.js";
import { hangingIndent } from "./wrap.js";

// The default special characters minus tab, line feed, and carriage return.
const SPECIAL_CHARS =
  /[\u0000-\u0008\u000b\u000c\u000e-\u001f\u007f-\u009f\u00ad\u061c\u200b\u200e\u200f\u2028\u2029\u202d\u202e\u2066\u2067\u2069\ufeff\ufff9-\ufffc]/g;

/** Marks a transaction that applies an engine change. */
export const remote = Annotation.define();

const setNodeRange = StateEffect.define();

// The source range of the selected node, kept in place across edits.
const nodeRange = StateField.define({
  create: () => Decoration.none,
  update(deco, tr) {
    deco = deco.map(tr.changes);
    for (const e of tr.effects) {
      if (!e.is(setNodeRange)) continue;
      const r = e.value;
      deco =
        r && r.to > r.from
          ? Decoration.set([Decoration.mark({ class: "cm-zen-node" }).range(r.from, r.to)])
          : Decoration.none;
    }
    return deco;
  },
  provide: (f) => EditorView.decorations.from(f),
});

export class CodeEditor {
  /**
   * `handlers`: `localChange(changes)`, `cursor(head)`, `undo()`, `redo()`,
   * `save()`, all called by the view.
   */
  constructor(parent, text, handlers) {
    this.handlers = handlers;
    this.cachedDoc = null;
    this.cachedText = "";
    const commands = Prec.highest(
      keymap.of([
        { key: "Mod-z", run: () => (handlers.undo(), true), preventDefault: true },
        { key: "Mod-Shift-z", run: () => (handlers.redo(), true), preventDefault: true },
        { key: "Mod-y", run: () => (handlers.redo(), true), preventDefault: true },
        { key: "Mod-s", run: () => (handlers.save(), true), preventDefault: true },
      ]),
    );
    const listener = EditorView.updateListener.of((update) => this.onUpdate(update));
    this.view = new EditorView({
      parent,
      state: EditorState.create({
        doc: text,
        extensions: [
          commands,
          lineNumbers(),
          // Zenith nodes are long single lines: wrap them in the split pane.
          EditorView.lineWrapping,
          hangingIndent,
          highlightActiveLineGutter(),
          // `\r` stays in the text, so a CRLF file keeps its bytes.
          EditorState.lineSeparator.of("\n"),
          highlightSpecialChars({ specialChars: SPECIAL_CHARS }),
          drawSelection(),
          dropCursor(),
          EditorState.allowMultipleSelections.of(true),
          EditorState.tabSize.of(2),
          indentUnit.of("  "),
          indentOnInput(),
          bracketMatching(),
          highlightActiveLine(),
          highlightSelectionMatches(),
          search({ top: true }),
          lintGutter(),
          nodeRange,
          kdlSupport(),
          codeTheme,
          EditorView.contentAttributes.of({ "aria-label": "Document source" }),
          keymap.of([...searchKeymap, ...lintKeymap, ...defaultKeymap, indentWithTab]),
          listener,
        ],
      }),
    });
  }

  onUpdate(update) {
    for (const tr of update.transactions) {
      if (tr.docChanged && !tr.annotation(remote)) this.handlers.localChange(tr.changes);
    }
    const user = update.transactions.some(
      (tr) => tr.isUserEvent("select") || tr.isUserEvent("input") || tr.isUserEvent("delete"),
    );
    if (update.selectionSet && user) this.handlers.cursor(update.state.selection.main.head);
  }

  /** The current text. Cached per document state, so repeat reads are free. */
  text() {
    const doc = this.view.state.doc;
    if (doc !== this.cachedDoc) {
      this.cachedDoc = doc;
      this.cachedText = doc.toString();
    }
    return this.cachedText;
  }

  /** Apply an engine change set as one remote transaction. */
  applyRemote(changes) {
    if (changes.empty) return;
    this.view.dispatch({ changes, annotations: remote.of(true) });
  }

  /** Show `list` as lint marks: `{from, to, severity, message, source}`. */
  setDiagnostics(list) {
    const len = this.view.state.doc.length;
    const items = list.map((d) => {
      const from = Math.max(0, Math.min(d.from, len));
      return { ...d, from, to: Math.max(from, Math.min(d.to, len)) };
    });
    this.view.dispatch(setDiagnostics(this.view.state, items));
  }

  /** Mark the selected node's range; scroll it into view unless visible. */
  highlightNode(from, to, { scroll = true } = {}) {
    const len = this.view.state.doc.length;
    const range = { from: Math.min(from, len), to: Math.min(to, len) };
    const effects = [setNodeRange.of(range)];
    if (scroll && !this.isVisible(range.from)) {
      effects.push(EditorView.scrollIntoView(range.from, { y: "start", yMargin: 48 }));
    }
    this.view.dispatch({ effects });
  }

  clearNode() {
    this.view.dispatch({ effects: setNodeRange.of(null) });
  }

  isVisible(pos) {
    const { from, to } = this.view.viewport;
    if (pos < from || pos > to) return false;
    const block = this.view.lineBlockAt(pos);
    const top = this.view.documentTop + block.top;
    const rect = this.view.scrollDOM.getBoundingClientRect();
    return top >= rect.top && top + block.height <= rect.bottom;
  }

  /** Put the cursor at `from` (selecting to `to`), scroll, and focus. */
  jumpTo(from, to = from) {
    const len = this.view.state.doc.length;
    const a = Math.min(from, len);
    this.view.dispatch({
      selection: EditorSelection.single(a, Math.min(Math.max(to, a), len)),
      effects: EditorView.scrollIntoView(a, { y: "center" }),
      userEvent: "select.jump",
    });
    this.view.focus();
  }

  /** The main cursor head, in UTF-16 units. */
  head() {
    return this.view.state.selection.main.head;
  }

  /** The 1-based line and column of the main cursor. */
  lineCol() {
    const head = this.head();
    const line = this.view.state.doc.lineAt(head);
    return { line: line.number, col: head - line.from + 1 };
  }

  focus() {
    this.view.focus();
  }
}
