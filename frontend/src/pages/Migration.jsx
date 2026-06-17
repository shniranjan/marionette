1|1|1|import { useState, useCallback, useEffect } from 'react';
2|2|2|import { api } from '../api/client';
3|3|3|import Modal from '../components/Modal';
4|4|4|import Spinner from '../components/Spinner';
5|5|5|import { useToast } from '../components/Toast';
6|6|6|import ConnectionReview from '../components/ConnectionReview';
7|7|7|import MigrationPlan from '../components/MigrationPlan';
8|8|8|import VolumeInspector from '../components/VolumeInspector';
9|9|9|
10|10|10|const TOTAL_STEPS = 9;
11|11|11|
12|12|12|const STEP_LABELS = [
13|13|13|  'Select Source',
14|14|14|  'Analyze',
15|15|15|  'Strategy',
16|16|16|  'Credentials',
17|17|17|  'Connection Fixes',
18|18|18|  'Target',
19|19|19|  'Dry Run',
20|20|20|  'Execute',
21|21|21|  'Verify',
22|22|22|];
23|23|23|
24|24|24|function StepIndicator({ currentStep, completedSteps = new Set() }) {
25|25|25|  return (
26|26|26|    <div style={{
27|27|27|      display: 'flex',
28|28|28|      alignItems: 'center',
29|29|29|      gap: '0',
30|30|30|      marginBottom: '24px',
31|31|31|      padding: '16px 0',
32|32|32|      overflow: 'hidden',
33|33|33|      flexWrap: 'nowrap',
34|34|34|    }}>
35|35|35|      {STEP_LABELS.map((label, i) => {
36|36|36|        const step = i + 1;
37|37|37|        const isComplete = completedSteps.has(step);
38|38|38|        const isCurrent = currentStep === step;
39|39|39|        const isPending = step > currentStep && !isComplete;
40|40|40|
41|41|41|        return (
42|42|42|          <div key={step} style={{ display: 'flex', alignItems: 'center', flex: i === 0 ? '0 0 auto' : '1 1 0', minWidth: 0 }}>
43|43|43|            {i > 0 && (
44|44|44|              <div style={{
45|45|45|                flex: 1,
46|46|46|                height: '2px',
47|47|47|                background: isComplete ? 'var(--green)' : 'var(--border)',
48|48|48|                margin: '0 4px',
49|49|49|                minWidth: '12px',
50|50|50|              }} />
51|51|51|            )}
52|52|52|            <div style={{
53|53|53|              display: 'flex',
54|54|54|              flexDirection: 'column',
55|55|55|              alignItems: 'center',
56|56|56|              gap: '4px',
57|57|57|              flexShrink: 0,
58|58|58|            }}>
59|59|59|              <div style={{
60|60|60|                width: '28px',
61|61|61|                height: '28px',
62|62|62|                borderRadius: '50%',
63|63|63|                display: 'flex',
64|64|64|                alignItems: 'center',
65|65|65|                justifyContent: 'center',
66|66|66|                fontSize: '0.75rem',
67|67|67|                fontWeight: 700,
68|68|68|                background: isComplete ? 'var(--green-dim)' :
69|69|69|                            isCurrent ? 'var(--accent-dim)' : 'var(--bg-tertiary)',
70|70|70|                color: isComplete ? 'var(--green)' :
71|71|71|                       isCurrent ? 'var(--accent)' :
72|72|72|                       'var(--text-secondary)',
73|73|73|                border: `2px solid ${isComplete ? 'var(--green)' :
74|74|74|                                     isCurrent ? 'var(--accent)' : 'var(--border)'}`,
75|75|75|                transition: 'all 0.3s',
76|76|76|              }}>
77|77|77|                {isComplete ? '✓' :
78|78|78|                 isCurrent ? '▸' :
79|79|79|                 step}
80|80|80|              </div>
81|81|81|              <span style={{
82|82|82|                fontSize: '0.6rem',
83|83|83|                color: isCurrent ? 'var(--accent)' :
84|84|84|                       isComplete ? 'var(--text-primary)' : 'var(--text-secondary)',
85|85|85|                fontWeight: isCurrent ? 600 : 400,
86|86|86|                textAlign: 'center',
87|87|87|                maxWidth: '60px',
88|88|88|                overflow: 'hidden',
89|89|89|                textOverflow: 'ellipsis',
90|90|90|                whiteSpace: 'nowrap',
91|91|91|              }}>
92|92|92|                {label}
93|93|93|              </span>
94|94|94|            </div>
95|95|95|          </div>
96|96|96|        );
97|97|97|      })}
98|98|98|    </div>
99|99|99|  );
100|100|100|}
101|101|101|
102|102|102|export default function Migration({ navigate }) {
103|103|103|  const toast = useToast();
104|104|104|
105|105|105|  // Wizard state
106|106|106|  const [step, setStep] = useState(1);
107|107|107|  const [completedSteps, setCompletedSteps] = useState(new Set());
108|108|108|  const [loading, setLoading] = useState(false);
109|109|109|  const [error, setError] = useState(null);
110|110|110|
111|111|111|  // Step 1 — Source
112|112|112|  const [endpoints, setEndpoints] = useState([]);
113|113|113|  const [sourceEndpoint, setSourceEndpoint] = useState('');
114|114|114|  const [containers, setContainers] = useState([]);
115|115|115|  const [containerSearch, setContainerSearch] = useState('');
116|116|116|  const [selectedContainer, setSelectedContainer] = useState(null);
117|117|117|
118|118|118|  // Step 2 — Analysis results
119|119|119|  const [analysis, setAnalysis] = useState(null);
120|120|120|
121|121|121|  // Step 3 — Strategy
122|122|122|  const [strategy, setStrategy] = useState({});
123|123|123|  const [transferMethod, setTransferMethod] = useState('rsync-over-ssh');
124|124|124|  const [compression, setCompression] = useState('pigz');
125|125|125|  const [postOptions, setPostOptions] = useState({});
126|126|126|
127|127|127|  // Step 4 — Credentials
128|128|128|  const [credentialsRevealed, setCredentialsRevealed] = useState({});
129|129|129|  const [credRevealConfirm, setCredRevealConfirm] = useState(null);
130|130|130|
131|131|131|  // Step 5 — Connection fixes
132|132|132|  const [connectionResolutions, setConnectionResolutions] = useState({});
133|133|133|
134|134|134|  // Step 6 — Target
135|135|135|  const [targetEndpoints, setTargetEndpoints] = useState([]);
136|136|136|  const [targetEndpoint, setTargetEndpoint] = useState('');
137|137|137|  const [targetStackName, setTargetStackName] = useState('');
138|138|138|  const [targetInfo, setTargetInfo] = useState(null);
139|139|139|
140|140|140|  // Step 7 — Dry run
141|141|141|  const [dryRunResult, setDryRunResult] = useState(null);
142|142|142|
143|143|143|  // Step 8 — Execute
144|144|144|  const [execPhase, setExecPhase] = useState('pending'); // pending, running, paused, done
145|145|145|  const [execCommands, setExecCommands] = useState([]);
146|146|146|  const [currentCommandGroup, setCurrentCommandGroup] = useState(0);
147|147|147|  const [execResults, setExecResults] = useState({});
148|148|148|  const [execStartTime, setExecStartTime] = useState(null);
149|149|149|
150|150|150|  // Step 9 — Verification
151|151|151|  const [verification, setVerification] = useState(null);
152|152|152|
153|153|153|  // Shared state
154|154|154|  const [migrationPlan, setMigrationPlan] = useState(null);
155|155|155|  const [migrationId, setMigrationId] = useState(null);
156|156|156|  const [revealAudit, setRevealAudit] = useState([]);
157|157|157|
158|158|158|  // Volume inspector
159|159|159|  const [inspectVolume, setInspectVolume] = useState(null);
160|160|160|
161|161|161|  const auditLog = useCallback((action, detail) => {
162|162|162|    const entry = { timestamp: new Date().toISOString(), action, detail };
163|163|163|    const audit = JSON.parse(localStorage.getItem('marionette-audit-log') || '[]');
164|164|164|    audit.push(entry);
165|165|165|    localStorage.setItem('marionette-audit-log', JSON.stringify(audit));
166|166|166|    setRevealAudit(prev => [...prev, entry]);
167|167|167|  }, []);
168|168|168|
169|169|169|  // Load endpoints on mount
170|170|170|  useEffect(() => {
171|171|171|    (async () => {
172|172|172|      try {
173|173|173|        const data = await api.get('/api/endpoints');
174|174|174|        const eps = Array.isArray(data) ? data : (data?.endpoints || []);
175|175|175|        setEndpoints(eps);
176|176|176|        setTargetEndpoints(eps);
177|177|177|      } catch {/* ignore */}
178|178|178|    })();
179|179|179|  }, []);
180|180|180|
181|181|181|  const filteredContainers = containers.filter(c => {
182|182|182|    const name = (c.Name || c.name || '').toLowerCase();
183|183|183|    const img = (c.Image || c.image || '').toLowerCase();
184|184|184|    const q = containerSearch.toLowerCase();
185|185|185|    return !q || name.includes(q) || img.includes(q);
186|186|186|  });
187|187|187|
188|188|188|  // === STEP 1: Select Source ===
189|189|189|  const handleSourceSelect = async (epId) => {
190|190|190|    setSourceEndpoint(epId);
191|191|191|    setSelectedContainer(null);
192|192|192|    setContainers([]);
193|193|193|    setLoading(true);
194|194|194|    setError(null);
195|195|195|    try {
196|196|196|      const data = await api.get(`/api/endpoints/${epId}/containers`);
197|197|197|      setContainers(Array.isArray(data) ? data : (data?.containers || []));
198|198|198|    } catch (err) {
199|199|199|      setError('Failed to load containers: ' + err.message);
200|200|200|    } finally {
201|201|201|      setLoading(false);
202|202|202|    }
203|203|203|  };
204|204|204|
205|205|205|  const handleContainerSelect = (container) => {
206|206|206|    setSelectedContainer(container);
207|207|207|  };
208|208|208|
209|209|209|  const handleAnalyze = async () => {
210|210|210|    if (!sourceEndpoint || !selectedContainer) return;
211|211|211|    setLoading(true);
212|212|212|    setError(null);
213|213|213|    try {
214|214|214|      const result = await api.post('/migration/analyze', {
215|215|215|        source_endpoint: sourceEndpoint,
216|216|216|        container_id: selectedContainer.Id || selectedContainer.id,
217|217|217|      });
218|218|218|      setAnalysis(result);
219|219|219|      setCompletedSteps(prev => new Set([...prev, 1]));
220|220|220|      setStep(2);
221|221|221|    } catch (err) {
222|222|222|      toast('Analysis failed: ' + err.message, 'error');
223|223|223|    } finally {
224|224|224|      setLoading(false);
225|225|225|    }
226|226|226|  };
227|227|227|
228|228|228|  // === STEP 3: Strategy ===
229|229|229|  const handleStrategyUpdate = useCallback((s) => {
230|230|230|    setStrategy(s);
231|231|231|    if (s.transfer_method) setTransferMethod(s.transfer_method);
232|232|232|    if (s.compression) setCompression(s.compression);
233|233|233|    if (s.post_options) setPostOptions(s.post_options);
234|234|234|  }, []);
235|235|235|
236|236|236|  const handleProceedFromStrategy = () => {
237|237|237|    setCompletedSteps(prev => new Set([...prev, 3]));
238|238|238|    setStep(4);
239|239|239|  };
240|240|240|
241|241|241|  // === STEP 4: Credentials ===
242|242|242|  const handleRevealCredential = (varName) => {
243|243|243|    if (credRevealConfirm !== varName) {
244|244|244|      setCredRevealConfirm(varName);
245|245|245|      return;
246|246|246|    }
247|247|247|    setCredentialsRevealed(prev => ({ ...prev, [varName]: true }));
248|248|248|    setCredRevealConfirm(null);
249|249|249|    auditLog('reveal_credential', { variable: varName });
250|250|250|  };
251|251|251|
252|252|252|  const handleCredentialsDone = () => {
253|253|253|    setCompletedSteps(prev => new Set([...prev, 4]));
254|254|254|    setStep(5);
255|255|255|  };
256|256|256|
257|257|257|  // === STEP 5: Connection Fixes ===
258|258|258|  const handleConnectionUpdate = useCallback((varName, action) => {
259|259|259|    setConnectionResolutions(prev => ({
260|260|260|      ...prev,
261|261|261|      [varName]: { action, resolved: true },
262|262|262|    }));
263|263|263|  }, []);
264|264|264|
265|265|265|  const allCriticalResolved = () => {
266|266|266|    const conns = analysis?.dbConnections || [];
267|267|267|    return conns.filter(c => c.willBreak).every(c => connectionResolutions[c.varName]?.resolved);
268|268|268|  };
269|269|269|
270|270|270|  const handleConnectionFixesDone = () => {
271|271|271|    if (!allCriticalResolved()) {
272|272|272|      toast('Resolve all critical connections first', 'error');
273|273|273|      return;
274|274|274|    }
275|275|275|    setCompletedSteps(prev => new Set([...prev, 5]));
276|276|276|    setStep(6);
277|277|277|  };
278|278|278|
279|279|279|  // === STEP 6: Target ===
280|280|280|  const handleTargetSelect = async (epId) => {
281|281|281|    setTargetEndpoint(epId);
282|282|282|    setTargetInfo(null);
283|283|283|    if (!epId) return;
284|284|284|    try {
285|285|285|      const data = await api.get(`/api/endpoints/${epId}`);
286|286|286|      setTargetInfo(data);
287|287|287|    } catch {/* ignore */}
288|288|288|  };
289|289|289|
290|290|290|  const handleTargetNext = async () => {
291|291|291|    if (!targetEndpoint) {
292|292|292|      toast('Select a target endpoint', 'error');
293|293|293|      return;
294|294|294|    }
295|295|295|    setCompletedSteps(prev => new Set([...prev, 6]));
296|296|296|    setStep(7);
297|297|297|  };
298|298|298|
299|299|299|  // === STEP 7: Dry Run ===
300|300|300|  const handleDryRun = async () => {
301|301|301|    setLoading(true);
302|302|302|    setError(null);
303|303|303|    try {
304|304|304|      const result = await api.post('/migration/dry-run', {
305|305|305|        source_endpoint: sourceEndpoint,
306|306|306|        target_endpoint: targetEndpoint,
307|307|307|        container_id: selectedContainer?.Id || selectedContainer?.id,
308|308|308|        transfer_method: transferMethod,
309|309|309|        compression,
310|310|310|        post_options: postOptions,
311|311|311|        connection_resolutions: connectionResolutions,
312|312|312|        target_stack_name: targetStackName || undefined,
313|313|313|      });
314|314|314|      setDryRunResult(result);
315|315|315|      setMigrationPlan(result);
316|316|316|      setMigrationId(result.migrationId);
317|317|317|    } catch (err) {
318|318|318|      toast('Dry run failed: ' + err.message, 'error');
319|319|319|    } finally {
320|320|320|      setLoading(false);
321|321|321|    }
322|322|322|  };
323|323|323|
324|324|324|  const handleProceedToExecute = () => {
325|325|325|    setCompletedSteps(prev => new Set([...prev, 7]));
326|326|326|    setStep(8);
327|327|327|    setExecPhase('pending');
328|328|328|    setCurrentCommandGroup(0);
329|329|329|    setExecResults({});
330|330|330|    setExecCommands(dryRunResult?.commands || []);
331|331|331|    setExecStartTime(Date.now());
332|332|332|  };
333|333|333|
334|334|334|  // === STEP 8: Execute ===
335|335|335|  const handleStartExecution = () => {
336|336|336|    setExecPhase('running');
337|337|337|    setCurrentCommandGroup(0);
338|338|338|    setExecStartTime(Date.now());
339|339|339|  };
340|340|340|
341|341|341|  const handleCommandGroupDone = async () => {
342|342|342|    const now = Date.now();
343|343|343|    setExecResults(prev => ({
344|344|344|      ...prev,
345|345|345|      [currentCommandGroup]: { time: now - execStartTime, status: 'done' },
346|346|346|    }));
347|347|347|
348|348|348|    if (currentCommandGroup + 1 >= execCommands.length) {
349|349|349|      setExecPhase('done');
350|350|350|      try {
351|351|351|        // Fetch migration result
352|352|352|        if (migrationId) {
353|353|353|          const result = await api.get(`/api/migration/${migrationId}`);
354|354|354|          setVerification(result);
355|355|355|        }
356|356|356|      } catch {/* ignore */}
357|357|357|      setCompletedSteps(prev => new Set([...prev, 8]));
358|358|358|      setStep(9);
359|359|359|    } else {
360|360|360|      setCurrentCommandGroup(prev => prev + 1);
361|361|361|    }
362|362|362|  };
363|363|363|
364|364|364|  const handleCancelExecution = () => {
365|365|365|    setExecPhase('pending');
366|366|366|    toast('Execution cancelled at safe point', 'info');
367|367|367|  };
368|368|368|
369|369|369|  // === STEP 9: Verify ===
370|370|370|  const handlePostMigration = async (action) => {
371|371|371|    try {
372|372|372|      if (action === 'remove_source') {
373|373|373|        await api.post(`/api/endpoints/${sourceEndpoint}/containers/${selectedContainer?.Id || selectedContainer?.id}/remove`);
374|374|374|        toast('Container removed from source', 'success');
375|375|375|      } else if (action === 'rollback') {
376|376|376|        await api.post(`/api/migration/${migrationId}/rollback`);
377|377|377|        toast('Rollback initiated', 'info');
378|378|378|      }
379|379|379|    } catch (err) {
380|380|380|      toast('Action failed: ' + err.message, 'error');
381|381|381|    }
382|382|382|  };
383|383|383|
384|384|384|  const handleDone = () => {
385|385|385|    navigate('dashboard');
386|386|386|    toast('Migration wizard complete 🎉', 'success');
387|387|387|  };
388|388|388|
389|389|389|  // Navigation helpers
390|390|390|  const canGoNext = () => {
391|391|391|    switch (step) {
392|392|392|      case 1: return selectedContainer !== null;
393|393|393|      case 2: return analysis !== null;
394|394|394|      case 3: return true;
395|395|395|      case 4: return true;
396|396|396|      case 5: return allCriticalResolved();
397|397|397|      case 6: return targetEndpoint !== '';
398|398|398|      case 7: return dryRunResult !== null;
399|399|399|      case 8: return execPhase === 'done';
400|400|400|      case 9: return true;
401|401|401|      default: return false;
402|402|402|    }
403|403|403|  };
404|404|404|
405|405|405|  const handleNext = () => {
406|406|406|    switch (step) {
407|407|407|      case 1: handleAnalyze(); break;
408|408|408|      case 2: setCompletedSteps(prev => new Set([...prev, 2])); setStep(3); break;
409|409|409|      case 3: handleProceedFromStrategy(); break;
410|410|410|      case 4: handleCredentialsDone(); break;
411|411|411|      case 5: handleConnectionFixesDone(); break;
412|412|412|      case 6: handleTargetNext(); break;
413|413|413|      case 7: handleProceedToExecute(); break;
414|414|414|      case 8: break; // Manual
415|415|415|      case 9: handleDone(); break;
416|416|416|    }
417|417|417|  };
418|418|418|
419|419|419|  const handlePrev = () => {
420|420|420|    if (step > 1) setStep(step - 1);
421|421|421|  };
422|422|422|
423|423|423|  // === RENDER ===
424|424|424|
425|425|425|  const renderStepContent = () => {
426|426|426|    switch (step) {
427|427|427|      // ── STEP 1: Select Source ──
428|428|428|      case 1:
429|429|429|        return (
430|430|430|          <div style={{ display: 'grid', gap: '16px' }}>
431|431|431|            <div className="card">
432|432|432|              <h3>Source Endpoint</h3>
433|433|433|              <select
434|434|434|                value={sourceEndpoint}
435|435|435|                onChange={e => handleSourceSelect(e.target.value)}
436|436|436|                style={{ width: '100%', marginTop: '8px' }}
437|437|437|              >
438|438|438|                <option value="">— Select source endpoint —</option>
439|439|439|                <option value="local">Local</option>
440|440|440|                {endpoints.map(ep => (
441|441|441|                  <option key={ep.id || ep.Id} value={ep.id || ep.Id}>
442|442|442|                    {ep.name || ep.Name}
443|443|443|                  </option>
444|444|444|                ))}
445|445|445|              </select>
446|446|446|            </div>
447|447|447|
448|448|448|            {sourceEndpoint && (
449|449|449|              <div className="card">
450|450|450|                <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '12px' }}>
451|451|451|                  <h3 style={{ margin: 0 }}>Containers</h3>
452|452|452|                  <input
453|453|453|                    type="text"
454|454|454|                    placeholder="Search containers..."
455|455|455|                    value={containerSearch}
456|456|456|                    onChange={e => setContainerSearch(e.target.value)}
457|457|457|                    style={{ width: '240px' }}
458|458|458|                  />
459|459|459|                </div>
460|460|460|                {loading ? (
461|461|461|                  <div className="loading-center"><div className="spinner" /></div>
462|462|462|                ) : containers.length === 0 ? (
463|463|463|                  <div className="text-secondary" style={{ padding: '24px', textAlign: 'center' }}>
464|464|464|                    No containers found on this endpoint
465|465|465|                  </div>
466|466|466|                ) : (
467|467|467|                  <div style={{ maxHeight: '400px', overflow: 'auto' }}>
468|468|468|                    <table>
469|469|469|                      <thead>
470|470|470|                        <tr>
471|471|471|                          <th style={{ width: '30px' }}></th>
472|472|472|                          <th>Name</th>
473|473|473|                          <th>Image</th>
474|474|474|                          <th>State</th>
475|475|475|                          <th>Stack</th>
476|476|476|                        </tr>
477|477|477|                      </thead>
478|478|478|                      <tbody>
479|479|479|                        {filteredContainers.map(c => {
480|480|480|                          const name = (c.Name || c.name || '').replace(/^\//, '');
481|481|481|                          const isCompose = (c.Labels || c.labels || {})['com.docker.compose.project'];
482|482|482|                          const selected = selectedContainer && (selectedContainer.Id || selectedContainer.id) === (c.Id || c.id);
483|483|483|                          return (
484|484|484|                            <tr
485|485|485|                              key={c.Id || c.id}
486|486|486|                              onClick={() => handleContainerSelect(c)}
487|487|487|                              style={{
488|488|488|                                cursor: 'pointer',
489|489|489|                                background: selected ? 'var(--bg-tertiary)' : 'transparent',
490|490|490|                                borderLeft: selected ? '3px solid var(--accent)' : '3px solid transparent',
491|491|491|                              }}
492|492|492|                            >
493|493|493|                              <td>
494|494|494|                                <input
495|495|495|                                  type="radio"
496|496|496|                                  checked={!!selected}
497|497|497|                                  onChange={() => handleContainerSelect(c)}
498|498|498|                                  style={{ accentColor: 'var(--accent)' }}
499|499|499|                                />
500|500|500|                              </td>
501|501|501|                              <td className="mono" style={{ fontWeight: 500, fontSize: '0.85rem' }}>{name}</td>
502|502|502|                              <td className="mono" style={{ fontSize: '0.8rem' }}>{c.Image || c.image || '—'}</td>
503|503|503|                              <td>
504|504|504|                                <span style={{
505|505|505|                                  display: 'inline-block',
506|506|506|                                  padding: '2px 10px',
507|507|507|                                  borderRadius: '12px',
508|508|508|                                  fontSize: '0.7rem',
509|509|509|                                  fontWeight: 600,
510|510|510|                                  color: (c.State || c.state || '').toLowerCase() === 'running' ? 'var(--green)' : 'var(--red)',
511|511|511|                                  background: (c.State || c.state || '').toLowerCase() === 'running' ? 'var(--green-dim)' : 'var(--red-dim)',
512|512|512|                                }}>
513|513|513|                                  {c.State || c.state || 'unknown'}
514|514|514|                                </span>
515|515|515|                              </td>
516|516|516|                              <td>
517|517|517|                                {isCompose ? (
518|518|518|                                  <span style={{
519|519|519|                                    display: 'inline-block',
520|520|520|                                    padding: '1px 8px',
521|521|521|                                    borderRadius: '10px',
522|522|522|                                    background: 'var(--bg-tertiary)',
523|523|523|                                    fontSize: '0.7rem',
524|524|524|                                    color: 'var(--accent)',
525|525|525|                                  }}>
526|526|526|                                    📚 {isCompose}
527|527|527|                                  </span>
528|528|528|                                ) : <span className="text-secondary">—</span>}
529|529|529|                              </td>
530|530|530|                            </tr>
531|531|531|                          );
532|532|532|                        })}
533|533|533|                      </tbody>
534|534|534|                    </table>
535|535|535|                  </div>
536|536|536|                )}
537|537|537|                {selectedContainer && (
538|538|538|                  <div style={{ marginTop: '12px', padding: '12px', background: 'var(--bg-tertiary)', borderRadius: '8px' }}>
539|539|539|                    <div style={{ fontWeight: 600, marginBottom: '4px' }}>
540|540|540|                      Selected: {(selectedContainer.Name || selectedContainer.name || '').replace(/^\//, '')}
541|541|541|                    </div>
542|542|542|                    <div className="mono" style={{ fontSize: '0.75rem', color: 'var(--text-secondary)' }}>
543|543|543|                      Image: {selectedContainer.Image || selectedContainer.image}
544|544|544|                    </div>
545|545|545|                    {(() => {
546|546|546|                      const labels = selectedContainer.Labels || selectedContainer.labels || {};
547|547|547|                      const project = labels['com.docker.compose.project'];
548|548|548|                      if (project) {
549|549|549|                        return (
550|550|550|                          <div style={{ marginTop: '8px', padding: '8px', background: 'var(--bg-secondary)', borderRadius: '4px', fontSize: '0.8rem' }}>
551|551|551|                            <span style={{ color: 'var(--accent)' }}>📚</span> This container is part of Compose stack <strong>{project}</strong>.
552|552|552|                            Consider migrating the entire stack.
553|553|553|                          </div>
554|554|554|                        );
555|555|555|                      }
556|556|556|                      return null;
557|557|557|                    })()}
558|558|558|                  </div>
559|559|559|                )}
560|560|560|              </div>
561|561|561|            )}
562|562|562|          </div>
563|563|563|        );
564|564|564|
565|565|565|      // ── STEP 2: Analyze ──
566|566|566|      case 2:
567|567|567|        if (!analysis) return <div className="loading-center"><div className="spinner spinner-lg" /></div>;
568|568|568|        const warnings = analysis.warnings || [];
569|569|569|        const volumes = analysis.volumes || [];
570|570|570|        const dbConns = analysis.dbConnections || [];
571|571|571|        return (
572|572|572|          <div style={{ display: 'grid', gap: '16px' }}>
573|573|573|            {/* Container info */}
574|574|574|            <div className="card">
575|575|575|              <h3>Container</h3>
576|576|576|              <table>
577|577|577|                <tbody>
578|578|578|                  <tr><td style={{ color: 'var(--text-secondary)', width: '120px' }}>Name</td><td className="mono" style={{ fontWeight: 500 }}>{analysis.containerName || '—'}</td></tr>
579|579|579|                  <tr><td style={{ color: 'var(--text-secondary)' }}>Image</td><td className="mono">{analysis.image || '—'}</td></tr>
580|580|580|                  <tr><td style={{ color: 'var(--text-secondary)' }}>ID</td><td className="mono" style={{ fontSize: '0.8rem' }}>{analysis.containerId || '—'}</td></tr>
581|581|581|                </tbody>
582|582|582|              </table>
583|583|583|            </div>
584|584|584|
585|585|585|            {/* Warnings */}
586|586|586|            {warnings.length > 0 && (
587|587|587|              <div className="card" style={{ borderLeft: '3px solid var(--yellow)' }}>
588|588|588|                <h3 style={{ color: 'var(--yellow)' }}>⚠ Warnings</h3>
589|589|589|                {warnings.map((w, i) => (
590|590|590|                  <div key={i} style={{ padding: '6px 0', fontSize: '0.85rem', color: 'var(--yellow)' }}>
591|591|591|                    ⚠ {w}
592|592|592|                  </div>
593|593|593|                ))}
594|594|594|              </div>
595|595|595|            )}
596|596|596|
597|597|597|            {/* Volumes */}
598|598|598|            {volumes.length > 0 && (
599|599|599|              <div className="card">
600|600|600|                <h3>Volumes ({volumes.length})</h3>
601|601|601|                <table>
602|602|602|                  <thead>
603|603|603|                    <tr>
604|604|604|                      <th>Name</th>
605|605|605|                      <th>Driver</th>
606|606|606|                      <th>Category</th>
607|607|607|                      <th>Size</th>
608|608|608|                      <th>Shared</th>
609|609|609|                      <th>Method</th>
610|610|610|                      <th></th>
611|611|611|                    </tr>
612|612|612|                  </thead>
613|613|613|                  <tbody>
614|614|614|                    {volumes.map(v => {
615|615|615|                      const sizeGB = v.sizeBytes ? (v.sizeBytes / 1073741824).toFixed(1) : '—';
616|616|616|                      return (
617|617|617|                        <tr key={v.name}>
618|618|618|                          <td className="mono" style={{ fontWeight: 500 }}>{v.name}</td>
619|619|619|                          <td>{v.driver || '—'}</td>
620|620|620|                          <td>{v.driverCategory || '—'}</td>
621|621|621|                          <td className="mono">{sizeGB === '—' ? '—' : `${sizeGB} GB`}</td>
622|622|622|                          <td>{v.shared ? '🔗 Yes' : 'No'}</td>
623|623|623|                          <td><span className="mono" style={{ fontSize: '0.75rem', color: 'var(--accent)' }}>{v.transferMethod || '—'}</span></td>
624|624|624|                          <td>
625|625|625|                            <button className="btn-sm" onClick={() => setInspectVolume(v.name)}>🔍 Inspect</button>
626|626|626|                          </td>
627|627|627|                        </tr>
628|628|628|                      );
629|629|629|                    })}
630|630|630|                  </tbody>
631|631|631|                </table>
632|632|632|              </div>
633|633|633|            )}
634|634|634|
635|635|635|            {/* DB Connections */}
636|636|636|            {dbConns.length > 0 && (
637|637|637|              <div className="card">
638|638|638|                <h3>Database Connections ({dbConns.length})</h3>
639|639|639|                <ConnectionReview
640|640|640|                  connections={dbConns.map(c => ({
641|641|641|                    ...c,
642|642|642|                    resolved: connectionResolutions[c.varName]?.resolved || false,
643|643|643|                    resolution: connectionResolutions[c.varName]?.action,
644|644|644|                  }))}
645|645|645|                  onUpdate={(varName, action) => setConnectionResolutions(prev => ({
646|646|646|                    ...prev, [varName]: { action, resolved: true }
647|647|647|                  }))}
648|648|648|                  blocked={false}
649|649|649|                />
650|650|650|              </div>
651|651|651|            )}
652|652|652|
653|653|653|            {/* Size estimate */}
654|654|654|            {analysis.estimatedSizeBytes > 0 && (
655|655|655|              <div className="card" style={{ borderLeft: '3px solid var(--accent)' }}>
656|656|656|                <h3>Estimated Transfer Size</h3>
657|657|657|                <div className="mono" style={{ fontSize: '1.2rem', color: 'var(--accent)' }}>
658|658|658|                  {(analysis.estimatedSizeBytes / 1073741824).toFixed(1)} GB
659|659|659|                  {analysis.compressed ? ' (compressed estimate)' : ''}
660|660|660|                </div>
661|661|661|              </div>
662|662|662|            )}
663|663|663|          </div>
664|664|664|        );
665|665|665|
666|666|666|      // ── STEP 3: Strategy ──
667|667|667|      case 3:
668|668|668|        return (
669|669|669|          <MigrationPlan
670|670|670|            plan={analysis || {}}
671|671|671|            volumes={analysis?.volumes || []}
672|672|672|            onUpdate={handleStrategyUpdate}
673|673|673|          />
674|674|674|        );
675|675|675|
676|676|676|      // ── STEP 4: Credentials Review ──
677|677|677|      case 4: {
678|678|678|        const envVars = analysis?.env_vars || [];
679|679|679|        const volOpts = (analysis?.volumes || []).filter(v => v.options && Object.keys(v.options).length > 0);
680|680|680|        const hasComposeSecrets = analysis?.has_compose_secrets;
681|681|681|        return (
682|682|682|          <div style={{ display: 'grid', gap: '16px' }}>
683|683|683|            <div className="card" style={{ borderLeft: '3px solid var(--yellow)' }}>
684|684|684|              <h3>⚠ Security Notice</h3>
685|685|685|              <div style={{ fontSize: '0.85rem', color: 'var(--text-secondary)' }}>
686|686|686|                All credentials are masked by default. Revealing them will be logged in the audit trail.
687|687|687|                Remember to delete any temporary files created during migration.
688|688|688|              </div>
689|689|689|            </div>
690|690|690|
691|691|691|            {hasComposeSecrets && (
692|692|692|              <div className="card" style={{ borderLeft: '3px solid var(--red)', background: 'var(--red-dim)' }}>
693|693|693|                <div style={{ color: '#fff', fontSize: '0.85rem' }}>
694|694|694|                  ⚠ This container uses Docker Compose secrets. Secrets will NOT be transferred.
695|695|695|                  You must manually re-create secrets on the target host.
696|696|696|                </div>
697|697|697|              </div>
698|698|698|            )}
699|699|699|
700|700|700|            {envVars.length > 0 && (
701|701|701|              <div className="card">
702|702|702|                <h3>Environment Variables ({envVars.length})</h3>
703|703|703|                <table>
704|704|704|                  <thead>
705|705|705|                    <tr>
706|706|706|                      <th>Variable</th>
707|707|707|                      <th>Value</th>
708|708|708|                      <th></th>
709|709|709|                    </tr>
710|710|710|                  </thead>
711|711|711|                  <tbody>
712|712|712|                    {envVars.map((env, i) => {
713|713|713|                      const isRevealed = credentialsRevealed[env.name];
714|714|714|                      const isConfirming = credRevealConfirm === env.name;
715|715|715|                      const looksSensitive = (env.name || '').toLowerCase().match(/pass|secret|key|token|auth|cred/);
716|716|716|                      return (
717|717|717|                        <tr key={i}>
718|718|718|                          <td className="mono" style={{ fontWeight: 500 }}>{env.name}</td>
719|719|719|                          <td className="mono" style={{ fontSize: '0.8rem' }}>
720|720|720|                            {isRevealed ? (env.value_masked || env.value || '') : (
721|721|721|                              <span style={{ letterSpacing: '2px', color: looksSensitive ? 'var(--yellow)' : 'inherit' }}>
722|722|722|                                ••••••••
723|723|723|                              </span>
724|724|724|                            )}
725|725|725|                          </td>
726|726|726|                          <td>
727|727|727|                            <button
728|728|728|                              className="btn-sm"
729|729|729|                              onClick={() => handleRevealCredential(env.name)}
730|730|730|                              style={isConfirming ? { background: 'var(--yellow-dim)', borderColor: 'var(--yellow)', color: '#fff' } : {}}
731|731|731|                            >
732|732|732|                              {isRevealed ? 'Hide' : isConfirming ? '⚠ Confirm' : '👁 Reveal'}
733|733|733|                            </button>
734|734|734|                          </td>
735|735|735|                        </tr>
736|736|736|                      );
737|737|737|                    })}
738|738|738|                  </tbody>
739|739|739|                </table>
740|740|740|              </div>
741|741|741|            )}
742|742|742|
743|743|743|            {volOpts.length > 0 && (
744|744|744|              <div className="card">
745|745|745|                <h3>Volume Options ({volOpts.length} volumes with options)</h3>
746|746|746|                <div style={{ fontSize: '0.8rem', color: 'var(--text-secondary)', marginBottom: '12px' }}>
747|747|747|                  Volume options may contain sensitive data. Use the Volume Inspector for details.
748|748|748|                </div>
749|749|749|                {volOpts.map(v => (
750|750|750|                  <div key={v.name} style={{ display: 'flex', alignItems: 'center', gap: '12px', marginBottom: '6px' }}>
751|751|751|                    <span className="mono">{v.name}</span>
752|752|752|                    <button className="btn-sm" onClick={() => setInspectVolume(v.name)}>🔍 Inspect</button>
753|753|753|                  </div>
754|754|754|                ))}
755|755|755|              </div>
756|756|756|            )}
757|757|757|
758|758|758|            {envVars.length === 0 && !hasComposeSecrets && volOpts.length === 0 && (
759|759|759|              <div className="text-secondary" style={{ padding: '24px', textAlign: 'center' }}>
760|760|760|                No credentials to review
761|761|761|              </div>
762|762|762|            )}
763|763|763|
764|764|764|            {revealAudit.length > 0 && (
765|765|765|              <div className="card" style={{ borderLeft: '3px solid var(--yellow)' }}>
766|766|766|                <h3>🔍 Audit Log ({revealAudit.length} entries this session)</h3>
767|767|767|                <div style={{ maxHeight: '120px', overflow: 'auto', fontSize: '0.75rem' }}>
768|768|768|                  {revealAudit.map((entry, i) => (
769|769|769|                    <div key={i} className="mono" style={{ padding: '2px 0', color: 'var(--text-secondary)' }}>
770|770|770|                      [{entry.timestamp}] {entry.action} — {entry.detail?.variable || entry.detail}
771|771|771|                    </div>
772|772|772|                  ))}
773|773|773|                </div>
774|774|774|              </div>
775|775|775|            )}
776|776|776|          </div>
777|777|777|        );
778|778|778|      }
779|779|779|
780|780|780|      // ── STEP 5: Connection Fixes ──
781|781|781|      case 5: {
782|782|782|        const conns = (analysis?.dbConnections || []).map(c => ({
783|783|783|          ...c,
784|784|784|          resolved: connectionResolutions[c.varName]?.resolved || false,
785|785|785|          resolution: connectionResolutions[c.varName]?.action,
786|786|786|        }));
787|787|787|        return (
788|788|788|          <div>
789|789|789|            <div className="card mb-16">
790|790|790|              <h3>Connection Review</h3>
791|791|791|              <div style={{ fontSize: '0.85rem', color: 'var(--text-secondary)', marginBottom: '12px' }}>
792|792|792|                Review database connections that may break after migration. All values are masked.
793|793|793|              </div>
794|794|794|            </div>
795|795|795|            <ConnectionReview
796|796|796|              connections={conns}
797|797|797|              onUpdate={handleConnectionUpdate}
798|798|798|              blocked={true}
799|799|799|            />
800|800|800|          </div>
801|801|801|        );
802|802|802|      }
803|803|803|
804|804|804|      // ── STEP 6: Target ──
805|805|805|      case 6:
806|806|806|        return (
807|807|807|          <div style={{ display: 'grid', gap: '16px' }}>
808|808|808|            <div className="card">
809|809|809|              <h3>Target Endpoint</h3>
810|810|810|              <select
811|811|811|                value={targetEndpoint}
812|812|812|                onChange={e => handleTargetSelect(e.target.value)}
813|813|813|                style={{ width: '100%', marginTop: '8px' }}
814|814|814|              >
815|815|815|                <option value="">— Select target endpoint —</option>
816|816|816|                <option value="local">Local</option>
817|817|817|                {targetEndpoints.filter(ep => (ep.id || ep.Id) !== sourceEndpoint).map(ep => (
818|818|818|                  <option key={ep.id || ep.Id} value={ep.id || ep.Id}>
819|819|819|                    {ep.name || ep.Name}
820|820|820|                  </option>
821|821|821|                ))}
822|822|822|              </select>
823|823|823|            </div>
824|824|824|
825|825|825|            {targetInfo && (
826|826|826|              <div className="card">
827|827|827|                <h3>Target Info</h3>
828|828|828|                <table>
829|829|829|                  <tbody>
830|830|830|                    <tr><td style={{ color: 'var(--text-secondary)', width: '160px' }}>Status</td><td>
831|831|831|                      <span style={{ color: (targetInfo.status || '').toLowerCase() === 'connected' ? 'var(--green)' : 'var(--red)' }}>
832|832|832|                        {targetInfo.status || 'unknown'}
833|833|833|                      </span>
834|834|834|                    </td></tr>
835|835|835|                    <tr><td style={{ color: 'var(--text-secondary)' }}>Docker Version</td><td className="mono">{targetInfo.docker_version || '—'}</td></tr>
836|836|836|                    <tr><td style={{ color: 'var(--text-secondary)' }}>Containers</td><td>{targetInfo.container_count ?? '—'}</td></tr>
837|837|837|                    {targetInfo.disk_free_bytes != null && (
838|838|838|                      <tr><td style={{ color: 'var(--text-secondary)' }}>Disk Free</td><td className="mono">{(targetInfo.disk_free_bytes / 1073741824).toFixed(1)} GB</td></tr>
839|839|839|                    )}
840|840|840|                    {targetInfo.disk_total_bytes != null && (
841|841|841|                      <tr><td style={{ color: 'var(--text-secondary)' }}>Disk Total</td><td className="mono">{(targetInfo.disk_total_bytes / 1073741824).toFixed(1)} GB</td></tr>
842|842|842|                    )}
843|843|843|                  </tbody>
844|844|844|                </table>
845|845|845|              </div>
846|846|846|            )}
847|847|847|
848|848|848|            <div className="card">
849|849|849|              <h3>Target Stack Name</h3>
850|850|850|              <input
851|851|851|                type="text"
852|852|852|                value={targetStackName}
853|853|853|                onChange={e => setTargetStackName(e.target.value)}
854|854|854|                placeholder="Leave blank to use original name"
855|855|855|                style={{ width: '100%', marginTop: '4px' }}
856|856|856|              />
857|857|857|            </div>
858|858|858|          </div>
859|859|859|        );
860|860|860|
861|861|861|      // ── STEP 7: Dry Run ──
862|862|862|      case 7:
863|863|863|        return (
864|864|864|          <div style={{ display: 'grid', gap: '16px' }}>
865|865|865|            {!dryRunResult ? (
866|866|866|              <div style={{ textAlign: 'center', padding: '32px' }}>
867|867|867|                <div className="text-secondary mb-16">Run a dry-run to preview all commands before execution</div>
868|868|868|                <button className="btn-primary" onClick={handleDryRun} disabled={loading}>
869|869|869|                  {loading ? 'Running...' : '🚀 Run Dry Run'}
870|870|870|                </button>
871|871|871|              </div>
872|872|872|            ) : (
873|873|873|              <>
874|874|874|                {/* Warnings banner */}
875|875|875|                {dryRunResult.warnings?.length > 0 && (
876|876|876|                  <div className="card" style={{ borderLeft: '3px solid var(--yellow)' }}>
877|877|877|                    <h3 style={{ color: 'var(--yellow)' }}>⚠ Warnings</h3>
878|878|878|                    {dryRunResult.warnings.map((w, i) => (
879|879|879|                      <div key={i} style={{ padding: '4px 0', fontSize: '0.85rem', color: 'var(--yellow)' }}>
880|880|880|                        ⚠ {w}
881|881|881|                      </div>
882|882|882|                    ))}
883|883|883|                  </div>
884|884|884|                )}
885|885|885|
886|886|886|                {/* Security banners */}
887|887|887|                {transferMethod === 'rsync-over-ssh' && (
888|888|888|                  <div className="card" style={{ borderLeft: '3px solid var(--green)' }}>
889|889|889|                    <div style={{ color: 'var(--green)', fontSize: '0.85rem' }}>
890|890|890|                      🔒 SSH transfer — all data encrypted in transit
891|891|891|                    </div>
892|892|892|                  </div>
893|893|893|                )}
894|894|894|
895|895|895|                {analysis?.estimatedSizeBytes > 10 * 1073741824 && (
896|896|896|                  <div className="card" style={{ borderLeft: '3px solid var(--yellow)' }}>
897|897|897|                    <div style={{ color: 'var(--yellow)', fontSize: '0.85rem' }}>
898|898|898|                      ⚠ Large volume ({((analysis?.estimatedSizeBytes || 0) / 1073741824).toFixed(1)} GB) —
899|899|899|                      transfer may take significant time
900|900|900|                    </div>
901|901|901|                  </div>
902|902|902|                )}
903|903|903|
904|904|904|                <div className="card" style={{ borderLeft: '3px solid var(--red)' }}>
905|905|905|                  <div style={{ color: 'var(--red)', fontSize: '0.8rem' }}>
906|906|906|                    ⚠ These commands are for REVIEW only. Actual execution requires admin confirmation.
907|907|907|                    Credentials remain masked at all times.
908|908|908|                  </div>
909|909|909|                </div>
910|910|910|
911|911|911|                {/* Commands */}
912|912|912|                <div className="card">
913|913|913|                  <h3>Migration Commands</h3>
914|914|914|                  {dryRunResult.commands?.map((cmd, i) => (
915|915|915|                    <div key={i} style={{ marginBottom: '12px' }}>
916|916|916|                      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '4px' }}>
917|917|917|                        <span style={{ fontSize: '0.75rem', color: 'var(--text-secondary)' }}>
918|918|918|                          Step {i + 1}: {cmd.host === 'source' ? '🖥 Source' : cmd.host === 'target' ? '🖥 Target' : '⚙ System'}
919|919|919|                        </span>
920|920|920|                        <button
921|921|921|                          className="btn-sm"
922|922|922|                          onClick={() => {
923|923|923|                            navigator.clipboard.writeText(cmd.command || cmd);
924|924|924|                            toast('Copied to clipboard', 'success');
925|925|925|                          }}
926|926|926|                          style={{ fontSize: '0.65rem', padding: '2px 8px' }}
927|927|927|                        >
928|928|928|                          📋 Copy
929|929|929|                        </button>
930|930|930|                      </div>
931|931|931|                      <pre style={{
932|932|932|                        background: 'var(--bg-primary)',
933|933|933|                        border: '1px solid var(--border)',
934|934|934|                        borderRadius: '6px',
935|935|935|                        padding: '12px',
936|936|936|                        fontSize: '0.78rem',
937|937|937|                        fontFamily: '"JetBrains Mono", monospace',
938|938|938|                        color: 'var(--green)',
939|939|939|                        overflow: 'auto',
940|940|940|                        whiteSpace: 'pre-wrap',
941|941|941|                        wordBreak: 'break-all',
942|942|942|                      }}>
943|943|943|                        <code>{typeof cmd === 'string' ? cmd : cmd.command}</code>
944|944|944|                      </pre>
945|945|945|                      {cmd.annotation && (
946|946|946|                        <div style={{ fontSize: '0.7rem', color: 'var(--text-secondary)', marginTop: '2px' }}>
947|947|947|                          {cmd.annotation}
948|948|948|                        </div>
949|949|949|                      )}
950|950|950|                    </div>
951|951|951|                  ))}
952|952|952|                </div>
953|953|953|
954|954|954|                {/* Summary */}
955|955|955|                <div className="card">
956|956|956|                  <h3>Migration Summary</h3>
957|957|957|                  <table>
958|958|958|                    <tbody>
959|959|959|                      <tr><td style={{ color: 'var(--text-secondary)', width: '180px' }}>Migration ID</td><td className="mono">{dryRunResult.migrationId || '—'}</td></tr>
960|960|960|                      <tr><td style={{ color: 'var(--text-secondary)' }}>Source</td><td>{dryRunResult.sourceEndpoint || sourceEndpoint}</td></tr>
961|961|961|                      <tr><td style={{ color: 'var(--text-secondary)' }}>Target</td><td>{dryRunResult.targetEndpoint || targetEndpoint}</td></tr>
962|962|962|                      <tr><td style={{ color: 'var(--text-secondary)' }}>Container</td><td className="mono">{dryRunResult.containerName || '—'}</td></tr>
963|963|963|                      <tr><td style={{ color: 'var(--text-secondary)' }}>Estimated Size</td><td className="mono">{dryRunResult.estimatedSizeBytes > 0 ? `${(dryRunResult.estimatedSizeBytes / 1073741824).toFixed(1)} GB` : '—'}</td></tr>
964|964|964|                      <tr><td style={{ color: 'var(--text-secondary)' }}>Method</td><td className="mono">{transferMethod}</td></tr>
965|965|965|                      <tr><td style={{ color: 'var(--text-secondary)' }}>Compression</td><td className="mono">{compression}</td></tr>
966|966|966|                    </tbody>
967|967|967|                  </table>
968|968|968|                </div>
969|969|969|              </>
970|970|970|            )}
971|971|971|          </div>
972|972|972|        );
973|973|973|
974|974|974|      // ── STEP 8: Execute ──
975|975|975|      case 8: {
976|976|976|        const totalGroups = execCommands.length;
977|977|977|        const elapsed = execStartTime ? Math.floor((Date.now() - execStartTime) / 1000) : 0;
978|978|978|        const elapsedStr = elapsed > 60 ? `${Math.floor(elapsed / 60)}m ${elapsed % 60}s` : `${elapsed}s`;
979|979|979|
980|980|980|        return (
981|981|981|          <div style={{ display: 'grid', gap: '16px' }}>
982|982|982|            {/* Progress */}
983|983|983|            <div className="card">
984|984|984|              <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '8px' }}>
985|985|985|                <h3 style={{ margin: 0 }}>
986|986|986|                  {execPhase === 'pending' ? 'Ready to Execute' :
987|987|987|                   execPhase === 'running' ? `Executing (${currentCommandGroup + 1}/${totalGroups})` :
988|988|988|                   'Execution Complete'}
989|989|989|                </h3>
990|990|990|                <span style={{ fontSize: '0.8rem', color: 'var(--text-secondary)' }}>{elapsedStr}</span>
991|991|991|              </div>
992|992|992|              {/* Progress bar */}
993|993|993|              <div style={{
994|994|994|                height: '6px',
995|995|995|                background: 'var(--bg-tertiary)',
996|996|996|                borderRadius: '3px',
997|997|997|                overflow: 'hidden',
998|998|998|                marginBottom: '16px',
999|999|999|              }}>
1000|1000|1000|                <div style={{
1001|1001|1001|                  height: '100%',
1002|1002|1002|                  width: `${execPhase === 'done' ? 100 : execPhase === 'pending' ? 0 : (currentCommandGroup / Math.max(totalGroups, 1)) * 100}%`,
1003|1003|1003|                  background: execPhase === 'done' ? 'var(--green)' : 'var(--accent)',
1004|1004|1004|                  borderRadius: '3px',
1005|1005|1005|                  transition: 'width 0.5s ease',
1006|1006|1006|                }} />
1007|1007|1007|              </div>
1008|1008|1008|
1009|1009|1009|              {/* Step status list */}
1010|1010|1010|              <div style={{ display: 'grid', gap: '4px' }}>
1011|1011|1011|                {execCommands.map((cmd, i) => {
1012|1012|1012|                  const result = execResults[i];
1013|1013|1013|                  const isComplete = !!result;
1014|1014|1014|                  const isCurrent = execPhase === 'running' && i === currentCommandGroup && !isComplete;
1015|1015|1015|                  const isPending = i > currentCommandGroup || execPhase === 'pending';
1016|1016|1016|
1017|1017|1017|                  return (
1018|1018|1018|                    <div key={i} style={{
1019|1019|1019|                      display: 'flex',
1020|1020|1020|                      alignItems: 'center',
1021|1021|1021|                      gap: '10px',
1022|1022|1022|                      padding: '8px 12px',
1023|1023|1023|                      borderRadius: '6px',
1024|1024|1024|                      background: isCurrent ? 'var(--bg-tertiary)' : 'transparent',
1025|1025|1025|                      border: isCurrent ? '1px solid var(--accent)' : '1px solid transparent',
1026|1026|1026|                    }}>
1027|1027|1027|                      <span style={{
1028|1028|1028|                        fontSize: '0.9rem',
1029|1029|1029|                        width: '20px',
1030|1030|1030|                        textAlign: 'center',
1031|1031|1031|                      }}>
1032|1032|1032|                        {isComplete ? <span style={{ color: 'var(--green)' }}>✓</span> :
1033|1033|1033|                         isCurrent ? <span style={{ color: 'var(--accent)' }}>▸</span> :
1034|1034|1034|                         <span style={{ color: 'var(--text-secondary)' }}>○</span>}
1035|1035|1035|                      </span>
1036|1036|1036|                      <code style={{
1037|1037|1037|                        flex: 1,
1038|1038|1038|                        fontSize: '0.75rem',
1039|1039|1039|                        color: isCurrent ? 'var(--accent)' : isComplete ? 'var(--text-secondary)' : 'var(--text-secondary)',
1040|1040|1040|                        overflow: 'hidden',
1041|1041|1041|                        textOverflow: 'ellipsis',
1042|1042|1042|                        whiteSpace: 'nowrap',
1043|1043|1043|                      }}>
1044|1044|1044|                        {typeof cmd === 'string' ? cmd : (cmd.command || cmd).substring(0, 80)}
1045|1045|1045|                      </code>
1046|1046|1046|                      {result?.time && (
1047|1047|1047|                        <span style={{ fontSize: '0.7rem', color: 'var(--text-secondary)' }}>
1048|1048|1048|                          {result.time > 60000 ? `${Math.floor(result.time / 60000)}m` : `${Math.floor(result.time / 1000)}s`}
1049|1049|1049|                        </span>
1050|1050|1050|                      )}
1051|1051|1051|                    </div>
1052|1052|1052|                  );
1053|1053|1053|                })}
1054|1054|1054|              </div>
1055|1055|1055|            </div>
1056|1056|1056|
1057|1057|1057|            {/* Current command display */}
1058|1058|1058|            {execPhase === 'running' && execCommands[currentCommandGroup] && (
1059|1059|1059|              <div className="card">
1060|1060|1060|                <h3>Current Command</h3>
1061|1061|1061|                <div style={{ fontSize: '0.8rem', color: 'var(--text-secondary)', marginBottom: '8px' }}>
1062|1062|1062|                  Run this command on the appropriate host, then click <strong>Done</strong> to proceed.
1063|1063|1063|                </div>
1064|1064|1064|                <pre style={{
1065|1065|1065|                  background: 'var(--bg-primary)',
1066|1066|1066|                  border: '1px solid var(--accent)',
1067|1067|1067|                  borderRadius: '6px',
1068|1068|1068|                  padding: '16px',
1069|1069|1069|                  fontSize: '0.8rem',
1070|1070|1070|                  fontFamily: '"JetBrains Mono", monospace',
1071|1071|1071|                  color: 'var(--accent)',
1072|1072|1072|                  overflow: 'auto',
1073|1073|1073|                  whiteSpace: 'pre-wrap',
1074|1074|1074|                  wordBreak: 'break-all',
1075|1075|1075|                }}>
1076|1076|1076|                  <code>{typeof execCommands[currentCommandGroup] === 'string'
1077|1077|1077|                    ? execCommands[currentCommandGroup]
1078|1078|1078|                    : execCommands[currentCommandGroup].command}</code>
1079|1079|1079|                </pre>
1080|1080|1080|                <div className="btn-group" style={{ marginTop: '12px' }}>
1081|1081|1081|                  <button className="btn-primary" onClick={handleCommandGroupDone}>
1082|1082|1082|                    ✅ Done — Continue
1083|1083|1083|                  </button>
1084|1084|1084|                  <button className="btn-warning" onClick={handleCancelExecution}>
1085|1085|1085|                    🛑 Cancel
1086|1086|1086|                  </button>
1087|1087|1087|                </div>
1088|1088|1088|              </div>
1089|1089|1089|            )}
1090|1090|1090|
1091|1091|1091|            {/* Action buttons */}
1092|1092|1092|            <div className="btn-group" style={{ justifyContent: 'center' }}>
1093|1093|1093|              {execPhase === 'pending' && (
1094|1094|1094|                <button className="btn-primary" onClick={handleStartExecution}>
1095|1095|1095|                  ▶ Start Execution
1096|1096|1096|                </button>
1097|1097|1097|              )}
1098|1098|1098|              {execPhase === 'done' && (
1099|1099|1099|                <div style={{ textAlign: 'center' }}>
1100|1100|1100|                  <div style={{ color: 'var(--green)', fontSize: '1.1rem', fontWeight: 600, marginBottom: '8px' }}>
1101|1101|1101|                    ✓ All commands completed
1102|1102|1102|                  </div>
1103|1103|1103|                </div>
1104|1104|1104|              )}
1105|1105|1105|            </div>
1106|1106|1106|
1107|1107|1107|            <div className="card" style={{ borderLeft: '3px solid var(--yellow)', marginTop: '8px' }}>
1108|1108|1108|              <div style={{ fontSize: '0.8rem', color: 'var(--text-secondary)' }}>
1109|1109|1109|                ⚠ <strong>Marionette never executes commands directly.</strong> Commands must be run manually by an administrator.
1110|1110|1110|                Credentials are never shown in plaintext.
1111|1111|1111|              </div>
1112|1112|1112|            </div>
1113|1113|1113|          </div>
1114|1114|1114|        );
1115|1115|1115|      }
1116|1116|1116|
1117|1117|1117|      // ── STEP 9: Verify ──
1118|1118|1118|      case 9:
1119|1119|1119|        const v = verification || {};
1120|1120|1120|        const allSteps = [
1121|1121|1121|          { name: 'Container Export', status: v.export_status },
1122|1122|1122|          { name: 'Volume Transfer', status: v.volume_status },
1123|1123|1123|          { name: 'DB Connection Migration', status: v.db_status },
1124|1124|1124|          { name: 'Container Import', status: v.import_status },
1125|1125|1125|          { name: 'Connectivity Test', status: v.connectivity_status },
1126|1126|1126|        ];
1127|1127|1127|
1128|1128|1128|        return (
1129|1129|1129|          <div style={{ display: 'grid', gap: '16px' }}>
1130|1130|1130|            <div className="card" style={{ borderLeft: '3px solid ' + (v.success ? 'var(--green)' : 'var(--red)') }}>
1131|1131|1131|              <h3>Migration {v.success ? 'Successful ✓' : 'Completed with issues ⚠'}</h3>
1132|1132|1132|              <table>
1133|1133|1133|                <tbody>
1134|1134|1134|                  <tr><td style={{ color: 'var(--text-secondary)', width: '160px' }}>Duration</td><td>{v.duration || '—'}</td></tr>
1135|1135|1135|                  <tr><td style={{ color: 'var(--text-secondary)' }}>Bytes Transferred</td><td className="mono">{v.bytes_transferred ? `${(v.bytes_transferred / 1073741824).toFixed(2)} GB` : '—'}</td></tr>
1136|1136|1136|                  <tr><td style={{ color: 'var(--text-secondary)' }}>Container</td><td className="mono">{v.container_name || analysis?.containerName || '—'}</td></tr>
1137|1137|1137|                  <tr><td style={{ color: 'var(--text-secondary)' }}>Source → Target</td><td>{sourceEndpoint} → {targetEndpoint}</td></tr>
1138|1138|1138|                </tbody>
1139|1139|1139|              </table>
1140|1140|1140|            </div>
1141|1141|1141|
1142|1142|1142|            {/* Step-by-step results */}
1143|1143|1143|            <div className="card">
1144|1144|1144|              <h3>Step Results</h3>
1145|1145|1145|              <div style={{ display: 'grid', gap: '6px' }}>
1146|1146|1146|                {allSteps.map((s, i) => (
1147|1147|1147|                  <div key={i} style={{
1148|1148|1148|                    display: 'flex',
1149|1149|1149|                    alignItems: 'center',
1150|1150|1150|                    gap: '10px',
1151|1151|1151|                    padding: '8px 12px',
1152|1152|1152|                    borderRadius: '6px',
1153|1153|1153|                    background: 'var(--bg-tertiary)',
1154|1154|1154|                  }}>
1155|1155|1155|                    <span style={{
1156|1156|1156|                      color: s.status === 'success' ? 'var(--green)' :
1157|1157|1157|                             s.status === 'failed' ? 'var(--red)' :
1158|1158|1158|                             s.status === 'skipped' ? 'var(--yellow)' : 'var(--text-secondary)',
1159|1159|1159|                      fontWeight: 600,
1160|1160|1160|                    }}>
1161|1161|1161|                      {s.status === 'success' ? '✓' : s.status === 'failed' ? '✗' : s.status === 'skipped' ? '⊘' : '—'}
1162|1162|1162|                    </span>
1163|1163|1163|                    <span style={{ flex: 1 }}>{s.name}</span>
1164|1164|1164|                    <span style={{ fontSize: '0.75rem', color: 'var(--text-secondary)' }}>
1165|1165|1165|                      {s.status || 'pending'}
1166|1166|1166|                    </span>
1167|1167|1167|                  </div>
1168|1168|1168|                ))}
1169|1169|1169|              </div>
1170|1170|1170|            </div>
1171|1171|1171|
1172|1172|1172|            {/* Connectivity test */}
1173|1173|1173|            {v.connectivity_result && (
1174|1174|1174|              <div className="card" style={{ borderLeft: `3px solid ${v.connectivity_result.success ? 'var(--green)' : 'var(--red)'}` }}>
1175|1175|1175|                <h3>Connectivity Test</h3>
1176|1176|1176|                <div style={{ fontSize: '0.85rem' }}>
1177|1177|1177|                  {v.connectivity_result.success ? (
1178|1178|1178|                    <span style={{ color: 'var(--green)' }}>✓ Connected successfully</span>
1179|1179|1179|                  ) : (
1180|1180|1180|                    <span style={{ color: 'var(--red)' }}>✗ {v.connectivity_result.error || 'Connection failed'}</span>
1181|1181|1181|                  )}
1182|1182|1182|                </div>
1183|1183|1183|              </div>
1184|1184|1184|            )}
1185|1185|1185|
1186|1186|1186|            {/* Post-migration actions */}
1187|1187|1187|            <div className="card">
1188|1188|1188|              <h3>Post-Migration Actions</h3>
1189|1189|1189|              <div className="btn-group">
1190|1190|1190|                <button
1191|1191|1191|                  className="btn-danger"
1192|1192|1192|                  onClick={() => handlePostMigration('remove_source')}
1193|1193|1193|                >
1194|1194|1194|                  🗑 Remove from Source
1195|1195|1195|                </button>
1196|1196|1196|                <button
1197|1197|1197|                  className="btn-warning"
1198|1198|1198|                  onClick={() => handlePostMigration('rollback')}
1199|1199|1199|                >
1200|1200|1200|                  🔄 Restart on Source (Rollback)
1201|1201|1201|                </button>
1202|1202|1202|              </div>
1203|1203|1203|            </div>
1204|1204|1204|
1205|1205|1205|            {/* Audit log */}
1206|1206|1206|            {revealAudit.length > 0 && (
1207|1207|1207|              <div className="card" style={{ borderLeft: '3px solid var(--yellow)' }}>
1208|1208|1208|                <h3>🔍 Migration Audit Log</h3>
1209|1209|1209|                <div style={{ maxHeight: '150px', overflow: 'auto', fontSize: '0.75rem' }}>
1210|1210|1210|                  {revealAudit.map((entry, i) => (
1211|1211|1211|                    <div key={i} className="mono" style={{ padding: '2px 0', color: 'var(--text-secondary)' }}>
1212|1212|1212|                      [{entry.timestamp}] {entry.action} — {entry.detail?.variable || JSON.stringify(entry.detail)}
1213|1213|1213|                    </div>
1214|1214|1214|                  ))}
1215|1215|1215|                </div>
1216|1216|1216|              </div>
1217|1217|1217|            )}
1218|1218|1218|          </div>
1219|1219|1219|        );
1220|1220|1220|    }
1221|1221|1221|  };
1222|1222|1222|
1223|1223|1223|  return (
1224|1224|1224|    <div>
1225|1225|1225|      <div className="section-header">
1226|1226|1226|        <h1>🚚 Container Migration</h1>
1227|1227|1227|        <div className="btn-group">
1228|1228|1228|          <button className="btn-sm" onClick={() => navigate('dashboard')}>
1229|1229|1229|            ← Dashboard
1230|1230|1230|          </button>
1231|1231|1231|        </div>
1232|1232|1232|      </div>
1233|1233|1233|
1234|1234|1234|      {error && (
1235|1235|1235|        <div className="text-danger mb-16" style={{ padding: '12px', background: 'var(--red-dim)', borderRadius: '6px' }}>
1236|1236|1236|          {error}
1237|1237|1237|        </div>
1238|1238|1238|      )}
1239|1239|1239|
1240|1240|1240|      <StepIndicator currentStep={step} completedSteps={completedSteps} />
1241|1241|1241|
1242|1242|1242|      <div style={{ minHeight: '300px' }}>
1243|1243|1243|        {renderStepContent()}
1244|1244|1244|      </div>
1245|1245|1245|
1246|1246|1246|      {/* Navigation footer */}
1247|1247|1247|      <div style={{
1248|1248|1248|        display: 'flex',
1249|1249|1249|        justifyContent: 'space-between',
1250|1250|1250|        marginTop: '24px',
1251|1251|1251|        padding: '16px 0',
1252|1252|1252|        borderTop: '1px solid var(--border)',
1253|1253|1253|      }}>
1254|1254|1254|        <button
1255|1255|1255|          onClick={handlePrev}
1256|1256|1256|          disabled={step === 1}
1257|1257|1257|        >
1258|1258|1258|          ← Previous
1259|1259|1259|        </button>
1260|1260|1260|
1261|1261|1261|        <div className="text-secondary" style={{ fontSize: '0.8rem' }}>
1262|1262|1262|          Step {step} of {TOTAL_STEPS}
1263|1263|1263|        </div>
1264|1264|1264|
1265|1265|1265|        {step !== 8 && (
1266|1266|1266|          <button
1267|1267|1267|            className="btn-primary"
1268|1268|1268|            onClick={handleNext}
1269|1269|1269|            disabled={loading || !canGoNext()}
1270|1270|1270|          >
1271|1271|1271|            {step === 9 ? '✅ Finish' : step === 1 ? 'Analyze →' : loading ? 'Loading...' : 'Next →'}
1272|1272|1272|          </button>
1273|1273|1273|        )}
1274|1274|1274|      </div>
1275|1275|1275|
1276|1276|1276|      {/* Volume Inspector Modal */}
1277|1277|1277|      {inspectVolume && (
1278|1278|1278|        <VolumeInspector
1279|1279|1279|          volumeName={inspectVolume}
1280|1280|1280|          onClose={() => setInspectVolume(null)}
1281|1281|1281|        />
1282|1282|1282|      )}
1283|1283|1283|    </div>
1284|1284|1284|  );
1285|1285|1285|}
1286|1286|1286|