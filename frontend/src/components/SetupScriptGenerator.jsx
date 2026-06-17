import { useState } from 'react';
import Modal from './Modal';
import YamlEditor from './YamlEditor';

function generateSetupScript({ host, port }) {
  const certsDir = '/etc/docker/certs';
  const daemonJsonPath = '/etc/docker/daemon.json';
  const hostIP = host || '<SERVER_IP>';
  const dockerPort = port || '2376';

  return `#!/bin/bash
# =============================================================================
# Marionette Remote Docker Setup Script
# Generated for: ${hostIP}:${dockerPort}
# =============================================================================
# Run this script ON THE REMOTE DOCKER HOST as root or with sudo.
# It will:
#   1. Generate TLS certificates (CA, server, client)
#   2. Configure the Docker daemon to listen on TCP with TLS
#   3. Output client cert + connection string for Marionette
# =============================================================================
set -euo pipefail

CERTS_DIR="${certsDir}"
DAEMON_JSON="${daemonJsonPath}"
HOST_IP="${hostIP}"
CLIENT_IP="<MARIONETTE_HOST_IP>"

echo "=== Marionette Remote Docker Setup ==="
echo ""

# ── Step 1: Create cert directory ──────────────────────────────────────────
echo "[1/6] Creating certificate directory: $CERTS_DIR"
mkdir -p "$CERTS_DIR"
chmod 700 "$CERTS_DIR"
cd "$CERTS_DIR"

# ── Step 2: Generate CA ────────────────────────────────────────────────────
echo "[2/6] Generating Certificate Authority (CA)..."
openssl genrsa -aes256 -passout pass:marionette-ca -out ca-key.pem 4096
openssl req -new -x509 -days 3650 -key ca-key.pem -sha256 -passin pass:marionette-ca \\
  -subj "/CN=Marionette-CA" -out ca.pem
chmod 400 ca-key.pem

# ── Step 3: Generate server certificate ────────────────────────────────────
echo "[3/6] Generating server certificate..."
openssl genrsa -out server-key.pem 4096
openssl req -new -key server-key.pem -sha256 \\
  -subj "/CN=$HOST_IP" -out server.csr

# SAN for IP address (needed for IP-based connections)
echo "subjectAltName = IP:$HOST_IP,IP:127.0.0.1" > extfile.cnf
echo "extendedKeyUsage = serverAuth" >> extfile.cnf

openssl x509 -req -days 365 -in server.csr -CA ca.pem -CAkey ca-key.pem \\
  -CAcreateserial -out server-cert.pem -sha256 \\
  -passin pass:marionette-ca -extfile extfile.cnf
chmod 400 server-key.pem
rm server.csr extfile.cnf ca.srl 2>/dev/null || true

# ── Step 4: Generate client certificate ────────────────────────────────────
echo "[4/6] Generating client certificate..."
openssl genrsa -out client-key.pem 4096
openssl req -new -key client-key.pem -sha256 \\
  -subj "/CN=marionette-client" -out client.csr

echo "extendedKeyUsage = clientAuth" > extfile-client.cnf

openssl x509 -req -days 365 -in client.csr -CA ca.pem -CAkey ca-key.pem \\
  -CAcreateserial -out client-cert.pem -sha256 \\
  -passin pass:marionette-ca -extfile extfile-client.cnf
chmod 400 client-key.pem
rm client.csr extfile-client.cnf ca.srl 2>/dev/null || true

# ── Step 5: Update Docker daemon configuration ─────────────────────────────
echo "[5/6] Configuring Docker daemon..."

# Read existing daemon.json or create new one
if [ -f "$DAEMON_JSON" ]; then
  echo "  Backing up existing $DAEMON_JSON to $DAEMON_JSON.bak"
  cp "$DAEMON_JSON" "$DAEMON_JSON.bak"
fi

# Build new config
cat > "$DAEMON_JSON" <<'DAEMONEOF'
{
  "hosts": ["unix:///var/run/docker.sock", "tcp://0.0.0.0:${dockerPort}"],
  "tlsverify": true,
  "tlscacert": "${certsDir}/ca.pem",
  "tlscert": "${certsDir}/server-cert.pem",
  "tlskey": "${certsDir}/server-key.pem"
}
DAEMONEOF

echo "  Restarting Docker daemon..."
systemctl restart docker
sleep 2
systemctl status docker --no-pager | head -5

# ── Step 6: Output connection details ──────────────────────────────────────
echo ""
echo "================================================================"
echo "  SETUP COMPLETE"
echo "================================================================"
echo ""
echo "  Copy the following client files to your Marionette host"
echo "  (e.g., /opt/marionette/certs/):"
echo ""
echo "    - $CERTS_DIR/ca.pem"
echo "    - $CERTS_DIR/client-cert.pem"
echo "    - $CERTS_DIR/client-key.pem"
echo ""
echo "  Connection string for Marionette:"
echo "    https://$HOST_IP:${dockerPort}"
echo ""
echo "  On your Marionette host, copy the client certs to a directory"
echo "  (e.g., /opt/marionette/certs/) and set DOCKER_CERT_PATH:"
echo ""
echo "    export DOCKER_CERT_PATH=/opt/marionette/certs"
echo ""
echo "  To verify from the Marionette host:"
echo "    curl --cacert ca.pem --cert client-cert.pem --key client-key.pem \\\\"
echo "      https://$HOST_IP:${dockerPort}/version"
echo ""
echo "================================================================"
`;
}

