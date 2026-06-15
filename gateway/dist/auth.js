"use strict";
/**
 * Auth helper functions for the marionette gateway.
 *
 * The gateway checks the X-Marionette-Key header against the MARIONETTE_KEY env var.
 * - If MARIONETTE_KEY is empty/not set → dev mode (allow all)
 * - If MARIONETTE_KEY is set → require matching header value
 * - Supports multiple comma-separated keys
 */
Object.defineProperty(exports, "__esModule", { value: true });
exports.extractKeys = extractKeys;
exports.isDevMode = isDevMode;
exports.validateKey = validateKey;
exports.createAuthHook = createAuthHook;
/**
 * Parse a comma-separated env var into an array of trimmed, non-empty keys.
 */
function extractKeys(envVar) {
    if (!envVar)
        return [];
    return envVar
        .split(',')
        .map((k) => k.trim())
        .filter((k) => k.length > 0);
}
/**
 * Check whether the gateway is running in development mode
 * (no API keys configured).
 */
function isDevMode(validKeys) {
    return validKeys.length === 0;
}
/**
 * Validate a single header value against the list of valid keys.
 * Returns true if the header matches any valid key.
 */
function validateKey(headerValue, validKeys) {
    if (isDevMode(validKeys))
        return true;
    if (!headerValue)
        return false;
    return validKeys.includes(headerValue);
}
/**
 * Create a Fastify preHandler hook that enforces the X-Marionette-Key header.
 * Returns a preHandler function suitable for fastify.addHook() or route options.
 */
function createAuthHook() {
    const keys = extractKeys(process.env.MARIONETTE_KEY);
    return async function authHook(request, reply) {
        if (isDevMode(keys))
            return;
        const headerValue = request.headers['x-marionette-key'];
        if (!validateKey(headerValue, keys)) {
            reply.status(401).send({ error: 'Unauthorized', message: 'Invalid or missing X-Marionette-Key header' });
        }
    };
}
//# sourceMappingURL=auth.js.map