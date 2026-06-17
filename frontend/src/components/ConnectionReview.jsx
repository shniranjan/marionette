1|import { useState, useCallback } from 'react';
2|
3|export default function ConnectionReview({ connections = [], onUpdate, blocked }) {
4|  const [revealed, setRevealed] = useState({});
5|  const [revealConfirm, setRevealConfirm] = useState(null);
6|  const [auditEntries, setAuditEntries] = useState([]);
7|
8|  const handleReveal = useCallback((varName) => {
9|    if (revealConfirm !== varName) {
10|      setRevealConfirm(varName);
11|      return;
12|    }
13|    setRevealed(prev => ({ ...prev, [varName]: true }));
14|    setRevealConfirm(null);
15|    // Audit log
16|    const entry = {
17|      timestamp: new Date().toISOString(),
18|      action: 'reveal_connection',
19|      variable: varName,
20|    };
21|    setAuditEntries(prev => [...prev, entry]);
22|    const audit = JSON.parse(localStorage.getItem('marionette-audit-log') || '[]');
23|    audit.push(entry);
24|    localStorage.setItem('marionette-audit-log', JSON.stringify(audit));
25|  }, [revealConfirm]);
26|
27|  const handleAction = useCallback((conn, action) => {
28|    if (onUpdate) onUpdate(conn.varName, action);
29|  }, [onUpdate]);
30|
31|  const criticalCount = connections.filter(c => c.willBreak && !c.resolved).length;
32|
33|  return (
34|    <div>
35|      {blocked && criticalCount > 0 && (
36|        <div style={{
37|          padding: '12px 16px',
38|          background: 'var(--red-dim)',
39|          border: '1px solid var(--red)',
40|          borderRadius: '8px',
41|          marginBottom: '16px',
42|          color: '#fff',
43|          fontSize: '0.85rem',
44|        }}>
45|          ⚠ {criticalCount} critical connection{criticalCount > 1 ? 's' : ''} must be resolved before proceeding.
46|        </div>
47|      )}
48|
49|      {auditEntries.length > 0 && (
50|        <div style={{
51|          padding: '8px 12px',
52|          background: 'var(--bg-tertiary)',
53|          border: '1px solid var(--yellow)',
54|          borderRadius: '6px',
55|          marginBottom: '12px',
56|          fontSize: '0.75rem',
57|          color: 'var(--yellow)',
58|        }}>
59|          🔍 Audit: {auditEntries.length} credential{auditEntries.length > 1 ? 's' : ''} revealed this session
60|        </div>
61|      )}
62|
63|      <table>
64|        <thead>
65|          <tr>
66|            <th>Env Variable</th>
67|            <th>Value</th>
68|            <th>Target Container</th>
69|            <th>Same Host?</th>
70|            <th>Will Break?</th>
71|            <th>Action</th>
72|          </tr>
73|        </thead>
74|        <tbody>
75|          {connections.length === 0 ? (
76|            <tr>
77|              <td colSpan={6} style={{ textAlign: 'center', color: 'var(--text-secondary)', padding: '24px' }}>
78|                No database connections detected
79|              </td>
80|            </tr>
81|          ) : (
82|            connections.map((conn, i) => {
83|              const isRevealed = revealed[conn.varName];
84|              const isConfirming = revealConfirm === conn.varName;
85|              const resolved = conn.resolved || false;
86|              return (
87|                <tr key={conn.varName || i} style={{ opacity: resolved ? 0.5 : 1 }}>
88|                  <td className="mono" style={{ fontWeight: 500 }}>{conn.varName || '—'}</td>
89|                  <td className="mono" style={{ fontSize: '0.8rem' }}>
90|                    {isRevealed ? (conn.valueMasked || conn.value || '—') : (
91|                      <span style={{ letterSpacing: '2px' }}>••••••••</span>
92|                    )}
93|                    <button
94|                      className="btn-sm"
95|                      style={{
96|                        marginLeft: '8px',
97|                        fontSize: '0.65rem',
98|                        padding: '1px 6px',
99|                        ...(isConfirming ? { background: 'var(--yellow-dim)', borderColor: 'var(--yellow)', color: '#fff' } : {}),
100|                      }}
101|                      onClick={() => handleReveal(conn.varName)}
102|                    >
103|                      {isRevealed ? 'Hide' : isConfirming ? '⚠ Confirm' : '👁'}
104|                    </button>
105|                    {isConfirming && (
106|                      <div style={{ color: 'var(--yellow)', fontSize: '0.7rem', marginTop: '2px' }}>
107|                        Click again to reveal (audited)
108|                      </div>
109|                    )}
110|                  </td>
111|                  <td className="mono" style={{ fontSize: '0.8rem' }}>
112|                    {conn.targetContainer || '—'}
113|                  </td>
114|                  <td>
115|                    <span style={{
116|                      color: conn.onSameHost ? 'var(--green)' : 'var(--yellow)',
117|                      fontWeight: 500,
118|                      fontSize: '0.8rem',
119|                    }}>
120|                      {conn.onSameHost ? 'Yes ✓' : 'No ✗'}
121|                    </span>
122|                  </td>
123|                  <td>
124|                    <span style={{
125|                      color: conn.willBreak ? 'var(--red)' : 'var(--green)',
126|                      fontWeight: 600,
127|                      fontSize: '0.8rem',
128|                    }}>
129|                      {conn.willBreak ? '⚠ YES' : '✓ No'}
130|                    </span>
131|                  </td>
132|                  <td>
133|                    {conn.willBreak && !resolved ? (
134|                      <div style={{ display: 'flex', flexDirection: 'column', gap: '4px' }}>
135|                        {conn.fixSuggestion && (
136|                          <div style={{ fontSize: '0.7rem', color: 'var(--text-secondary)', marginBottom: '4px' }}>
137|                            {conn.fixSuggestion}
138|                          </div>
139|                        )}
140|                        <div className="btn-group">
141|                          <button
142|                            className="btn-sm"
143|                            style={{ fontSize: '0.7rem' }}
144|                            onClick={() => handleAction(conn, 'migrate_together')}
145|                          >
146|                            Migrate Together
147|                          </button>
148|                          <button
149|                            className="btn-sm"
150|                            style={{ fontSize: '0.7rem' }}
151|                            onClick={() => handleAction(conn, 'update_string')}
152|                          >
153|                            Update String
154|                          </button>
155|                          <button
156|                            className="btn-sm"
157|                            style={{ fontSize: '0.7rem' }}
158|                            onClick={() => handleAction(conn, 'skip')}
159|                          >
160|                            Skip
161|                          </button>
162|                        </div>
163|                      </div>
164|                    ) : resolved ? (
165|                      <span style={{ color: 'var(--green)', fontSize: '0.8rem' }}>
166|                        {conn.resolution || 'Resolved ✓'}
167|                      </span>
168|                    ) : (
169|                      <span style={{ color: 'var(--text-secondary)', fontSize: '0.8rem' }}>—</span>
170|                    )}
171|                  </td>
172|                </tr>
173|              );
174|            })
175|          )}
176|        </tbody>
177|      </table>
178|    </div>
179|  );
180|}
181|