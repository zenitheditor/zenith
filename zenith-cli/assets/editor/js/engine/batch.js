// `commands.batch`: several engine commands in one call (see `index.js`).

/**
 * Split the envelope `env` of a `commands.batch` call into one envelope
 * per step, each shaped as if the step ran alone. `steps` are the requests
 * sent. The batch image goes to the step `result.image_step` names. When
 * the batch itself failed (offline, refused), every step gets its error.
 */
export function splitBatch(env, steps) {
  const shared = { version: env.version, dirty: env.dirty };
  if (env.offline) shared.offline = true;
  if (!env.ok) {
    return steps.map((s) => ({ ...shared, ok: false, command: s.command, error: env.error }));
  }
  const replies = env.result?.steps ?? [];
  return steps.map((s, i) => {
    const r = replies[i];
    const out = { ...shared, ok: !!r?.ok, command: s.command, work: r?.work };
    if (r?.ok) out.result = r.result;
    else out.error = r?.error ?? { code: "editor.skipped", message: `'${s.command}' did not run` };
    if (env.image && env.result.image_step === i) out.image = env.image;
    return out;
  });
}

/** The diagnostics a batch reply carries, from every step. */
export function batchDiagnostics(result) {
  const out = [];
  for (const step of result?.steps ?? []) {
    const list = step.ok ? step.result?.diagnostics : step.error?.diagnostics;
    if (Array.isArray(list)) out.push(...list);
  }
  return out;
}
