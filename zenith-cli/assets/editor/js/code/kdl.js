// A KDL stream grammar for CodeMirror's `StreamLanguage`, with Zenith
// annotations. Pure: it imports nothing, so a Node test can drive it.
//
// Token names (mapped to highlight tags in `language.js`):
//   comment       `// …`, `/* … */` (nested), and the `/-` slashdash
//   keyword       a node name
//   propertyName  the key of `key=value`
//   operator      `=`
//   string        quoted, raw (`#"…"#`, `r#"…"#`), and bare-word values
//   tokenRef      the string after a `(token)` annotation
//   unit          a type annotation such as `(px)` or `(deg)`
//   tokenTag      the `(token)` annotation
//   number        decimal, hex, octal, binary, and `#inf`/`#-inf`/`#nan`
//   bool          `#true` / `#false` (and KDL 1 `true` / `false`)
//   null          `#null` (and KDL 1 `null`)
//   brace         `{` / `}`
//   punctuation   `;` and the `\` line continuation
//   invalid       a character no KDL token starts with

const NUMBER =
  /^[+-]?(?:0x[0-9a-fA-F][0-9a-fA-F_]*|0o[0-7][0-7_]*|0b[01][01_]*|[0-9][0-9_]*(?:\.[0-9][0-9_]*)?(?:[eE][+-]?[0-9][0-9_]*)?)/;
const BARE = /^[^\s\\/(){}<>;\[\]=,"#]+/;
const KEYWORD = /^#(?:true|false|null|inf|-inf|nan)\b/;
const DELIMITER = /[\s\\/(){};=]/;

/** A fresh tokenizer state. */
function startState() {
  return {
    // Open `/* */` comments, nested.
    comment: 0,
    // Inside a string: `"` quoted, or the number of `#` of a raw string.
    string: null,
    stringType: "string",
    // The next word starts a node.
    expectNode: true,
    // The line ended with `\`: the node continues.
    continued: false,
    // The annotation that applies to the next value.
    annotation: null,
    // `{` depth, for indentation.
    depth: 0,
  };
}

function copyState(s) {
  return { ...s, string: s.string === null ? null : { ...s.string } };
}

/** Read a block comment body; `state.comment` counts open comments. */
function blockComment(stream, state) {
  while (!stream.eol()) {
    if (stream.match("/*")) state.comment++;
    else if (stream.match("*/")) {
      state.comment--;
      if (state.comment === 0) break;
    } else stream.next();
  }
  return "comment";
}

/** Read a string body up to its close (or the line end). */
function stringBody(stream, state) {
  const s = state.string;
  while (!stream.eol()) {
    if (s.raw === null) {
      if (s.multi ? stream.match('"""') : stream.match('"')) {
        state.string = null;
        return state.stringType;
      }
      const ch = stream.next();
      if (ch === "\\") stream.next();
    } else {
      const close = s.multi ? `"""${"#".repeat(s.raw)}` : `"${"#".repeat(s.raw)}`;
      if (stream.match(close)) {
        state.string = null;
        return state.stringType;
      }
      stream.next();
    }
  }
  // A single-line quoted string ends at the line end; keep raw and
  // multi-line strings open.
  if (s.raw === null && !s.multi && !s.continues) state.string = null;
  return state.stringType;
}

/** The token type of a value and the reset after it. */
function value(state, type) {
  state.annotation = null;
  return type;
}

function openString(stream, state, raw, multi) {
  state.stringType = state.annotation === "token" ? "tokenRef" : "string";
  state.annotation = null;
  state.string = { raw, multi, continues: multi };
  state.expectNode = false;
  return stringBody(stream, state);
}

function token(stream, state) {
  if (stream.sol() && state.string === null && state.comment === 0) {
    if (!state.continued) state.expectNode = true;
    state.continued = false;
  }
  if (state.comment > 0) return blockComment(stream, state);
  if (state.string !== null) return stringBody(stream, state);
  if (stream.eatSpace()) return null;

  if (stream.match("//")) {
    stream.skipToEnd();
    return "comment";
  }
  if (stream.match("/*")) {
    state.comment = 1;
    return blockComment(stream, state);
  }
  if (stream.match("/-")) return "comment";

  const ch = stream.peek();
  if (ch === "{") {
    stream.next();
    state.depth++;
    state.expectNode = true;
    state.annotation = null;
    return "brace";
  }
  if (ch === "}") {
    stream.next();
    state.depth = Math.max(0, state.depth - 1);
    state.expectNode = true;
    return "brace";
  }
  if (ch === ";") {
    stream.next();
    state.expectNode = true;
    return "punctuation";
  }
  if (ch === "\\") {
    stream.next();
    state.continued = true;
    return "punctuation";
  }
  if (ch === "=") {
    stream.next();
    return "operator";
  }
  if (ch === "(") {
    const m = stream.match(/^\(\s*([^)\s]*)\s*\)/);
    if (m) {
      state.annotation = m[1];
      return m[1] === "token" ? "tokenTag" : "unit";
    }
    stream.next();
    return "invalid";
  }
  if (stream.match('"""')) return openString(stream, state, null, true);
  if (ch === '"') {
    stream.next();
    return openString(stream, state, null, false);
  }
  // Raw strings: KDL 2 `#"…"#`, KDL 1 `r#"…"#` and `r"…"`.
  const raw = stream.match(/^r?(#*)("""|")/);
  if (raw && (raw[0].startsWith("r") || raw[1].length > 0)) {
    return openString(stream, state, raw[1].length, raw[2] === '"""');
  }
  if (raw) stream.backUp(raw[0].length);
  const kw = stream.match(KEYWORD);
  if (kw) {
    state.expectNode = false;
    if (kw[0] === "#true" || kw[0] === "#false") return value(state, "bool");
    if (kw[0] === "#null") return value(state, "null");
    return value(state, "number");
  }
  if (!state.expectNode) {
    const n = stream.match(NUMBER, false);
    if (n) {
      const after = stream.string.charAt(stream.pos + n[0].length);
      if (after === "" || DELIMITER.test(after)) {
        stream.match(NUMBER);
        return value(state, "number");
      }
    }
  }
  const word = stream.match(BARE);
  if (word) {
    if (state.expectNode) {
      state.expectNode = false;
      state.annotation = null;
      return "keyword";
    }
    if (stream.peek() === "=") return "propertyName";
    if (word[0] === "true" || word[0] === "false") return value(state, "bool");
    if (word[0] === "null") return value(state, "null");
    return value(state, state.annotation === "token" ? "tokenRef" : "string");
  }
  stream.next();
  return "invalid";
}

/** The indentation of a line that starts with `textAfter`. */
function indent(state, textAfter, cx) {
  if (state.comment > 0 || state.string !== null) return null;
  const close = /^\s*\}/.test(textAfter) ? 1 : 0;
  return Math.max(0, state.depth - close) * cx.unit;
}

/** The `StreamLanguage.define` spec. */
export const kdl = {
  name: "kdl",
  startState,
  copyState,
  token,
  indent,
  languageData: {
    commentTokens: { line: "//", block: { open: "/*", close: "*/" } },
    indentOnInput: /^\s*\}$/,
    closeBrackets: { brackets: ["(", "{", '"'] },
  },
};
