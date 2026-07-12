#!/bin/bash
set -e

echo "=== Marionette starting ==="
echo "Core port:  ${MARIONETTE_PORT:-9119}"
echo "Gateway port: ${MARIONETTE_GATEWAY_PORT:-3000}"
echo "Relay port:  ${MARIONETTE_RELAY_PORT:-9120}"
echo ""

# If relay-agent is the command, run it directly (used by docker-compose.prod.yml relay profile)
if [ "$1" = "relay-agent" ]; then
    exec /usr/local/bin/relay-agent
fi

# Default: run supervisord with marionette-core + frontend server
exec /usr/bin/supervisord -c /etc/supervisord.conf
