import {
  createServer,
  type IncomingMessage,
  type OutgoingHttpHeader,
  type OutgoingHttpHeaders,
  type ServerResponse,
} from "node:http";

// Emitted by adapter-node at build time, so it carries no types of its own.
import { handler } from "./build/handler.js";

type RequestHandler = (request: IncomingMessage, response: ServerResponse) => void | Promise<void>;

/** Both `writeHead` overloads as one tuple: the optional middle arg is a status message or headers. */
type WriteHeadArgs = [
  statusCode: number,
  statusMessageOrHeaders?: string | OutgoingHttpHeaders | OutgoingHttpHeader[],
  headers?: OutgoingHttpHeaders | OutgoingHttpHeader[],
];

const IMMUTABLE = "public, max-age=31536000, immutable";
const packPattern = /^\/offline\/pack\.[A-Za-z0-9_-]+\.json$/u;
const host = process.env.HOST ?? "0.0.0.0";
const port = Number(process.env.PORT ?? 3000);

if (!Number.isSafeInteger(port) || port < 1 || port > 65_535) {
  throw new Error(`[server] invalid PORT: ${process.env.PORT ?? ""}`);
}

function applyHeaders(response: ServerResponse, pathname: string, statusCode: number): void {
  if (statusCode >= 500) {
    response.setHeader("Cache-Control", "no-store");
  } else if (
    pathname.startsWith("/_app/immutable/") ||
    pathname.startsWith("/_quran/") ||
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

const server = createServer((request: IncomingMessage, response: ServerResponse) => {
  const pathname = new URL(request.url ?? "/", "http://localhost").pathname;
  const writeHead = response.writeHead.bind(response);
  // Forward args verbatim — writeHead's middle arg (statusMessage) is optional,
  // so reshaping the call risks dropping the headers object.
  //
  // `writeHead` is overloaded, and `Parameters<>` collapses to the last overload
  // `(statusCode, headers?)`. Spelling both shapes as one rest tuple keeps every real call
  // forwarding untouched.
  // SAFETY: the rest tuple spells both writeHead overloads and the assertions re-attach
  // the overloaded type on the way out.
  response.writeHead = function headerAwareWriteHead(
    this: ServerResponse,
    ...args: WriteHeadArgs
  ): ServerResponse {
    applyHeaders(this, pathname, args[0]);
    // SAFETY: writeHead was captured from this response; the tuple spells its overloads.
    return (writeHead as (...forwarded: WriteHeadArgs) => ServerResponse)(...args);
  } as ServerResponse["writeHead"];

  // SAFETY: adapter-node emits handler.js without types; RequestHandler spells its real signature.
  Promise.resolve((handler as RequestHandler)(request, response)).catch((cause: unknown) => {
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
