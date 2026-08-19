export const result = await new Promise((r) => setTimeout(() => r("ok"), 10));
if (result !== "ok") throw new Error("timer result mismatch");