const order = [];
await new Promise((r) => {
  setTimeout(() => {
    order.push("first");
    r();
  }, 30);
  setTimeout(() => order.push("second"), 5);
});
if (order[0] !== "second") throw new Error("shorter timer must fire first");