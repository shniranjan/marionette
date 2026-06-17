import Fastify from 'fastify';
import cors from '@fastify/cors';
import fastifyHttpProxy from '@fastify/http-proxy';
import fastifyStatic from '@fastify/static';
import { resolve, join } from 'path';

import { createAuthHook } from './auth';
import {
  PROXY_UPSTREAM,
  PROXY_PREFIX,
  PROXY_WEBSOCKET,
  rewriteRequestHeaders,
} from './proxy';

const PORT = parseInt(process.env.PORT || '8000', 10);
const HOST = process.env.HOST || '0.0.0.0';

async function main() {
  const server = Fastify({
    logger: true,
  });

  // --- CORS (allow all origins) ---
  await server.register(cors, {
    origin: '*',
    methods: ['GET', 'POST', 'PUT', 'DELETE', 'PATCH', 'OPTIONS', 'HEAD'],
    allowedHeaders: ['Content-Type', 'Authorization', 'X-Marionette-Key'],
    credentials: false,
  });

  // --- Auth middleware for /api/* routes ---
  const authHook = createAuthHook();
  server.addHook('onRoute', (routeOptions) => {
    if (routeOptions.url && routeOptions.url.startsWith(PROXY_PREFIX)) {
      if (!routeOptions.preHandler) {
        routeOptions.preHandler = [];
      } else if (typeof routeOptions.preHandler === 'function') {
        routeOptions.preHandler = [routeOptions.preHandler];
      }
      (routeOptions.preHandler as unknown[]).push(authHook);
    }
  });

  // --- Proxy /api/* to Rust core ---
  await server.register(fastifyHttpProxy, {
    upstream: PROXY_UPSTREAM,
    prefix: PROXY_PREFIX,
    rewritePrefix: '/',
    websocket: PROXY_WEBSOCKET,
    replyOptions: {
      rewriteRequestHeaders,
    },
    http2: false,
  });

  // --- Serve SPA static files ---
  const staticRoot = resolve(__dirname, '..', '..', 'frontend', 'dist');

  await server.register(fastifyStatic, {
    root: staticRoot,
    prefix: '/',
    wildcard: false,
  });

  // SPA fallback: any non-/api GET that isn't a static file → index.html
  server.setNotFoundHandler((request, reply) => {
    if (request.url.startsWith(PROXY_PREFIX)) {
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
    server.log.info(`Proxying ${PROXY_PREFIX}/* → ${PROXY_UPSTREAM}`);
    server.log.info(`Serving static files from ${staticRoot}`);
  } catch (err) {
    server.log.error(err);
    process.exit(1);
  }
}

main();
