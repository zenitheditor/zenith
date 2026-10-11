// Live check of the API: one tool call and one image per model.
// Usage: node bench/revision/smoke.mjs <model>
import { chat } from "./llm.mjs";

// 64x64 solid red PNG.
const RED =
  "iVBORw0KGgoAAAANSUhEUgAAAEAAAABACAYAAACqaXHeAAAAl0lEQVR4nO3QMREAIAzAwCqpf2XYABk/kOH3XObs3p+NDtAaoAO0BugArQE6QGuADtAaoAO0BugArQE6QGuADtAaoAO0BugArQE6QGuADtAaoAO0BugArQE6QGuADtAaoAO0BugArQE6QGuADtAaoAO0BugArQE6QGuADtAaoAO0BugArQE6QGuADtAaoAO0BugArQE6QHsJwBH/+x4FDQAAAABJRU5ErkJggg==";

const model = process.argv[2];
const tools = [
  {
    type: "function",
    function: {
      name: "echo",
      description: "Echo text back.",
      parameters: { type: "object", properties: { text: { type: "string" } }, required: ["text"] },
    },
  },
];

const a = await chat({
  model,
  messages: [{ role: "user", content: "Call the echo tool with text 'ok'." }],
  tools,
});
console.log("tool call:", JSON.stringify(a.message.tool_calls ?? a.message.content), a.usage);

const b = await chat({
  model,
  messages: [
    {
      role: "user",
      content: [
        { type: "text", text: "What single color fills this image? One word." },
        { type: "image_url", image_url: { url: `data:image/png;base64,${RED}` } },
      ],
    },
  ],
});
console.log("image:", JSON.stringify(b.message.content), b.usage);
