// Assets panel in the left rail: browse every image, SVG, font, and sibling
// `.zen` design loaded in the workspace/project, preview thumbnails, click an
// image/SVG to insert it onto the current page, upload new files, or locate
// missing declared assets.

import { clear, h } from "../util/dom.js";
import { icon } from "../ui/icons.js";

const IMAGE_EXT = /\.(png|jpe?g|webp|gif|svg)$/i;
const FONT_EXT = /\.(ttf|otf|woff2?)$/i;
const ZEN_EXT = /\.zen$/i;

function mimeFor(path) {
  const ext = path.split(".").pop()?.toLowerCase() ?? "";
  return {
    png: "image/png",
    jpg: "image/jpeg",
    jpeg: "image/jpeg",
    webp: "image/webp",
    gif: "image/gif",
    svg: "image/svg+xml",
  }[ext] ?? "application/octet-stream";
}

function approxBytes(b64) {
  const bytes = Math.floor((String(b64 ?? "").length * 3) / 4);
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

export class AssetsPanel {
  /**
   * `hooks`:
   *   `files()`: map of `{ [relPath]: base64 }`
   *   `sourceText()`: current `.zen` source
   *   `currentPath()`: current open `.zen` path
   *   `onInsert(relPath)`: insert existing project image/SVG onto current page
   *   `onUpload()`: pick & upload a new image/SVG/asset
   *   `onOpenDoc(relPath)`: open a sibling `.zen` document from the project
   *   `onLocateMissing(paths)`: prompt to import missing files
   */
  constructor(host, hooks) {
    this.host = host;
    this.hooks = hooks ?? {};
    this.query = "";
  }

  refresh() {
    if (!this.host) return;
    const files = this.hooks.files?.() ?? {};
    const srcText = this.hooks.sourceText?.() ?? "";
    const curPath = this.hooks.currentPath?.() ?? "";

    // Find declared `src="..."` assets that are missing from `files`.
    const missing = [];
    for (const m of srcText.matchAll(/\bsrc="([^"]+)"/g)) {
      const rel = m[1];
      if (!files[rel] && !missing.includes(rel)) missing.push(rel);
    }

    const q = this.query.trim().toLowerCase();
    const match = (p) => !q || p.toLowerCase().includes(q);

    const images = [];
    const docs = [];
    const fonts = [];
    const other = [];
    for (const [rel, b64] of Object.entries(files).sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0))) {
      if (!match(rel)) continue;
      if (IMAGE_EXT.test(rel)) images.push([rel, b64]);
      else if (ZEN_EXT.test(rel)) docs.push([rel, b64]);
      else if (FONT_EXT.test(rel)) fonts.push([rel, b64]);
      else other.push([rel, b64]);
    }

    clear(this.host);

    // Toolbar: search + upload
    const search = h("input", {
      type: "search",
      class: "edit-input asset-search",
      placeholder: "Filter…",
      value: this.query,
      "aria-label": "Filter assets",
      oninput: (e) => {
        this.query = e.target.value;
        this.refresh();
        const next = this.host.querySelector(".asset-search");
        if (next) {
          next.focus();
          next.setSelectionRange(this.query.length, this.query.length);
        }
      },
    });
    const uploadBtn = this.hooks.onUpload
      ? h(
          "button",
          {
            type: "button",
            class: "page-act-btn asset-upload-btn",
            title: "Upload image or SVG",
            onclick: () => this.hooks.onUpload(),
          },
          icon("plus"),
          h("span", { text: "Add" }),
        )
      : null;
    this.host.appendChild(h("div", { class: "asset-bar" }, search, uploadBtn));

    // Missing declared assets
    const filteredMissing = missing.filter(match);
    if (filteredMissing.length) {
      this.host.appendChild(h("div", { class: "tok-group", text: "Missing" }));
      for (const rel of filteredMissing) {
        this.host.appendChild(
          h(
            "button",
            {
              type: "button",
              class: "asset-item missing",
              title: `Locate ${rel}`,
              onclick: () => this.hooks.onLocateMissing?.([rel]),
            },
            icon("warning"),
            h("span", { class: "tok-name", text: rel }),
            h("span", { class: "asset-badge", text: "Locate" }),
          ),
        );
      }
    }

    // Images & SVGs grid with live thumbnails
    if (images.length) {
      this.host.appendChild(h("div", { class: "tok-group", text: `Images (${images.length})` }));
      const grid = h("div", { class: "asset-grid" });
      for (const [rel, b64] of images) {
        const name = rel.split("/").pop();
        const img = h("img", {
          src: `data:${mimeFor(rel)};base64,${b64}`,
          alt: name,
          loading: "lazy",
        });
        grid.appendChild(
          h(
            "button",
            {
              type: "button",
              class: "asset-card",
              title: `${rel} (${approxBytes(b64)}) — click to insert`,
              onclick: () => this.hooks.onInsert?.(rel, b64),
            },
            h("span", { class: "asset-thumb" }, img),
            h("span", { class: "asset-card-name", text: name }),
          ),
        );
      }
      this.host.appendChild(grid);
    }

    // Workspace .zen files
    if (docs.length) {
      this.host.appendChild(h("div", { class: "tok-group", text: `Designs (${docs.length})` }));
      for (const [rel] of docs) {
        const active = rel === curPath || rel === curPath.split("/").pop();
        this.host.appendChild(
          h(
            "button",
            {
              type: "button",
              class: `asset-item${active ? " on" : ""}`,
              title: active ? `${rel} (open)` : `Open ${rel}`,
              onclick: () => {
                if (!active) this.hooks.onOpenDoc?.(rel);
              },
            },
            icon("page"),
            h("span", { class: "tok-name", text: rel }),
            active ? h("span", { class: "asset-badge", text: "Open" }) : null,
          ),
        );
      }
    }

    // Fonts & other files
    if (fonts.length || other.length) {
      const rest = [...fonts, ...other];
      this.host.appendChild(h("div", { class: "tok-group", text: `Files (${rest.length})` }));
      for (const [rel, b64] of rest) {
        this.host.appendChild(
          h(
            "div",
            { class: "asset-item static", title: `${rel} (${approxBytes(b64)})` },
            icon(FONT_EXT.test(rel) ? "text" : "page"),
            h("span", { class: "tok-name", text: rel }),
            h("span", { class: "v", text: approxBytes(b64) }),
          ),
        );
      }
    }

    if (!filteredMissing.length && !images.length && !docs.length && !fonts.length && !other.length) {
      this.host.appendChild(
        h("p", {
          class: "empty",
          text: q
            ? "No matching assets."
            : "No project assets loaded. Click + Add to upload an image/SVG, or open a project folder.",
        }),
      );
    }
  }
}
