function setTimeout(fn, ms) {
  const id = Deno.core.ops.op_timer_create();
  Deno.core.ops.op_timer_set_timeout(id, ms).then((armed) => {
    if (armed) fn();
  });
  return id;
}

function clearTimeout(id) {
  Deno.core.ops.op_timer_clear(id);
}

globalThis.setTimeout = setTimeout;
globalThis.clearTimeout = clearTimeout;