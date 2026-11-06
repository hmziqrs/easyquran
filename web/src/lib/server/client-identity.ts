import { timingSafeEqual } from "node:crypto";

// Header Cloudflare stamps on every origin request that crossed the edge
// (Transform Rule). Same name and value the API checks (INGRESS_EDGE_MARKER).
const MARKER_HEADER = "x-eq-ingress-marker";
const CF_CLIENT_IP_HEADER = "cf-connecting-ip";

function constantTimeEqual(presented: string, expected: string): boolean {
  const a = Buffer.from(presented);
  const b = Buffer.from(expected);
  if (a.length !== b.length) return false;
  return timingSafeEqual(a, b);
}

/**
 * Client address the web-origin limiter may trust.
 *
 * `cf-connecting-ip` is believed only when the request proves it crossed the
 * edge: `x-eq-ingress-marker` equals `INGRESS_EDGE_MARKER` (constant-time). With
 * no marker configured (local dev) or no proof, the socket peer is used, so an
 * unmarked client can never pick its own bucket by setting the header.
 */
export function trustedClientIp(
  request: Request,
  peer: string,
  edgeMarker: string | undefined,
): string {
  if (edgeMarker === undefined || edgeMarker === "") return peer;
  const presented = request.headers.get(MARKER_HEADER);
  if (presented === null || !constantTimeEqual(presented, edgeMarker)) return peer;
  const forwarded = request.headers.get(CF_CLIENT_IP_HEADER)?.trim() ?? "";
  return forwarded === "" ? peer : forwarded;
}
