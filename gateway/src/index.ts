import Fastify from 'fastify';
import cors from '@fastify/cors';
import fastifyHttpProxy from '@fastify/http-proxy';
import fastifyStatic from '@fastify/static';
import { readFileSync, existsSync } from 'fs';
import https from 'https';
import { resolve } from 'path';

import { createAuthHook } from './auth';
import {
  PROXY_UPSTREAM,
  PROXY_PREFIX,
  PROXY_WEBSOCKET,
  rewriteRequestHeaders,
} from './proxy';

const PORT = parseInt(process.env.PORT || '8000', 10);
const HOST = process.env.HOST || '0.0.0.0';
const TLS_KEY = process.env.TLS_KEY || '';
const TLS_CERT = process.env.TLS_CERT || '';
const TLS_ENABLED = !!(TLS_KEY && TLS_CERT && existsSync(TLS_KEY) && existsSync(TLS_CERT));

function createServer() {
  const opts: any = { logger: true };
  if (TLS_ENABLED) {
    const key = readFileSync(TLS_KEY);
    const cert = readFileSync(TLS_CERT);
    opts.serverFactory = (handler: any) =>
      https.createServer({ key, cert }, handler);
  }
  return Fastify(opts);
}

async function main() {
  const server = createServer();

  // --- CORS (allow all origins) ---
  (server as any).register(cors, {
    origin: '*',
    methods: ['GET', 'POST', 'PUT', 'DELETE', 'PATCH', 'OPTIONS', 'HEAD'],
    allowedHeaders: ['Content-Type', 'Authorization', 'X-Marionette-Key'],
    credentials: false,
  });

  // --- Auth middleware for /api/* routes ---
  const authHook = createAuthHook();
  (server as any).addHook('onRoute', (routeOptions: any) => {
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
  (server as any).register(fastifyHttpProxy, {
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

  (server as any).register(fastifyStatic, {
    root: staticRoot,
    prefix: '/',
    wildcard: false,
  });

  // SPA fallback: any non-/api GET that isn't a static file → index.html
  (server as any).setNotFoundHandler((request: any, reply: any) => {
    if (request.url.startsWith(PROXY_PREFIX)) {
      reply.status(404).send({ error: 'Not Found' });
      return;
    }
    reply.sendFile('index.html');
  });

  // --- Start ---
  try {
    await server.listen({ port: PORT, host: HOST });
    const protocol = TLS_ENABLED ? 'https' : 'http';
    server.log.info(`Gateway listening on ${protocol}://${HOST}:${PORT}`);
    server.log.info(`Proxying ${PROXY_PREFIX}/* → ${PROXY_UPSTREAM}`);
    server.log.info(`Serving static files from ${staticRoot}`);
    if (!TLS_ENABLED) {
      server.log.warn('TLS not configured — serving HTTP only.');
      server.log.warn('Generate a self-signed cert: scripts/generate-cert.sh');
      server.log.warn('Then set TLS_KEY and TLS_CERT environment variables.');
    }
  } catch (err) {
    server.log.error(err);
    process.exit(1);
  }
}

main();
