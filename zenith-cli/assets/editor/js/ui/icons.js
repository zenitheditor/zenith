// Inline SVG icons, drawn for this page on a 16 px grid with 1.5 px
// strokes in `currentColor`. Each entry is the inner SVG markup.

const PATHS = {
  panelLeft:
    '<rect x="1.75" y="2.75" width="12.5" height="10.5" rx="1.5"/><path d="M6 2.75v10.5"/>',
  panelRight:
    '<rect x="1.75" y="2.75" width="12.5" height="10.5" rx="1.5"/><path d="M10 2.75v10.5"/>',
  panelBottom:
    '<rect x="1.75" y="2.75" width="12.5" height="10.5" rx="1.5"/><path d="M1.75 9.5h12.5"/>',
  undo: '<path d="M5.5 4 2.75 6.75 5.5 9.5"/><path d="M3 6.75h6.5a3.5 3.5 0 0 1 0 7H7"/>',
  redo: '<path d="M10.5 4l2.75 2.75L10.5 9.5"/><path d="M13 6.75H6.5a3.5 3.5 0 0 0 0 7H9"/>',
  save:
    '<path d="M3.25 2.25h7.5l2.5 2.5v8a1 1 0 0 1-1 1h-9a1 1 0 0 1-1-1v-9.5a1 1 0 0 1 1-1z"/><path d="M5.25 2.25v3h5v-3M5 13.75v-4h6v4"/>',
  expand: '<path d="M2.75 6V2.75H6M10 2.75h3.25V6M13.25 10v3.25H10M6 13.25H2.75V10"/>',
  collapse: '<path d="M6 2.75V6H2.75M13.25 6H10V2.75M10 13.25V10h3.25M2.75 10H6v3.25"/>',
  zoomIn: '<circle cx="7" cy="7" r="4.25"/><path d="M10.25 10.25 13.5 13.5M5 7h4M7 5v4"/>',
  zoomOut: '<circle cx="7" cy="7" r="4.25"/><path d="M10.25 10.25 13.5 13.5M5 7h4"/>',
  fit: '<rect x="4.25" y="4.25" width="7.5" height="7.5" rx="1"/><path d="M1.75 4.5v-2.75H4.5M11.5 1.75h2.75V4.5M14.25 11.5v2.75H11.5M4.5 14.25H1.75V11.5"/>',
  theme: '<circle cx="8" cy="8" r="5.25"/><path d="M8 2.75v10.5a5.25 5.25 0 0 0 0-10.5z" fill="currentColor" stroke="none"/>',
  open: '<path d="M1.75 12.75V3.5a1 1 0 0 1 1-1h3l1.5 1.75h5a1 1 0 0 1 1 1V6"/><path d="M1.75 12.75l1.9-5.5a1 1 0 0 1 .95-.7h9.15a.75.75 0 0 1 .72.98l-1.7 5.22z"/>',
  folder: '<path d="M1.75 12.5v-8a1 1 0 0 1 1-1h3l1.5 1.75h5.5a1 1 0 0 1 1 1v6.25a1 1 0 0 1-1 1h-9a1 1 0 0 1-1-1z"/>',
  download: '<path d="M8 2.25v8M4.75 7.25 8 10.5l3.25-3.25M2.75 13.5h10.5"/>',
  close: '<path d="M4 4l8 8M12 4l-8 8"/>',
  chevron: '<path d="M6 4l4 4-4 4"/>',
  page: '<path d="M4 1.75h5.5l2.75 2.75v9.75H4z"/><path d="M9.25 1.75V4.75h3"/>',
  lock: '<rect x="3.5" y="7" width="9" height="6.5" rx="1"/><path d="M5.5 7V5a2.5 2.5 0 0 1 5 0v2"/>',
  hidden:
    '<path d="M2 8s2.25-4 6-4 6 4 6 4-2.25 4-6 4-6-4-6-4z"/><circle cx="8" cy="8" r="1.75"/><path d="M2.5 13.5l11-11"/>',
  error: '<circle cx="8" cy="8" r="6"/><path d="M8 4.75v3.75M8 10.75v.5"/>',
  warning: '<path d="M8 2.25 14.25 13.25H1.75z"/><path d="M8 6.25v3M8 11v.5"/>',
  info: '<circle cx="8" cy="8" r="6"/><path d="M8 7.25v4M8 4.75v.5"/>',
  check: '<path d="M3.25 8.5 6.5 11.75 12.75 4.5"/>',
  magnet: '<path d="M3.25 2.75v5a4.75 4.75 0 0 0 9.5 0v-5"/><path d="M6.25 2.75v5a1.75 1.75 0 0 0 3.5 0v-5"/><path d="M3.25 5.25h3M9.75 5.25h3"/>',
  link: '<path d="M6.75 9.25 9.25 6.75"/><path d="M8.5 4.5l1-1a2.5 2.5 0 0 1 3.5 3.5l-1 1M7.5 11.5l-1 1A2.5 2.5 0 0 1 3 9l1-1"/>',
  // Node kinds.
  rect: '<rect x="2.75" y="3.75" width="10.5" height="8.5" rx="0.5"/>',
  ellipse: '<ellipse cx="8" cy="8" rx="5.5" ry="4.25"/>',
  line: '<path d="M3 13 13 3"/>',
  text: '<path d="M3.5 4V2.75h9V4M8 2.75v10.5M6.25 13.25h3.5"/>',
  code: '<path d="M5.5 4.5 2 8l3.5 3.5M10.5 4.5 14 8l-3.5 3.5"/>',
  frame: '<path d="M4.75 1.75v12.5M11.25 1.75v12.5M1.75 4.75h12.5M1.75 11.25h12.5"/>',
  group: '<rect x="1.75" y="1.75" width="7.5" height="7.5" rx="1"/><rect x="6.75" y="6.75" width="7.5" height="7.5" rx="1"/>',
  image: '<rect x="1.75" y="2.75" width="12.5" height="10.5" rx="1"/><circle cx="5.5" cy="6" r="1.25"/><path d="M2 12l4-4 3 3 2-2 3 3"/>',
  path: '<path d="M2.5 12.5C4 4 12 12 13.5 3.5"/><circle cx="2.5" cy="12.5" r="1"/><circle cx="13.5" cy="3.5" r="1"/>',
  polygon: '<path d="M8 2.25 13.75 6.5 11.5 13.25h-7L2.25 6.5z"/>',
  instance: '<path d="M8 1.75 14.25 8 8 14.25 1.75 8z"/><path d="M8 5 11 8 8 11 5 8z"/>',
  table: '<rect x="1.75" y="2.75" width="12.5" height="10.5" rx="1"/><path d="M1.75 6.25h12.5M1.75 9.75h12.5M6 6.25v7"/>',
  connector: '<circle cx="3.25" cy="12.75" r="1.5"/><circle cx="12.75" cy="3.25" r="1.5"/><path d="M4.5 11.5c3 0 2-7 7-7"/>',
  chart: '<path d="M2.25 13.75h11.5"/><path d="M4 12V8.5M8 12V4.5M12 12V7"/>',
  node: '<circle cx="8" cy="8" r="2.25"/><circle cx="8" cy="8" r="5.75"/>',
  layers: '<path d="M8 2.25 14 5.5 8 8.75 2 5.5z"/><path d="M2 8.5l6 3.25 6-3.25M2 11.25l6 3.25 6-3.25"/>',
};

