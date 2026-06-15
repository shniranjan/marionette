"use strict";
/**
 * Proxy configuration for the marionette gateway.
 *
 * Proxies /api/* requests to the Rust core server at http://127.0.0.1:9119.
 */
Object.defineProperty(exports, "__esModule", { value: true });
exports.PROXY_WEBSOCKET = exports.PROXY_PREFIX = exports.PROXY_UPSTREAM = void 0;
exports.rewriteRequestHeaders = rewriteRequestHeaders;
exports.PROXY_UPSTREAM = 'http://127.0.0.1:9119';
exports.PROXY_PREFIX = '/api';
/**
 * Whether to enable WebSocket proxying.
 */
exports.PROXY_WEBSOCKET = true;
/**
 * Rewrite request headers before forwarding to upstream.
 * Strips the X-Marionette-Key header since the Rust core doesn't need it.
 */
function rewriteRequestHeaders(_req, headers) {
    const { 'x-marionette-key': _, ...rest } = headers;
    return rest;
}
//# sourceMappingURL=proxy.js.map