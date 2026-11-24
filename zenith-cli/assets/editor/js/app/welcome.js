import { canPickDirectory, pickDocument, pickProject, readDirectoryHandle } from "../engine/wasm/files.js";
import { byId } from "../util/dom.js";
import { load, save } from "../util/store.js";
import { projectDocument } from "./project-picker.js";
import { blankPreset } from "./samples.js";
import { loadFolderHandle } from "./workspace-store.js";
import { hydrateIcons } from "../ui/icons.js";
import { openNewDocDialog } from "../ui/newdoc.js";

function showGallery() {
  if (document.querySelector('meta[name="zenith-gallery"]')?.content !== "true") return;
  byId("welcome-poster-image").src = "starters/poster.png";
  byId("welcome-report-image").src = "starters/harbor-report.png";
  byId("welcome-diagram-image").src = "starters/print-workflow.png";
  byId("welcome-slide-image").src = "starters/keynote-slide.png";
  byId("welcome-spec-image").src = "starters/spec-sheet.png";
  byId("welcome-card-image").src = "starters/launch-card.png";
  byId("welcome-poster").hidden = false;
  byId("welcome-gallery").hidden = false;
}

async function pickFolder() {
  const project = await pickProject();
  if (!project) return null;
  if (!project.documents.length) {
    throw new Error(`${project.root} has no .zen file. Choose another folder.`);
  }
  return projectDocument(project);
}

export function showWelcome(open) {
  const welcome = byId("welcome");
  const choices = byId("welcome-choices");
  const status = byId("welcome-status");
  byId("app").hidden = true;
  byId("skip-source").hidden = true;
  showGallery();
  hydrateIcons(welcome);
  welcome.hidden = false;

  async function choose(button, pick) {
    if (choices.inert) return;
    choices.inert = true;
    status.textContent = "";
    status.dataset.error = "false";
    try {
      const spec = await pick();
      if (!spec) return;
      status.textContent = `Opening ${spec.name}…`;
      await open(spec);
    } catch (error) {
      status.textContent = error.message;
      status.dataset.error = "true";
    } finally {
      choices.inert = false;
      if (!welcome.hidden && !choices.hidden) button?.focus();
    }
  }

  for (const btn of welcome.querySelectorAll("[data-preset]")) {
    btn.addEventListener("click", () => {
      if (btn.dataset.preset === "custom") {
        openNewDocDialog((spec) => choose(btn, async () => spec), { preset: "custom" });
      } else {
        choose(btn, async () => blankPreset(btn.dataset.preset));
      }
    });
  }

  const device = async (pick) => {
    const spec = await pick();
    return spec && { ...spec, draftSource: { kind: "device" } };
  };

  byId("welcome-open")?.addEventListener("click", () => choose(byId("welcome-open"), () => device(pickDocument)));
  const canDir = canPickDirectory() || "webkitdirectory" in document.createElement("input");
  const folderBtn = byId("welcome-folder");
  if (folderBtn && canDir) {
    folderBtn.hidden = false;
    folderBtn.addEventListener("click", () => choose(folderBtn, () => device(pickFolder)));
  }
  const reconnectBtn = byId("welcome-reconnect");
  if (reconnectBtn && canDir) {
    void loadFolderHandle().then((dir) => {
      if (!dir) return;
      reconnectBtn.hidden = false;
      const lbl = byId("welcome-reconnect-label");
      if (lbl) lbl.textContent = `Reconnect ${dir.name}`;
      reconnectBtn.onclick = () =>
        choose(reconnectBtn, async () => {
          const project = await readDirectoryHandle(dir);
          if (!project) return null;
          if (!project.documents.length) throw new Error(`${project.root} has no .zen file.`);
          const spec = await projectDocument(project);
          return spec && { ...spec, draftSource: { kind: "device" } };
        });
    });
  }

  // Open dialog trigger from Welcome header
  const openDlg = byId("open-dlg");
  byId("welcome-open-dlg")?.addEventListener("click", () => {
    if (!openDlg) return;
    hydrateIcons(openDlg);
    openDlg.showModal();
  });
  byId("open-dlg-close")?.addEventListener("click", () => openDlg?.close());
  openDlg?.addEventListener("click", (e) => {
    if (e.target === openDlg) openDlg.close();
  });
  byId("open-device-file")?.addEventListener("click", () => {
    openDlg?.close();
    choose(byId("welcome-open"), () => device(pickDocument));
  });
  const devFolder = byId("open-device-folder");
  if (devFolder) {
    devFolder.hidden = !canDir;
    devFolder.addEventListener("click", () => {
      openDlg?.close();
      choose(byId("welcome-folder"), () => device(pickFolder));
    });
  }
  const storedLayout = load("layout", {});
  if (storedLayout.theme === "dark" || storedLayout.theme === "light") {
    document.documentElement.dataset.theme = storedLayout.theme;
  } else {
    document.documentElement.removeAttribute("data-theme");
  }
  byId("welcome-theme")?.addEventListener("click", () => {
    const doc = document.documentElement;
    const isDark = doc.dataset.theme === "dark"
      || (!doc.dataset.theme && window.matchMedia?.("(prefers-color-scheme: dark)").matches);
    const next = isDark ? "light" : "dark";
    doc.dataset.theme = next;
    save("layout", { ...load("layout", {}), theme: next });
  });
}