const KINDS = {
  rect: "rect",
  ellipse: "ellipse",
  line: "line",
  text: "text",
  code: "code",
  frame: "frame",
  group: "group",
  image: "image",
  path: "path",
  polygon: "polygon",
  polyline: "path",
  instance: "instance",
  table: "table",
  connector: "connector",
  chart: "chart",
  shape: "polygon",
  field: "text",
  toc: "text",
  footnote: "text",
};

const parsed = new Map();

/** An `<svg>` element for icon `name`, hidden from assistive tech. */
export function icon(name, className = "icon") {
  const key = PATHS[name] ? name : "node";
  let template = parsed.get(key);
  if (!template) {
    const ns = "http://www.w3.org/2000/svg";
    template = document.createElementNS(ns, "svg");
    template.setAttribute("viewBox", "0 0 16 16");
    template.setAttribute("aria-hidden", "true");
    template.setAttribute("focusable", "false");
    template.innerHTML = PATHS[key];
    parsed.set(key, template);
  }
  // Parse each icon once; clone it after.
  const svg = template.cloneNode(true);
  svg.setAttribute("class", className);
  return svg;
}

/** The icon name for node kind `kind`. */
export function kindIcon(kind) {
  return KINDS[kind] ?? "node";
}

/** Replace every `<i data-icon="name">` under `root` with its SVG. */
export function hydrateIcons(root) {
  for (const el of root.querySelectorAll("i[data-icon]")) {
    el.replaceWith(icon(el.dataset.icon, el.className || "icon"));
  }
}
