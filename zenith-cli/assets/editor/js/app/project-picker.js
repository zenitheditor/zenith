import { hydrateIcons } from "../ui/icons.js";
import { byId, h, plural } from "../util/dom.js";

function decodeText(b64) {
  const bytes = Uint8Array.from(atob(b64), (c) => c.charCodeAt(0));
  return new TextDecoder().decode(bytes);
}

/** Classify a picked folder's `.zen` files into primary Designs vs. Libraries. */
export function classifyProject(project) {
  const designs = [];
  const libraries = [];
  for (const path of project.documents ?? []) {
    const parts = path.toLowerCase().split("/");
    const inLibDir = parts.some((p) => p === "library" || p === "libraries");
    let isLib = inLibDir;
    if (!isLib && project.files?.[path]) {
      try {
        const text = decodeText(project.files[path]);
        if (/\blibrary\b/.test(text) && !/\bdocument\b/.test(text)) isLib = true;
      } catch {
        // Ignore decode errors; treat as design.
      }
    }
    if (isLib) libraries.push(path);
    else designs.push(path);
  }
  return { designs, libraries };
}

function renderGroup(list, label, note, paths, dimmed, pick) {
  if (!paths.length) return;
  list.appendChild(
    h(
      "li",
      { class: "fr-group" },
      label,
      note ? h("span", { class: "muted", text: ` · ${note}` }) : null,
    ),
  );
  for (const path of paths) {
    const parts = path.split("/");
    const filename = parts.pop();
    list.appendChild(
      h(
        "li",
        {},
        h(
          "button",
          {
            type: "button",
            class: `button project-document${dimmed ? " dim" : ""}`,
            dataset: { path },
            onclick: () => pick(path),
          },
          h("span", { text: filename }),
          parts.length ? h("small", { text: `${parts.join("/")}/` }) : null,
        ),
      ),
    );
  }
}

function choose(project) {
  const dialog = byId("doc-dialog");
  hydrateIcons(dialog);
  const list = byId("doc-dialog-list");
  const { designs, libraries } = classifyProject(project);
  const totalAssets = Math.max(0, Object.keys(project.files ?? {}).length - (project.documents?.length ?? 0));
  byId("doc-dialog-note").textContent = [
    project.root,
    plural(designs.length, "design"),
    libraries.length ? plural(libraries.length, "library", "libraries") : null,
    totalAssets ? plural(totalAssets, "asset") : null,
  ]
    .filter(Boolean)
    .join(" · ");
  byId("doc-dialog-cancel").onclick = () => dialog.close();
  return new Promise((resolve) => {
    let chosen = null;
    const pick = (path) => {
      chosen = path;
      dialog.close();
    };
    list.replaceChildren();
    renderGroup(list, "Designs", null, designs, false, pick);
    renderGroup(list, "Libraries", "used by designs, not opened directly", libraries, true, pick);
    dialog.addEventListener("close", () => resolve(chosen), { once: true });
    dialog.showModal();
    list.querySelector("button")?.focus();
  });
}

/** Return the selected folder document, or null on cancellation. */
export async function projectDocument(project) {
  const { designs } = classifyProject(project);
  const path =
    project.documents.length === 1
      ? project.documents[0]
      : designs.length === 1 && project.documents.length === 1
        ? designs[0]
        : await choose(project);
  if (!path) return null;
  return { name: path.split("/").pop(), text: decodeText(project.files[path]), path, project };
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
