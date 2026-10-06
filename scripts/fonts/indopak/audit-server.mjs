import { createServer } from "vite-plus";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const web = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../../../web");
const port = Number(process.env.INDOPAK_SPECIMEN_PORT ?? 5391);
if (!Number.isSafeInteger(port) || port < 1 || port > 65535)
  throw new Error("Invalid specimen port");
process.env.PUBLIC_ENV ??= "local";
const serverOptions = { host: "0.0.0.0", port, strictPort: true, hmr: false };
if (process.env.INDOPAK_AUDIT_ALLOW)
  serverOptions.fs = { allow: [path.resolve(process.env.INDOPAK_AUDIT_ALLOW)] };
const simulatorOutput = process.env.INDOPAK_SIMULATOR_OUTPUT;
const plugins = [];
if (simulatorOutput) {
  const destination = path.resolve(simulatorOutput);
  await mkdir(destination, { recursive: true });
  plugins.push({
    name: "indopak-simulator-evidence",
    configureServer(instance) {
      instance.middlewares.use("/__indopak_simulator_module", async (request, response) => {
        const url = new URL(request.url, "http://localhost");
        const label = url.searchParams.get("label");
        if (!label || !/^[a-z0-9-]{1,80}$/.test(label)) {
          response.writeHead(400).end("Invalid simulator state");
          return;
        }
        try {
          const modulePath = path.join(web, ".svelte-kit/indopak-simulator-probe.mjs");
          const source = await readFile(modulePath, "utf8");
          response
            .writeHead(200, {
              "content-type": "text/javascript; charset=utf-8",
              "cache-control": "no-store",
            })
            .end(`${source}\nrun(${JSON.stringify(label)});`);
        } catch (error) {
          response.writeHead(500).end(String(error));
        }
      });
      instance.middlewares.use("/__indopak_simulator_report", async (request, response) => {
        if (request.method !== "POST") {
          response.writeHead(405).end();
          return;
        }
        try {
          const chunks = [];
          let bytes = 0;
          for await (const chunk of request) {
            bytes += chunk.length;
            if (bytes > 10_000_000) throw new Error("Oversize simulator evidence");
            chunks.push(chunk);
          }
          const report = JSON.parse(Buffer.concat(chunks).toString("utf8"));
          if (!/^[a-z0-9-]{1,80}$/.test(report.label)) throw new Error("Invalid evidence label");
          await writeFile(
            path.join(destination, `${report.label}.json`),
            JSON.stringify(report, null, 2) + "\n",
          );
          response.writeHead(200, { "content-type": "application/json" }).end('{"saved":true}');
        } catch (error) {
          response.writeHead(400, { "content-type": "text/plain" }).end(String(error));
        }
      });
    },
  });
}
process.chdir(web);
const server = await createServer({
  server: serverOptions,
  plugins,
});
await server.listen();
server.printUrls();
