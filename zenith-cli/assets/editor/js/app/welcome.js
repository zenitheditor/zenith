import { canPickDirectory, pickDocument, pickProject } from "../engine/wasm/files.js";
import { byId } from "../util/dom.js";
import { projectDocument } from "./project-picker.js";
import { loadSample } from "./samples.js";

function showGallery() {
  if (document.querySelector('meta[name="zenith-gallery"]')?.content !== "true") return;
  byId("welcome-poster-image").src = "starters/poster.png";
  byId("welcome-report-image").src = "starters/article.png";
  byId("welcome-diagram-image").src = "starters/flowchart.png";
  byId("welcome-poster-image").hidden = false;
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
      if (!welcome.hidden && !choices.hidden) button.focus();
    }
  }

  function bind(id, pick) {
    const button = byId(id);
    button.addEventListener("click", () => choose(button, pick));
  }

  bind("welcome-blank", async () => ({
    ...(await loadSample("starters/blank.zen")), name: "Untitled.zen", draftSource: { kind: "blank" },
  }));
  const device = async (pick) => {
    const spec = await pick();
    return spec && { ...spec, draftSource: { kind: "device" } };
  };
  bind("welcome-open", () => device(pickDocument));
  if (canPickDirectory() || "webkitdirectory" in document.createElement("input")) {
    byId("welcome-folder").hidden = false;
    bind("welcome-folder", () => device(pickFolder));
  }
}
