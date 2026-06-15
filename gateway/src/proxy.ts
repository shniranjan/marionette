/**
 * Proxy configuration for the marionette gateway.
 *
 * Proxies /api/* requests to the Rust core server at http://127.0.0.1:9119.
 */

import type { IncomingHttpHeaders } from 'http';

export const PROXY_UPSTREAM = 'http://127.0.0.1:9119';
export const PROXY_PREFIX = '/api';

/**
 * Whether to enable WebSocket proxying.
 */
export const PROXY_WEBSOCKET = true;

/**
 * Rewrite request headers before forwarding to upstream.
 * Strips the X-Marionette-Key header since the Rust core doesn't need it.
 */
export function rewriteRequestHeaders(
  _req: unknown,
  headers: IncomingHttpHeaders,
): IncomingHttpHeaders {
  const { 'x-marionette-key': _, ...rest } = headers;
  return rest;
}
