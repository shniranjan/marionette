#!/bin/sh
# docker-entrypoint.sh — Fix Docker socket permissions at runtime.
# The Docker socket on the host is owned by root:docker (GID varies per host).
# This script runs as root, detects the socket's GID, adds the relay user
# to that group, then drops privileges to run relay-agent.

set -e

SOCKET="/var/run/docker.sock"

if [ -S "$SOCKET" ]; then
    DOCKER_GID=$(stat -c '%g' "$SOCKET" 2>/dev/null || echo "")
    if [ -n "$DOCKER_GID" ] && [ "$DOCKER_GID" != "0" ]; then
        GROUP_NAME="docker-socket"

        # Check if a group with this GID already exists
        if ! getent group "$DOCKER_GID" >/dev/null 2>&1; then
            echo "entrypoint: creating group $GROUP_NAME with GID $DOCKER_GID"
            addgroup --gid "$DOCKER_GID" "$GROUP_NAME" 2>/dev/null || \
                groupadd --gid "$DOCKER_GID" "$GROUP_NAME" 2>/dev/null || true
        else
            GROUP_NAME=$(getent group "$DOCKER_GID" | cut -d: -f1)
            echo "entrypoint: found existing group $GROUP_NAME with GID $DOCKER_GID"
        fi

        # Add relay user to the docker-socket group
        if [ -n "$GROUP_NAME" ]; then
            echo "entrypoint: adding relay to group $GROUP_NAME"
            adduser relay "$GROUP_NAME" 2>/dev/null || \
                usermod -aG "$GROUP_NAME" relay 2>/dev/null || true
        fi
    fi
else
    echo "entrypoint: WARNING — $SOCKET not found, Docker operations will fail"
fi

echo "entrypoint: starting relay-agent as relay user"
exec su relay -c 'exec /usr/local/bin/relay-agent'
