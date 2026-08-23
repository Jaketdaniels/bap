try {
  const bad = Bap.numeric.unitAdd(Bap.numeric.unit("metre", 1), Bap.numeric.unit("second", 2));
  console.log(bad);
} catch (error) {
  console.log("rejected:", error.message);
}
