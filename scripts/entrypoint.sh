#!/bin/sh
set -e

# Create stacks directory if it doesn't exist
mkdir -p /stacks

# Start supervisor (manages core + gateway)
exec /usr/bin/supervisord -c /app/supervisord.conf
