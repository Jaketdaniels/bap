// Per-request unit for `bap bench`: one full render per execution.
import { render } from "./route.js";

const html = await render("/users/42");
if (!html.includes("</html>")) throw new Error("incomplete render");
