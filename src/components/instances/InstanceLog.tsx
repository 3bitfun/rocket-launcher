import { useEffect, useRef, useState } from 'react';
import { AlertCircle, Copy, RefreshCw } from 'lucide-react';
import { errorMessage, native } from '../../services/native';
import { loaderName } from '../../services/loaders';
import { isGameActive } from '../../services/readiness';
import type { CrashDiagnostics, GameStatus, Instance } from '../../types';

const labels = {
  crash: 'Crash report', jvm: 'JVM report', game: 'Game log', output: 'Output', errors: 'Errors',
} as const;
const outcomes = { failed: 'Failed', exited: 'Exited normally', stopped: 'Stopped', unknown: 'Result unknown' };

type LogKind = keyof typeof labels;

export function InstanceLog({ instance, status }: { instance: Instance; status: GameStatus }) {
  const [tab, setTab] = useState<LogKind | null>(null);
  const [sessionId, setSessionId] = useState('');
  const [revision, setRevision] = useState(0);
  const [diagnostics, setDiagnostics] = useState<CrashDiagnostics | null>(null);
  const [loading, setLoading] = useState(true);
  const [message, setMessage] = useState('');
  const output = useRef<HTMLPreElement>(null);
  const running = !sessionId && isGameActive(status.readiness);
  const logs = loading ? [] : diagnostics?.logs ?? [];
  const log = logs.find((item) => item.kind === tab)
    ?? (Object.keys(labels) as LogKind[]).map((kind) => logs.find((item) => item.kind === kind && item.text)).find(Boolean)
    ?? logs[0];
  const findings = running ? [] : diagnostics?.findings ?? [];
  const outcome = diagnostics?.session?.outcome;
  const exitCode = diagnostics?.session?.exitCode;
  const sessionLabel = running ? 'Minecraft is active. Refresh for newer output.'
    : outcome === 'failed' ? `Minecraft exited with an error${exitCode == null ? '.' : ` (code ${exitCode}).`}`
    : outcome === 'stopped' ? 'This session was stopped.'
    : outcome === 'exited' ? 'This session exited normally.'
    : logs.length ? 'Available logs were found. The session result is unavailable.'
    : 'No logs were found for this instance.';

  useEffect(() => {
    let active = true;
    setLoading(true);
    setMessage('');
    void native.diagnoseInstance(instance.id, sessionId || undefined).then((value) => {
      if (active) setDiagnostics(value);
    }).catch((error) => {
      if (active) {
        setDiagnostics(null);
        setMessage(errorMessage(error));
      }
    }).finally(() => {
      if (active) setLoading(false);
    });
    return () => { active = false; };
  }, [instance.id, sessionId, status.readiness, status.exitCode, revision]);

  useEffect(() => {
    if (output.current) output.current.scrollTop = log?.kind === 'crash' || log?.kind === 'jvm' ? 0 : output.current.scrollHeight;
  }, [log]);

  async function copy(text: string, label: string) {
    if (!text) return;
    try {
      await navigator.clipboard.writeText(text);
      setMessage(`${label} copied.`);
    } catch {
      setMessage('Could not copy automatically. Select the log text to copy it.');
    }
  }

  function copySummary() {
    const selected = diagnostics?.sessions.find((session) => session.id === diagnostics.selectedSession);
    const summary = [
      'Rocket Launcher crash summary',
      `Current instance settings: Minecraft ${instance.version} · ${loaderName(instance.loader)}${instance.loaderVersion ? ` ${instance.loaderVersion}` : ''}`,
      ...(selected ? [`Session: ${new Date(selected.startedAt * 1000).toISOString()}`] : []),
      sessionLabel,
      ...(findings.length ? findings.map((finding) => `${finding.title} (${finding.source})\n${finding.suggestion}`) : ['No known crash signature found.']),
    ].join('\n\n');
    void copy(summary, 'Summary');
  }

  return <div className="instance-log">
    {!!diagnostics?.sessions.length && <label className="field"><span>Minecraft session</span>
      <select value={sessionId} disabled={loading} onChange={(event) => {
        setSessionId(event.target.value);
        setTab(null);
      }}>
        <option value="">Latest session</option>
        {diagnostics.sessions.map((session, index) => (index > 0 || session.id === sessionId) &&
          <option value={session.id} key={session.id}>{new Date(session.startedAt * 1000).toLocaleString()}{session.outcome ? ` · ${outcomes[session.outcome]}` : ''}</option>)}
      </select>
    </label>}
    <div className="inline-actions">
      {!diagnostics && sessionId && !loading && <button className="button secondary small-button" onClick={() => { setSessionId(''); setTab(null); }}>Back to latest session</button>}
      <button className="button secondary small-button" disabled={loading} onClick={() => setRevision((value) => value + 1)}><RefreshCw size={14} /> Refresh</button>
      <button className="button secondary small-button" disabled={!diagnostics || loading} onClick={copySummary}><Copy size={14} /> Copy summary</button>
    </div>
    {loading && <p className="panel-footnote" role="status">Checking Minecraft session…</p>}
    {!loading && diagnostics && <p className="panel-footnote" role="status">{sessionLabel}{outcome === 'failed' && !findings.length && ' No known cause matched. Check the available logs below.'}</p>}
    {!loading && findings.length > 0 && <div className="crash-findings" aria-label="Possible causes">
      {findings.map((finding) => <div key={finding.title} className="notice"><AlertCircle size={17} /><div>
        <strong>{finding.title}</strong><p>{finding.suggestion}</p><small>Matched in {finding.source}</small>
      </div></div>)}
    </div>}
    {log && <div className="inline-actions">
      <div className="segmented log-tabs" role="group" aria-label="Log source">
        {(Object.keys(labels) as LogKind[]).filter((kind) => logs.some((item) => item.kind === kind)).map((kind) =>
          <button type="button" key={kind} className={log.kind === kind ? 'active' : ''} aria-pressed={log.kind === kind} onClick={() => setTab(kind)}>{labels[kind]}</button>)}
      </div>
      <button className="button secondary small-button" disabled={!log.text || loading} onClick={() => void copy(log.text, 'Log')}><Copy size={14} /> Copy log</button>
    </div>}
    {log && <p className="panel-footnote">{log.name}{log.truncated ? ' · excerpt' : ''}. Review logs for private details before sharing.</p>}
    {log?.text ? <pre className="log-view" ref={output} tabIndex={0} aria-label={labels[log.kind]}>{log.text}</pre>
      : log && <p className="panel-footnote">This log is empty.</p>}
    {message && <p className="panel-footnote" role="status">{message}</p>}
  </div>;
}
