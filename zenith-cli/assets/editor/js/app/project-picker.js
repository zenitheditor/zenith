import { byId, h, plural } from "../util/dom.js";

function choose(project) {
  const dialog = byId("doc-dialog");
  const list = byId("doc-dialog-list");
  byId("doc-dialog-note").textContent = `${project.root} holds ${project.documents.length} documents.`;
  byId("doc-dialog-cancel").onclick = () => dialog.close();
  return new Promise((resolve) => {
    let chosen = null;
    list.replaceChildren(
      ...project.documents.map((path) =>
        h(
          "li",
          {},
          h("button", {
            type: "button",
            class: "button",
            text: path,
            onclick: () => {
              chosen = path;
              dialog.close();
            },
          }),
        ),
      ),
    );
    dialog.addEventListener("close", () => resolve(chosen), { once: true });
    dialog.showModal();
    list.querySelector("button")?.focus();
  });
}

/** Return the selected folder document, or null on cancellation. */
export async function projectDocument(project) {
  const path = project.documents.length === 1 ? project.documents[0] : await choose(project);
  if (!path) return null;
  const bytes = Uint8Array.from(atob(project.files[path]), (c) => c.charCodeAt(0));
  return { name: path.split("/").pop(), text: new TextDecoder().decode(bytes), path, project };
}

export function showSkippedFiles(notices, project) {
  if (!project?.skipped?.length) return;
  notices.show("project-skipped", {
    level: "warning",
    title: project.root,
    message: `Skipped ${plural(project.skipped.length, "file")} due to size limits. Reduce file sizes or open a smaller folder.`,
    detail: project.skipped.join("\n"),
  });
}
