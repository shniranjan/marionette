"use strict";
var __importDefault = (this && this.__importDefault) || function (mod) {
    return (mod && mod.__esModule) ? mod : { "default": mod };
};
Object.defineProperty(exports, "__esModule", { value: true });
const fastify_1 = __importDefault(require("fastify"));
const cors_1 = __importDefault(require("@fastify/cors"));
const http_proxy_1 = __importDefault(require("@fastify/http-proxy"));
const static_1 = __importDefault(require("@fastify/static"));
const path_1 = require("path");
const auth_1 = require("./auth");
const proxy_1 = require("./proxy");
const PORT = parseInt(process.env.PORT || '8000', 10);
const HOST = process.env.HOST || '0.0.0.0';
async function main() {
    const server = (0, fastify_1.default)({
        logger: true,
    });
    // --- CORS (allow all origins) ---
    await server.register(cors_1.default, {
        origin: '*',
        methods: ['GET', 'POST', 'PUT', 'DELETE', 'PATCH', 'OPTIONS', 'HEAD'],
        allowedHeaders: ['Content-Type', 'Authorization', 'X-Marionette-Key'],
        credentials: false,
    });
    // --- Auth middleware for /api/* routes ---
    const authHook = (0, auth_1.createAuthHook)();
    server.addHook('onRoute', (routeOptions) => {
        if (routeOptions.url && routeOptions.url.startsWith(proxy_1.PROXY_PREFIX)) {
            if (!routeOptions.preHandler) {
                routeOptions.preHandler = [];
            }
            else if (typeof routeOptions.preHandler === 'function') {
                routeOptions.preHandler = [routeOptions.preHandler];
            }
            routeOptions.preHandler.push(authHook);
        }
    });
    // --- Proxy /api/* to Rust core ---
    await server.register(http_proxy_1.default, {
        upstream: proxy_1.PROXY_UPSTREAM,
        prefix: proxy_1.PROXY_PREFIX,
        websocket: proxy_1.PROXY_WEBSOCKET,
        replyOptions: {
            rewriteRequestHeaders: proxy_1.rewriteRequestHeaders,
        },
        http2: false,
    });
    // --- Serve SPA static files ---
    const staticRoot = (0, path_1.resolve)(__dirname, '..', '..', 'frontend', 'dist');
    await server.register(static_1.default, {
        root: staticRoot,
        prefix: '/',
        wildcard: false,
    });
    // SPA fallback: any non-/api GET that isn't a static file → index.html
    server.setNotFoundHandler((request, reply) => {
        if (request.url.startsWith(proxy_1.PROXY_PREFIX)) {
            // API routes should have been handled by the proxy; if we reach here,
            // the upstream returned 404, so pass it through.
            reply.status(404).send({ error: 'Not Found' });
            return;
        }
        // For non-API routes, serve the SPA index.html
        reply.sendFile('index.html');
    });
    // --- Start ---
    try {
        await server.listen({ port: PORT, host: HOST });
        server.log.info(`Gateway listening on ${HOST}:${PORT}`);
        server.log.info(`Proxying ${proxy_1.PROXY_PREFIX}/* → ${proxy_1.PROXY_UPSTREAM}`);
        server.log.info(`Serving static files from ${staticRoot}`);
    }
    catch (err) {
        server.log.error(err);
        process.exit(1);
    }
}
main();
//# sourceMappingURL=index.js.map