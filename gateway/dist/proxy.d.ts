/**
 * Proxy configuration for the marionette gateway.
 *
 * Proxies /api/* requests to the Rust core server at http://127.0.0.1:9119.
 */
import type { IncomingHttpHeaders } from 'http';
export declare const PROXY_UPSTREAM = "http://127.0.0.1:9119";
export declare const PROXY_PREFIX = "/api";
/**
 * Whether to enable WebSocket proxying.
 */
export declare const PROXY_WEBSOCKET = true;
/**
 * Rewrite request headers before forwarding to upstream.
 * Strips the X-Marionette-Key header since the Rust core doesn't need it.
 */
export declare function rewriteRequestHeaders(_req: unknown, headers: IncomingHttpHeaders): IncomingHttpHeaders;
