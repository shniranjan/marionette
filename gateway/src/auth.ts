/**
 * Auth helper functions for the marionette gateway.
 *
 * The gateway checks the X-Marionette-Key header against the MARIONETTE_KEY env var.
 * - If MARIONETTE_KEY is empty/not set → dev mode (allow all)
 * - If MARIONETTE_KEY is set → require matching header value
 * - Supports multiple comma-separated keys
 */

/**
 * Parse a comma-separated env var into an array of trimmed, non-empty keys.
 */
export function extractKeys(envVar: string | undefined): string[] {
  if (!envVar) return [];
  return envVar
    .split(',')
    .map((k) => k.trim())
    .filter((k) => k.length > 0);
}

/**
 * Check whether the gateway is running in development mode
 * (no API keys configured).
 */
export function isDevMode(validKeys: string[]): boolean {
  return validKeys.length === 0;
}

/**
 * Validate a single header value against the list of valid keys.
 * Returns true if the header matches any valid key.
 */
export function validateKey(
  headerValue: string | undefined,
  validKeys: string[],
): boolean {
  if (isDevMode(validKeys)) return true;
  if (!headerValue) return false;
  return validKeys.includes(headerValue);
}

/**
 * Create a Fastify preHandler hook that enforces the X-Marionette-Key header.
 * Returns a preHandler function suitable for fastify.addHook() or route options.
 */
export function createAuthHook() {
  const keys = extractKeys(process.env.MARIONETTE_KEY);

  return async function authHook(
    request: { headers: Record<string, string | undefined> },
    reply: { status: (code: number) => { send: (body: unknown) => void } },
  ) {
    if (isDevMode(keys)) return;

    const headerValue = request.headers['x-marionette-key'];
    if (!validateKey(headerValue, keys)) {
      reply.status(401).send({ error: 'Unauthorized', message: 'Invalid or missing X-Marionette-Key header' });
    }
  };
}
