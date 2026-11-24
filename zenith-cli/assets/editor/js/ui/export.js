// Full multi-format Export & Project Bundle (.zip) dialog.
// Supports PNG, JPEG, WebP (with scale & quality), vector SVG, strict PDF
// (Digital sRGB / Print CMYK, current page or all pages), .zen source, and
// .zip project bundle (.zen + project assets/fonts + optional PDF).

import { download, downloadBlob } from "../engine/wasm/files.js";
import { byId } from "../util/dom.js";
import { buildZip } from "../util/zip.js";

function base64ToBytes(b64) {
  const bin = atob(b64);
  const out = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
  return out;
}

async function transcodeImage(pngBlob, mime, quality) {
  const bitmap = await createImageBitmap(pngBlob);
  try {
    const canvas = document.createElement("canvas");
    canvas.width = bitmap.width;
    canvas.height = bitmap.height;
    const ctx = canvas.getContext("2d");
    if (mime === "image/jpeg") {
      ctx.fillStyle = "#ffffff";
      ctx.fillRect(0, 0, canvas.width, canvas.height);
    }
    ctx.drawImage(bitmap, 0, 0);
    return await new Promise((resolve, reject) => {
      canvas.toBlob((b) => (b ? resolve(b) : reject(new Error(`Browser could not encode ${mime}.`))), mime, quality);
    });
  } finally {
    bitmap.close();
  }
}

export class ExportDialog {
  constructor(app) {
    this.app = app;
    this.dlg = byId("export-dlg");
    this.format = "png";
    this.scale = 2;
    this.quality = 0.9;
    this.pdfPreset = "srgb";
    this.pdfScope = "all";
    this.zipIncludePdf = true;
    this.busy = false;
    this.bind();
  }

  bind() {
    if (!this.dlg) return;
    byId("export-dlg-close")?.addEventListener("click", () => this.dlg.close());
    this.dlg.addEventListener("click", (e) => {
      if (e.target === this.dlg) this.dlg.close();
    });
    for (const btn of this.dlg.querySelectorAll("[data-exp-fmt]")) {
      btn.addEventListener("click", () => {
        this.format = btn.dataset.expFmt;
        this.render();
      });
    }
    for (const btn of this.dlg.querySelectorAll("[data-exp-scale]")) {
      btn.addEventListener("click", () => {
        this.scale = Number(btn.dataset.expScale) || 1;
        this.render();
      });
    }
    byId("exp-quality")?.addEventListener("input", (e) => {
      this.quality = Number(e.target.value) / 100;
      const val = byId("exp-quality-val");
      if (val) val.textContent = `${e.target.value}%`;
    });
    byId("exp-pdf-preset")?.addEventListener("change", (e) => {
      this.pdfPreset = e.target.value;
    });
    byId("exp-pdf-scope")?.addEventListener("change", (e) => {
      this.pdfScope = e.target.value;
    });
    byId("exp-zip-pdf")?.addEventListener("change", (e) => {
      this.zipIncludePdf = e.target.checked;
    });
    byId("exp-run")?.addEventListener("click", () => this.run());
  }

  open() {
    if (!this.dlg) return;
    this.render();
    if (!this.dlg.open) this.dlg.showModal();
  }

  stem() {
    return (this.app.name || "design.zen").replace(/\.zen$/i, "") || "design";
  }

  render() {
    if (!this.dlg) return;
    for (const btn of this.dlg.querySelectorAll("[data-exp-fmt]")) {
      const on = btn.dataset.expFmt === this.format;
      btn.classList.toggle("on", on);
      btn.setAttribute("aria-pressed", String(on));
    }
    for (const btn of this.dlg.querySelectorAll("[data-exp-scale]")) {
      const on = Number(btn.dataset.expScale) === this.scale;
      btn.classList.toggle("on", on);
      btn.setAttribute("aria-pressed", String(on));
    }
    const isRaster = ["png", "jpeg", "webp"].includes(this.format);
    const hasQuality = ["jpeg", "webp"].includes(this.format);
    byId("exp-raster-opts").hidden = !isRaster;
    byId("exp-quality-row").hidden = !hasQuality;
    byId("exp-pdf-opts").hidden = this.format !== "pdf";
    byId("exp-zip-opts").hidden = this.format !== "zip";

    const pg = this.app.inspector?.outline?.pages?.find((p) => p.page === this.app.page);
    const dimEl = byId("exp-dims");
    if (dimEl && pg) {
      dimEl.textContent = `${Math.round(pg.w * this.scale)} × ${Math.round(pg.h * this.scale)} px`;
    }
    const assetCount = Object.keys(this.app.host?.worker?.files ?? {}).length;
    const zipSummary = byId("exp-zip-summary");
    if (zipSummary) {
      zipSummary.textContent = assetCount
        ? `Includes ${this.app.name} + ${assetCount} project file${assetCount === 1 ? "" : "s"}.`
        : `Includes ${this.app.name} (no linked project files loaded).`;
    }
    const blocked = !this.app.valid || this.app.stale;
    const needsValid = this.format !== "zen" && this.format !== "zip";
    const runBtn = byId("exp-run");
    if (runBtn) {
      runBtn.disabled = this.busy || (needsValid && blocked);
      const label = {
        png: `Export PNG (${this.scale}×)`,
        jpeg: `Export JPEG (${this.scale}×)`,
        webp: `Export WebP (${this.scale}×)`,
        svg: "Export Vector SVG",
        pdf: "Export PDF",
        zen: "Download .zen",
        zip: "Download .zip Bundle",
      }[this.format];
      runBtn.textContent = this.busy ? "Exporting…" : label;
    }
  }

