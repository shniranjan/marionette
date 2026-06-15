const STORAGE_KEY = 'marionette-auth-key';

export function setKey(key) {
  localStorage.setItem(STORAGE_KEY, key);
}

export function getKey() {
  return localStorage.getItem(STORAGE_KEY);
}

function clearKey() {
  localStorage.removeItem(STORAGE_KEY);
}

async function request(method, path, body) {
  const key = getKey();
  const headers = { 'Content-Type': 'application/json' };
  if (key) {
    headers['X-Marionette-Key'] = key;
  }

  const opts = { method, headers };
  if (body && method !== 'GET') {
    opts.body = JSON.stringify(body);
  }

  const res = await fetch(path, opts);

  if (res.status === 401) {
    clearKey();
    window.dispatchEvent(new CustomEvent('auth:expired'));
    throw new Error('Unauthorized');
  }

  if (!res.ok) {
    const text = await res.text();
    let msg;
    try {
      msg = JSON.parse(text).error || text;
    } catch {
      msg = text;
    }
    throw new Error(msg || `HTTP ${res.status}`);
  }

  const ct = res.headers.get('content-type') || '';
  if (ct.includes('application/json')) {
    return res.json();
  }
  return res.text();
}

export const api = {
  get(path) {
    return request('GET', path);
  },
  post(path, body) {
    return request('POST', path, body);
  },
  put(path, body) {
    return request('PUT', path, body);
  },
  delete(path) {
    return request('DELETE', path);
  },
};

export function wsUrl(path) {
  const key = getKey();
  const proto = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
  const host = window.location.host;
  const endpoint = new URLSearchParams(window.location.search).get('endpoint') || 'local';
  return `${proto}//${host}${path}?endpoint=${encodeURIComponent(endpoint)}&key=${encodeURIComponent(key || '')}`;
}
