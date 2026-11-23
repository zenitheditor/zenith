import { canPickDirectory, pickDocument, pickProject } from "../engine/wasm/files.js";
import { byId } from "../util/dom.js";
import { projectDocument } from "./project-picker.js";
import { blankPreset } from "./samples.js";
import { hydrateIcons } from "../ui/icons.js";

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
    btn.addEventListener("click", () => choose(btn, async () => blankPreset(btn.dataset.preset)));
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
  byId("welcome-theme")?.addEventListener("click", () => {
    const h = document.documentElement;
    h.dataset.theme = h.dataset.theme === "dark" ? "light" : "dark";
  });
}