  async run() {
    if (this.busy) return;
    this.busy = true;
    this.render();
    const stem = this.stem();
    const suffix = this.scale > 1 ? `@${this.scale}x` : "";
    try {
      await this.app.intents.run(() => this.app.sync.exclusive(async () => {
        if (this.format === "zen") {
          download(this.app.name, this.app.code.text());
          this.app.notices.toast(`Downloaded ${this.app.name}.`);
          return;
        }
        if (!(await this.app.sync.flushNow())) throw new Error("Sync the editor text before exporting.");
        if (this.format === "png" || this.format === "jpeg" || this.format === "webp") {
          const env = await this.app.engine.run("doc.render", { page: this.app.page, scale: this.scale });
          if (!env.ok) throw new Error(env.error?.message || "Render failed.");
          if (!env.image) throw new Error("Render returned no image.");
          const pngBlob = await this.app.engine.image(env.image);
          if (this.format === "png") {
            downloadBlob(`${stem}${suffix}.png`, pngBlob);
          } else {
            const mime = this.format === "jpeg" ? "image/jpeg" : "image/webp";
            const ext = this.format === "jpeg" ? "jpg" : "webp";
            const out = await transcodeImage(pngBlob, mime, this.quality);
            downloadBlob(`${stem}${suffix}.${ext}`, out);
          }
          this.app.notices.toast(`Exported ${stem}${suffix}.${this.format === "jpeg" ? "jpg" : this.format}.`);
        } else if (this.format === "svg") {
          const env = await this.app.engine.run("doc.export", { format: "svg", page: this.app.page });
          if (!env.ok) throw new Error(env.error?.message || "SVG export failed.");
          const blob = new Blob([base64ToBytes(env.result.base64)], { type: "image/svg+xml" });
          downloadBlob(`${stem}.svg`, blob);
          this.app.notices.toast(`Exported ${stem}.svg.`);
        } else if (this.format === "pdf") {
          const params = {
            format: "pdf",
            print: this.pdfPreset === "cmyk",
            colorspace: this.pdfPreset === "cmyk" ? "cmyk" : "srgb",
          };
          if (this.pdfScope === "current") params.page = this.app.page;
          const env = await this.app.engine.run("doc.export", params);
          if (!env.ok) throw new Error(env.error?.message || "PDF export failed.");
          const blob = new Blob([base64ToBytes(env.result.base64)], { type: "application/pdf" });
          downloadBlob(`${stem}.pdf`, blob);
          this.app.notices.toast(`Exported ${stem}.pdf.`);
        } else if (this.format === "zip") {
          const enc = new TextEncoder();
          const entries = [{ name: this.app.name, data: enc.encode(this.app.code.text()) }];
          const files = this.app.host?.worker?.files ?? {};
          for (const [rel, b64] of Object.entries(files)) {
            if (rel === this.app.name || rel === this.app.path) continue;
            entries.push({ name: rel, data: base64ToBytes(b64) });
          }
          if (this.zipIncludePdf && this.app.valid && !this.app.stale) {
            const env = await this.app.engine.run("doc.export", { format: "pdf", colorspace: "srgb" });
            if (env.ok && env.result?.base64) {
              entries.push({ name: `${stem}.pdf`, data: base64ToBytes(env.result.base64) });
            }
          }
          const zip = buildZip(entries);
          downloadBlob(`${stem}-bundle.zip`, zip);
          this.app.notices.toast(`Exported ${stem}-bundle.zip (${entries.length} files).`);
        }
      }));
      this.dlg.close();
    } catch (err) {
      this.app.notices.error("doc.export", { code: "export.failed", message: err.message });
    } finally {
      this.busy = false;
      this.render();
    }
  }
}
