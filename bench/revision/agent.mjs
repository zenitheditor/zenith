// Shared agent loop: one conversation, tool calls executed by an arm, usage summed.
import { readFileSync } from "node:fs";
import path from "node:path";
import { chat } from "./llm.mjs";

const PRICES = JSON.parse(readFileSync(path.join(import.meta.dirname, "models.json"), "utf8"));

export const cost = (model, u) => {
  const p = PRICES[model];
  if (!p) return null;
  return ((u.input - u.cached) * p.in + u.cached * p.cached + u.output * p.out) / 1e6;
};

// Replace the images of earlier renders so the context holds only the latest one.
function dropOldImages(messages) {
  for (const m of messages) {
    if (m.role === "user" && Array.isArray(m.content)) {
      m.content = "(earlier render omitted)";
    }
  }
}

/** Counts an edit of the design file by its route: typed `zenith tx`, or raw file tools. */
function countEdit(edits, arm, name, args, text) {
  const failed = /^(error|exit )/.test(text);
  if (name === "zenith" && Array.isArray(args.args) && args.args[0] === "tx") {
    if (!args.args.includes("--apply")) edits.tx_dry++;
    else if (failed) edits.tx_failed++;
    else edits.tx++;
  } else if ((name === "edit_file" || name === "write_file") && args.path === arm.file) {
    if (failed) edits[`${name}_failed`]++;
    else edits[name]++;
  }
}

export const emptyEdits = () => ({ tx: 0, tx_failed: 0, tx_dry: 0, edit_file: 0, edit_file_failed: 0, write_file: 0, write_file_failed: 0 });

// `maxCalls` caps tool calls per step; calls past it get a refusal and end the step.
export async function agentStep({ model, arm, messages, instruction, render, seed, maxTurns, maxCalls = Infinity, log, budgetNotice = false }) {
  messages.push({ role: "user", content: instruction });
  const usage = { input: 0, output: 0, cached: 0 };
  const calls = {};
  const edits = emptyEdits();
  let used = 0;
  let turns = 0;
  let finished = false;
  let error = null;
  while (turns < maxTurns) {
    turns++;
    if (budgetNotice && turns === maxTurns - 1) {
      messages.push({ role: "user", content: "You have 2 turns left. Make sure the design still validates and renders, then finish with one short sentence and no tool call." });
    }
    let reply;
    if (model === "noop") reply = { message: { role: "assistant", content: "done" }, usage };
    else {
      try {
        reply = await chat({ model, messages, tools: arm.tools, seed });
      } catch (err) {
        error = String(err.message ?? err);
        break;
      }
    }
    for (const k of Object.keys(usage)) if (reply.usage !== usage) usage[k] += reply.usage[k];
    const msg = reply.message;
    messages.push({ role: "assistant", content: msg.content ?? "", ...(msg.tool_calls ? { tool_calls: msg.tool_calls } : {}) });
    // `usage` is this turn's request alone; the step total sums them.
    log({ turn: turns, assistant: msg.content ?? "", tool_calls: msg.tool_calls ?? [], finish: reply.finish, usage: reply.usage });
    if (reply.finish === "length") error = "reply cut off at the output token limit";
    if (!msg.tool_calls?.length) {
      finished = true;
      break;
    }
    const images = [];
    for (const call of msg.tool_calls) {
      let args = {};
      try {
        args = JSON.parse(call.function.arguments || "{}");
      } catch {
        args = {};
      }
      if (used >= maxCalls) {
        messages.push({ role: "tool", tool_call_id: call.id, content: "tool budget for this step is spent; the call did not run" });
        error = "tool budget spent";
        continue;
      }
      used++;
      calls[call.function.name] = (calls[call.function.name] ?? 0) + 1;
      const r = await arm.exec(call.function.name, args, render);
      countEdit(edits, arm, call.function.name, args, r.text);
      // `result` is a 1500-char copy; `result_chars` is the full length the model receives.
      log({ turn: turns, tool: call.function.name, args, result: r.text.slice(0, 1500), result_chars: r.text.length, image: Boolean(r.image) });
      messages.push({ role: "tool", tool_call_id: call.id, content: r.text });
      if (r.image) images.push(r.image);
    }
    if (error === "tool budget spent") break;
    if (images.length) dropOldImages(messages);
    for (const img of images) {
      messages.push({
        role: "user",
        content: [
          { type: "text", text: "Current render:" },
          { type: "image_url", image_url: { url: `data:image/png;base64,${img.toString("base64")}` } },
        ],
      });
    }
  }
  return { turns, finished, error, usage, calls, edits };
}

