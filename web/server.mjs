import { createServer } from "node:http";
import { handler } from "./build/handler.js";

const IMMUTABLE = "public, max-age=31536000, immutable";
const packPattern = /^\/offline\/pack\.[A-Za-z0-9_-]+\.json$/u;
const host = process.env.HOST ?? "0.0.0.0";
const port = Number(process.env.PORT ?? 3000);

if (!Number.isSafeInteger(port) || port < 1 || port > 65_535) {
  throw new Error(`[server] invalid PORT: ${process.env.PORT ?? ""}`);
}

function applyHeaders(response, pathname, statusCode) {
  if (statusCode >= 500) {
    response.setHeader("Cache-Control", "no-store");
  } else if (
    pathname.startsWith("/_app/immutable/") ||
    pathname.startsWith("/_quran/tanzil/") ||
    packPattern.test(pathname)
  ) {
    response.setHeader("Cache-Control", IMMUTABLE);
  } else {
    response.setHeader("Cache-Control", "no-cache");
  }
  if (pathname.endsWith(".md") || pathname.endsWith(".txt")) {
    response.setHeader("X-Robots-Tag", "noindex, follow");
  }
}

const server = createServer((request, response) => {
  const pathname = new URL(request.url ?? "/", "http://localhost").pathname;
  const writeHead = response.writeHead.bind(response);
  response.writeHead = function headerAwareWriteHead(statusCode, reasonPhrase, headers) {
    applyHeaders(this, pathname, statusCode);
    if (typeof reasonPhrase === "string") {
      return writeHead(statusCode, reasonPhrase, headers);
    }
    return writeHead(statusCode, reasonPhrase);
  };

  Promise.resolve(handler(request, response)).catch((cause) => {
    console.error("[server] unhandled request error", cause);
    if (response.headersSent) {
      response.destroy();
      return;
    }
    response.statusCode = 500;
    applyHeaders(response, pathname, 500);
    response.end("Internal Server Error");
  });
});

server.listen(port, host, () => {
  console.log(`[server] listening on http://${host}:${port}`);
});
