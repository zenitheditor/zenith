// The code pane theme. Every color is a CSS custom property of the page,
// so light and dark mode need no second theme.

import { EditorView } from "../../vendor/codemirror.js";

// Lint gutter markers, drawn for this page.
const ERROR =
  "<circle cx='6' cy='6' r='5' fill='#d9443a'/><path d='M6 3.2v3.3M6 8.4v.4' stroke='#fff' stroke-width='1.4' stroke-linecap='round'/>";
const WARNING =
  "<path d='M6 1 11.4 10.6H.6z' fill='#d49b1f'/><path d='M6 4.4v2.6M6 8.6v.3' stroke='#fff' stroke-width='1.3' stroke-linecap='round'/>";
const INFO =
  "<circle cx='6' cy='6' r='5' fill='#3d73d6'/><path d='M6 5.3v3.2M6 3.4v.3' stroke='#fff' stroke-width='1.4' stroke-linecap='round'/>";

function marker(body) {
  const svg = `<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 12 12'>${body}</svg>`;
  return `url("data:image/svg+xml,${encodeURIComponent(svg)}")`;
}

export const codeTheme = EditorView.theme({
  "&": {
    height: "100%",
    color: "var(--text)",
    backgroundColor: "var(--code-bg)",
    fontSize: "var(--code-size)",
  },
  "&.cm-focused": { outline: "none" },
  ".cm-scroller": {
    fontFamily: "var(--font-mono)",
    lineHeight: "1.6",
  },
  ".cm-content": { caretColor: "var(--accent)", padding: "8px 0" },
  ".cm-cursor, .cm-dropCursor": { borderLeftColor: "var(--accent)", borderLeftWidth: "2px" },
  "&.cm-focused > .cm-scroller > .cm-selectionLayer .cm-selectionBackground, .cm-selectionBackground, .cm-content ::selection":
    { backgroundColor: "var(--code-selection)" },
  ".cm-gutters": {
    backgroundColor: "var(--code-bg)",
    color: "var(--text-faint)",
    border: "none",
    borderRight: "1px solid var(--border-subtle)",
  },
  ".cm-lineNumbers .cm-gutterElement": { padding: "0 8px 0 12px", minWidth: "40px" },
  ".cm-activeLine": { backgroundColor: "var(--code-active-line)" },
  ".cm-activeLineGutter": { backgroundColor: "var(--code-active-line)", color: "var(--text-muted)" },
  ".cm-matchingBracket, &.cm-focused .cm-matchingBracket": {
    backgroundColor: "var(--code-bracket)",
    outline: "1px solid var(--border-strong)",
  },
  ".cm-nonmatchingBracket": { color: "var(--danger)" },
  ".cm-selectionMatch": { backgroundColor: "var(--code-match)" },
  ".cm-searchMatch": {
    backgroundColor: "var(--code-match)",
    outline: "1px solid var(--warning)",
  },
  ".cm-searchMatch.cm-searchMatch-selected": { backgroundColor: "var(--code-selection)" },
  ".cm-zen-node": {
    backgroundColor: "var(--code-node)",
    borderRadius: "2px",
  },
  ".cm-panels": {
    backgroundColor: "var(--surface-2)",
    color: "var(--text)",
    fontFamily: "var(--font-ui)",
    fontSize: "13px",
  },
  ".cm-panels.cm-panels-top": { borderBottom: "1px solid var(--border)" },
  ".cm-panel.cm-search": { padding: "8px 12px" },
  ".cm-panel.cm-search input, .cm-panel.cm-search button, .cm-panel.cm-search label": {
    fontSize: "13px",
  },
  ".cm-textfield": {
    backgroundColor: "var(--surface)",
    color: "var(--text)",
    border: "1px solid var(--border)",
    borderRadius: "var(--radius-sm)",
    padding: "4px 8px",
  },
  ".cm-button": {
    backgroundImage: "none",
    backgroundColor: "var(--surface)",
    color: "var(--text)",
    border: "1px solid var(--border)",
    borderRadius: "var(--radius-sm)",
    padding: "4px 8px",
  },
  ".cm-tooltip": {
    backgroundColor: "var(--surface)",
    color: "var(--text)",
    border: "1px solid var(--border)",
    borderRadius: "var(--radius-sm)",
    boxShadow: "var(--shadow-pop)",
  },
  ".cm-diagnostic": { fontFamily: "var(--font-ui)", fontSize: "13px", padding: "6px 10px" },
  ".cm-diagnostic-error": { borderLeftColor: "var(--danger)" },
  ".cm-diagnostic-warning": { borderLeftColor: "var(--warning)" },
  ".cm-diagnostic-info": { borderLeftColor: "var(--info)" },
  ".cm-lintRange-error": {
    backgroundImage: "none",
    textDecoration: "underline wavy var(--danger)",
    textUnderlineOffset: "3px",
  },
  ".cm-lintRange-warning": {
    backgroundImage: "none",
    textDecoration: "underline wavy var(--warning)",
    textUnderlineOffset: "3px",
  },
  ".cm-lintRange-info": {
    backgroundImage: "none",
    textDecoration: "underline dotted var(--info)",
    textUnderlineOffset: "3px",
  },
  ".cm-lint-marker": { width: "12px", height: "12px" },
  ".cm-lint-marker-error": { content: marker(ERROR) },
  ".cm-lint-marker-warning": { content: marker(WARNING) },
  ".cm-lint-marker-info": { content: marker(INFO) },
});
