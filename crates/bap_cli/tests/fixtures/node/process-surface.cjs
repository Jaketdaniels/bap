function check(condition, message) {
  if (!condition) throw new Error(message);
}

check(typeof process.cwd() === "string", "cwd must be a string");
check(process.platform.length > 0, "platform must be non-empty");
check(process.arch.length > 0, "arch must be non-empty");
check(process.version.includes("bap"), "version should identify bap");
check(process.env.BAP_PROCESS_PROBE === "visible", "env probe missing");
let ticked = false;
process.nextTick(() => {
  ticked = true;
  console.log("process surface ok");
});
check(ticked === false, "nextTick ran synchronously");
