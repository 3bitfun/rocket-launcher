import { useEffect, useRef, useState } from 'react';
import { Copy, RefreshCw } from 'lucide-react';
import { errorMessage, native } from '../../services/native';

export function InstanceLog({ id }: { id: string }) {
  const [errors, setErrors] = useState(false);
  const [revision, setRevision] = useState(0);
  const [text, setText] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [message, setMessage] = useState('');
  const output = useRef<HTMLPreElement>(null);

  useEffect(() => {
    let active = true;
    setLoading(true);
    setText(null);
    setMessage('');
    void native.instanceLog(id, errors).then((value) => {
      if (active) setText(value);
    }).catch((error) => {
      if (active) setMessage(errorMessage(error));
    }).finally(() => {
      if (active) setLoading(false);
    });
    return () => { active = false; };
  }, [id, errors, revision]);

  useEffect(() => {
    if (output.current) output.current.scrollTop = output.current.scrollHeight;
  }, [text]);

  async function copy() {
    if (!text) return;
    try {
      await navigator.clipboard.writeText(text);
      setMessage('Log copied.');
    } catch {
      setMessage('Could not copy automatically. Select the log text to copy it.');
    }
  }

  return <div className="instance-log">
    <div className="inline-actions">
      <div className="segmented" role="group" aria-label="Log stream">
        <button type="button" className={!errors ? 'active' : ''} aria-pressed={!errors} onClick={() => setErrors(false)}>Output</button>
        <button type="button" className={errors ? 'active' : ''} aria-pressed={errors} onClick={() => setErrors(true)}>Errors</button>
      </div>
      <button className="button secondary small-button" disabled={loading} onClick={() => setRevision((value) => value + 1)}><RefreshCw size={14} /> Refresh</button>
      <button className="button secondary small-button" disabled={!text || loading} onClick={() => void copy()}><Copy size={14} /> Copy</button>
    </div>
    {text ? <pre className="log-view" ref={output} tabIndex={0} aria-label={errors ? 'Minecraft error output' : 'Minecraft output'}>{text}</pre>
      : !message && <p className="panel-footnote" role="status">{loading ? 'Loading the latest session…' : text === null ? 'No session logs yet. Launch this instance to record one.' : 'This stream is empty.'}</p>}
    {message && <p className="panel-footnote" role="status">{message}</p>}
  </div>;
}
