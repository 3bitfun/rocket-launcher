import { useEffect, useState } from 'react';
import { Download } from 'lucide-react';
import { errorMessage, native } from '../../services/native';
import { Spinner } from '../../components/ui/Primitives';

export function LauncherUpdates({ offline }: { offline: boolean }) {
  const [attempt, setAttempt] = useState(0);
  const [version, setVersion] = useState<string | null>(null);
  const [checking, setChecking] = useState(false);
  const [error, setError] = useState('');

  useEffect(() => {
    if (offline) return;
    let active = true;
    setChecking(true);
    setError('');
    void native.checkUpdate()
      .then((value) => { if (active) setVersion(value); })
      .catch((reason) => { if (active) { setVersion(null); setError(errorMessage(reason)); } })
      .finally(() => { if (active) setChecking(false); });
    return () => { active = false; };
  }, [attempt, offline]);

  return <div className="setting-row">
    <div><strong>Launcher updates</strong>
      <small aria-live="polite">{offline ? 'Go online to check for updates.' : checking ? 'Checking GitHub…' : error || (version ? `Rocket Launcher v${version} is available.` : 'You’re up to date.')}</small>
    </div>
    <div className="inline-actions">
      <button type="button" className="button secondary small-button" disabled={checking || offline} onClick={() => setAttempt(attempt + 1)}>{checking && !offline && <Spinner size={14} />} Check for updates</button>
      {version && !offline && <button type="button" className="button primary small-button" onClick={() => void native.openRelease(version).catch((reason) => setError(errorMessage(reason)))}><Download size={14} /> Open release</button>}
    </div>
  </div>;
}
