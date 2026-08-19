const fired = [];
await new Promise((r) => {
  const id = setTimeout(() => fired.push("fired"), 5);
  clearTimeout(id);
  setTimeout(() => r(), 20);
});
if (fired.length !== 0) throw new Error("cleared timer must not fire");