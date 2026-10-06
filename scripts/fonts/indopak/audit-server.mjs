import { createServer } from "vite-plus";
import path from "node:path";
import { fileURLToPath } from "node:url";

const web = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../../../web");
const port = Number(process.env.INDOPAK_SPECIMEN_PORT ?? 5391);
if (!Number.isSafeInteger(port) || port < 1 || port > 65535)
  throw new Error("Invalid specimen port");
process.env.PUBLIC_ENV ??= "local";
process.chdir(web);
const server = await createServer({
  server: { host: "0.0.0.0", port, strictPort: true, hmr: false },
});
await server.listen();
server.printUrls();
