// The KDL language and its highlight style for the code pane.

import {
  HighlightStyle,
  LanguageSupport,
  StreamLanguage,
  syntaxHighlighting,
  tags,
} from "../../vendor/codemirror.js";
import { kdl } from "./kdl.js";

const tokenRef = tags.special(tags.string);
const tokenTag = tags.special(tags.annotation);

const language = StreamLanguage.define({
  ...kdl,
  tokenTable: { tokenRef, tokenTag },
});

// Colors come from CSS custom properties, so light and dark follow the
// page theme with no second style.
const style = HighlightStyle.define([
  { tag: tags.comment, color: "var(--syn-comment)", fontStyle: "italic" },
  { tag: tags.keyword, color: "var(--syn-node)", fontWeight: "600" },
  { tag: tags.propertyName, color: "var(--syn-prop)" },
  { tag: tags.operator, color: "var(--syn-punct)" },
  { tag: tags.string, color: "var(--syn-string)" },
  { tag: tokenRef, color: "var(--syn-token)" },
  { tag: tokenTag, color: "var(--syn-token)", fontStyle: "italic" },
  { tag: tags.unit, color: "var(--syn-unit)" },
  { tag: tags.number, color: "var(--syn-number)" },
  { tag: [tags.bool, tags.null], color: "var(--syn-atom)" },
  { tag: [tags.brace, tags.punctuation], color: "var(--syn-punct)" },
  { tag: tags.invalid, color: "var(--danger)", textDecoration: "underline wavy" },
]);

/** KDL language support with Zenith highlighting. */
export function kdlSupport() {
  return [new LanguageSupport(language), syntaxHighlighting(style)];
}
