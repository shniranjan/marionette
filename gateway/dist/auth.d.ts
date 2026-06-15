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
export declare function extractKeys(envVar: string | undefined): string[];
/**
 * Check whether the gateway is running in development mode
 * (no API keys configured).
 */
export declare function isDevMode(validKeys: string[]): boolean;
/**
 * Validate a single header value against the list of valid keys.
 * Returns true if the header matches any valid key.
 */
export declare function validateKey(headerValue: string | undefined, validKeys: string[]): boolean;
/**
 * Create a Fastify preHandler hook that enforces the X-Marionette-Key header.
 * Returns a preHandler function suitable for fastify.addHook() or route options.
 */
export declare function createAuthHook(): (request: {
    headers: Record<string, string | undefined>;
}, reply: {
    status: (code: number) => {
        send: (body: unknown) => void;
    };
}) => Promise<void>;