export default function SetupScriptGenerator({ onClose }) {
  const [host, setHost] = useState('');
  const [port, setPort] = useState('2376');
  const [script, setScript] = useState('');
  const [copied, setCopied] = useState(false);

  const handleGenerate = () => {
    if (!host.trim()) return;
    setScript(generateSetupScript({ host: host.trim(), port: port.trim() || '2376' }));
  };

  const handleCopy = async () => {
    try {
      await navigator.clipboard.writeText(script);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch {
      // fallback: select text
    }
  };

  return (
    <Modal
      title="Generate Remote Docker Setup Script"
      size="large"
      onClose={onClose}
      footer={
        script ? (
          <>
            <button onClick={onClose}>Close</button>
            <button className="btn-primary" onClick={handleCopy}>
              {copied ? '✓ Copied' : '📋 Copy Script'}
            </button>
          </>
        ) : (
          <>
            <button onClick={onClose}>Cancel</button>
            <button className="btn-primary" onClick={handleGenerate} disabled={!host.trim()}>
              Generate
            </button>
          </>
        )
      }
    >
      {!script ? (
        <div style={{ display: 'grid', gap: '16px', maxWidth: '500px' }}>
          <p style={{ color: 'var(--pico-muted-color)', fontSize: '0.9rem' }}>
            This generates a self-contained bash script that your server admin runs once
            on the remote Docker host. It sets up TLS certificates and configures the
            Docker daemon for secure remote access.
          </p>
          <div>
            <label style={{ display: 'block', marginBottom: '4px', fontWeight: 500 }}>
              Remote Host IP / Hostname
            </label>
            <input
              type="text"
              value={host}
              onChange={e => setHost(e.target.value)}
              placeholder="e.g. 192.168.1.100 or my-server.local"
              style={{ width: '100%' }}
              autoFocus
            />
          </div>
          <div>
            <label style={{ display: 'block', marginBottom: '4px', fontWeight: 500 }}>
              Docker API Port
            </label>
            <input
              type="text"
              value={port}
              onChange={e => setPort(e.target.value)}
              placeholder="2376"
              style={{ width: '120px' }}
            />
            <div style={{ fontSize: '0.75rem', color: 'var(--pico-muted-color)', marginTop: '4px' }}>
              Default: 2376 (standard Docker TLS port). Must be open in the firewall.
            </div>
          </div>
          <div style={{
            padding: '12px',
            background: 'var(--card-bg)',
            border: '1px solid var(--card-border)',
            borderRadius: '6px',
            fontSize: '0.8rem',
            color: 'var(--pico-muted-color)',
          }}>
            <strong>What happens next:</strong>
            <ol style={{ margin: '8px 0 0 16px', lineHeight: '1.8' }}>
              <li>Copy the generated script</li>
              <li>Send it to your server admin</li>
              <li>Admin runs it once on the remote host</li>
              <li>Admin sends you back the client certs + connection details</li>
              <li>Paste the connection string into Marionette</li>
            </ol>
          </div>
        </div>
      ) : (
        <YamlEditor
          value={script}
          onChange={() => {}}
          readOnly={true}
          fill
        />
      )}
    </Modal>
  );
}
