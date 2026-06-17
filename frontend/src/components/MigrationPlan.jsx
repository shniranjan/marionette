1|import { useState, useCallback } from 'react';
2|
3|const TRANSFER_METHODS = [
4|  { id: 'scp', label: 'SCP', desc: 'Secure copy over SSH. Good for single files/small volumes.', icon: '🔒' },
5|  { id: 'rsync-over-ssh', label: 'Rsync over SSH', desc: 'Delta-transfer over SSH. Best for large volumes with incremental changes.', icon: '🔄' },
6|  { id: 'pipe-direct', label: 'Pipe Direct', desc: 'Direct pipe via SSH (docker export | ssh docker import). No temp files.', icon: '⚡' },
7|  { id: 'export-s3', label: 'Export to S3', desc: 'Archive to S3 bucket, download on target. Good for cross-region.', icon: '☁️' },
8|];
9|
10|const COMPRESSION_LEVELS = [
11|  { id: 'pigz', label: 'pigz (parallel gzip)', speed: 'Fast', ratio: 'Medium', est: '~2:1' },
12|  { id: 'zstd', label: 'zstd', speed: 'Very Fast', ratio: 'High', est: '~3:1' },
13|  { id: 'lz4', label: 'lz4', speed: 'Extremely Fast', ratio: 'Low', est: '~1.5:1' },
14|  { id: 'none', label: 'No Compression', speed: 'N/A', ratio: 'None', est: '1:1' },
15|];
16|
17|export default function MigrationPlan({ plan = {}, volumes = [], onUpdate }) {
18|  const [transferMethod, setTransferMethod] = useState(plan.transferMethod || 'rsync-over-ssh');
19|  const [compression, setCompression] = useState(plan.compression || 'pigz');
20|  const [postOptions, setPostOptions] = useState({
21|    startOnTarget: plan.start_on_target !== false,
22|    verifyConnectivity: plan.verify_connectivity !== false,
23|    removeFromSource: false,
24|    rotateCredentials: false,
25|  });
26|  const [volumeOverrides, setVolumeOverrides] = useState({});
27|
28|  const handleTransferChange = useCallback((method) => {
29|    setTransferMethod(method);
30|    if (onUpdate) onUpdate({ transfer_method: method, compression, post_options: postOptions, volume_overrides: volumeOverrides });
31|  }, [compression, postOptions, volumeOverrides, onUpdate]);
32|
33|  const handleCompressionChange = useCallback((comp) => {
34|    setCompression(comp);
35|    if (onUpdate) onUpdate({ transfer_method: transferMethod, compression: comp, post_options: postOptions, volume_overrides: volumeOverrides });
36|  }, [transferMethod, postOptions, volumeOverrides, onUpdate]);
37|
38|  const handlePostOption = useCallback((key) => {
39|    const updated = { ...postOptions, [key]: !postOptions[key] };
40|    setPostOptions(updated);
41|    if (onUpdate) onUpdate({ transfer_method: transferMethod, compression, post_options: updated, volume_overrides: volumeOverrides });
42|  }, [transferMethod, compression, volumeOverrides, onUpdate]);
43|
44|  const handleVolumeOverride = useCallback((volName, field, value) => {
45|    const updated = { ...volumeOverrides, [volName]: { ...(volumeOverrides[volName] || {}), [field]: value } };
46|    setVolumeOverrides(updated);
47|    if (onUpdate) onUpdate({ transfer_method: transferMethod, compression, post_options: postOptions, volume_overrides: updated });
48|  }, [transferMethod, compression, postOptions, onUpdate]);
49|
50|  const selectedMethod = TRANSFER_METHODS.find(m => m.id === transferMethod);
51|  const selectedCompression = COMPRESSION_LEVELS.find(c => c.id === compression);
52|
53|  return (
54|    <div style={{ display: 'grid', gap: '20px' }}>
55|      {/* Transfer Method */}
56|      <div className="card">
57|        <h3>Transfer Method</h3>
58|        <div style={{ display: 'grid', gap: '10px' }}>
59|          {TRANSFER_METHODS.map((m) => (
60|            <label
61|              key={m.id}
62|              style={{
63|                display: 'flex',
64|                alignItems: 'flex-start',
65|                gap: '12px',
66|                padding: '12px',
67|                border: `2px solid ${transferMethod === m.id ? 'var(--accent)' : 'var(--border)'}`,
68|                borderRadius: '8px',
69|                cursor: 'pointer',
70|                background: transferMethod === m.id ? 'var(--bg-tertiary)' : 'transparent',
71|                transition: 'all 0.15s',
72|              }}
73|            >
74|              <input
75|                type="radio"
76|                name="transfer_method"
77|                value={m.id}
78|                checked={transferMethod === m.id}
79|                onChange={() => handleTransferChange(m.id)}
80|                style={{ marginTop: '2px', accentColor: 'var(--accent)' }}
81|              />
82|              <div style={{ flex: 1 }}>
83|                <div style={{ fontWeight: 600, marginBottom: '4px' }}>
84|                  {m.icon} {m.label}
85|                </div>
86|                <div style={{ fontSize: '0.8rem', color: 'var(--text-secondary)' }}>
87|                  {m.desc}
88|                </div>
89|              </div>
90|            </label>
91|          ))}
92|        </div>
93|      </div>
94|
95|      {/* Compression */}
96|      <div className="card">
97|        <h3>Compression</h3>
98|        <table>
99|          <thead>
100|            <tr>
101|              <th style={{ width: '40px' }}></th>
102|              <th>Algorithm</th>
103|              <th>Speed</th>
104|              <th>Ratio</th>
105|              <th>Est. Compression</th>
106|            </tr>
107|          </thead>
108|          <tbody>
109|            {COMPRESSION_LEVELS.map((c) => (
110|              <tr
111|                key={c.id}
112|                onClick={() => handleCompressionChange(c.id)}
113|                style={{
114|                  cursor: 'pointer',
115|                  background: compression === c.id ? 'var(--bg-tertiary)' : 'transparent',
116|                }}
117|              >
118|                <td>
119|                  <input
120|                    type="radio"
121|                    name="compression"
122|                    value={c.id}
123|                    checked={compression === c.id}
124|                    onChange={() => handleCompressionChange(c.id)}
125|                    style={{ accentColor: 'var(--accent)' }}
126|                  />
127|                </td>
128|                <td className="mono" style={{ fontWeight: compression === c.id ? 600 : 400 }}>
129|                  {c.label}
130|                </td>
131|                <td>{c.speed}</td>
132|                <td>{c.ratio}</td>
133|                <td className="mono">{c.est}</td>
134|              </tr>
135|            ))}
136|          </tbody>
137|        </table>
138|      </div>
139|
140|      {/* Post-Migration Options */}
141|      <div className="card">
142|        <h3>Post-Migration Actions</h3>
143|        <div style={{ display: 'grid', gap: '8px' }}>
144|          <label style={{ display: 'flex', alignItems: 'center', gap: '10px', cursor: 'pointer' }}>
145|            <input
146|              type="checkbox"
147|              checked={postOptions.startOnTarget}
148|              onChange={() => handlePostOption('startOnTarget')}
149|              style={{ accentColor: 'var(--accent)', width: '16px', height: '16px' }}
150|            />
151|            <span>Start container on target host</span>
152|          </label>
153|          <label style={{ display: 'flex', alignItems: 'center', gap: '10px', cursor: 'pointer' }}>
154|            <input
155|              type="checkbox"
156|              checked={postOptions.verifyConnectivity}
157|              onChange={() => handlePostOption('verifyConnectivity')}
158|              style={{ accentColor: 'var(--accent)', width: '16px', height: '16px' }}
159|            />
160|            <span>Verify connectivity after migration</span>
161|          </label>
162|          <label style={{ display: 'flex', alignItems: 'center', gap: '10px', cursor: 'pointer' }}>
163|            <input
164|              type="checkbox"
165|              checked={postOptions.removeFromSource}
166|              onChange={() => handlePostOption('removeFromSource')}
167|              style={{ accentColor: 'var(--accent)', width: '16px', height: '16px' }}
168|            />
169|            <span style={{ color: postOptions.removeFromSource ? 'var(--red)' : 'inherit' }}>
170|              ⚠ Remove container from source host
171|            </span>
172|          </label>
173|          <label style={{ display: 'flex', alignItems: 'center', gap: '10px', cursor: 'pointer' }}>
174|            <input
175|              type="checkbox"
176|              checked={postOptions.rotateCredentials}
177|              onChange={() => handlePostOption('rotateCredentials')}
178|              style={{ accentColor: 'var(--accent)', width: '16px', height: '16px' }}
179|            />
180|            <span>Rotate credentials after migration</span>
181|          </label>
182|        </div>
183|      </div>
184|
185|      {/* Volume Overrides */}
186|      {volumes.length > 0 && (
187|        <div className="card">
188|          <h3>Per-Volume Transfer Overrides</h3>
189|          <div style={{ fontSize: '0.8rem', color: 'var(--text-secondary)', marginBottom: '12px' }}>
190|            Customize transfer method or target path per volume
191|          </div>
192|          <table>
193|            <thead>
194|              <tr>
195|                <th>Volume</th>
196|                <th>Size</th>
197|                <th>Transfer Method</th>
198|                <th>Custom Path</th>
199|              </tr>
200|            </thead>
201|            <tbody>
202|              {volumes.map((v) => {
203|                const override = volumeOverrides[v.name] || {};
204|                return (
205|                  <tr key={v.name}>
206|                    <td className="mono" style={{ fontWeight: 500 }}>{v.name}</td>
207|                    <td className="mono" style={{ fontSize: '0.8rem' }}>
208|                      {v.sizeBytes ? `${(v.sizeBytes / 1073741824).toFixed(1)} GB` : '—'}
209|                    </td>
210|                    <td>
211|                      <select
212|                        value={override.transfer_method || v.transferMethod || transferMethod}
213|                        onChange={(e) => handleVolumeOverride(v.name, 'transfer_method', e.target.value)}
214|                        style={{ fontSize: '0.75rem', padding: '4px 8px' }}
215|                      >
216|                        <option value="inherit">Inherit ({v.transferMethod || transferMethod})</option>
217|                        {TRANSFER_METHODS.map(m => (
218|                          <option key={m.id} value={m.id}>{m.label}</option>
219|                        ))}
220|                      </select>
221|                    </td>
222|                    <td>
223|                      <input
224|                        type="text"
225|                        value={override.custom_path || ''}
226|                        onChange={(e) => handleVolumeOverride(v.name, 'custom_path', e.target.value)}
227|                        placeholder="Default path"
228|                        style={{ fontSize: '0.75rem', padding: '4px 8px', width: '180px' }}
229|                      />
230|                    </td>
231|                  </tr>
232|                );
233|              })}
234|            </tbody>
235|          </table>
236|        </div>
237|      )}
238|
239|      {/* Summary */}
240|      <div className="card" style={{ borderLeft: '3px solid var(--accent)' }}>
241|        <h3>Migration Summary</h3>
242|        <div style={{ display: 'grid', gap: '6px', fontSize: '0.85rem' }}>
243|          <div>
244|            <span style={{ color: 'var(--text-secondary)' }}>Method: </span>
245|            <span className="mono">{selectedMethod?.label || transferMethod}</span>
246|          </div>
247|          <div>
248|            <span style={{ color: 'var(--text-secondary)' }}>Compression: </span>
249|            <span className="mono">{selectedCompression?.label || compression}</span>
250|          </div>
251|          <div>
252|            <span style={{ color: 'var(--text-secondary)' }}>Start on target: </span>
253|            <span>{postOptions.startOnTarget ? 'Yes ✓' : 'No'}</span>
254|          </div>
255|          <div>
256|            <span style={{ color: 'var(--text-secondary)' }}>Verify connectivity: </span>
257|            <span>{postOptions.verifyConnectivity ? 'Yes ✓' : 'No'}</span>
258|          </div>
259|          {plan.estimatedSizeBytes > 0 && (
260|            <div>
261|              <span style={{ color: 'var(--text-secondary)' }}>Estimated size: </span>
262|              <span className="mono">
263|                {(plan.estimatedSizeBytes / 1073741824).toFixed(1)} GB
264|                {plan.compressed ? ' (compressed)' : ''}
265|              </span>
266|            </div>
267|          )}
268|          {volumes.length > 0 && (
269|            <div>
270|              <span style={{ color: 'var(--text-secondary)' }}>Volumes: </span>
271|              <span>{volumes.length} volume{volumes.length > 1 ? 's' : ''}</span>
272|            </div>
273|          )}
274|        </div>
275|      </div>
276|    </div>
277|  );
278|}
279|