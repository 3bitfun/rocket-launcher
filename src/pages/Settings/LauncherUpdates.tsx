import { useEffect, useState } from 'react';
import { Download } from 'lucide-react';
import { errorMessage, native } from '../../services/native';
import { Spinner } from '../../components/ui/Primitives';

export function LauncherUpdates({ offline, blocked, onInstallingChange }: { offline: boolean; blocked: boolean; onInstallingChange: (installing: boolean) => void }) {
  const [attempt, setAttempt] = useState(0);
  const [version, setVersion] = useState<string | null>(null);
  const [checking, setChecking] = useState(false);
  const [error, setError] = useState('');
  const [progress, setProgress] = useState<number | null>(null);

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

  async function install() {
    if (!version || offline || blocked || checking || progress !== null) return;
    setError('');
    setProgress(0);
    onInstallingChange(true);
    try {
      await native.installUpdate(version, setProgress);
    } catch (reason) {
      setError(errorMessage(reason));
      setProgress(null);
      onInstallingChange(false);
    }
  }

  const message = progress === 100 ? 'Installing update. Rocket Launcher will reopen when it finishes.'
    : progress !== null ? `Downloading installer… ${progress}%`
    : offline ? 'Go online to check for updates.'
    : checking ? 'Checking GitHub…'
    : error || (version ? `Rocket Launcher v${version} is available. The launcher will restart after installing.` : 'You’re up to date.');

  return <div className="setting-row">
    <div><strong>Launcher updates</strong>
      <small aria-live="polite">{message}</small>
      {progress !== null && progress < 100 && <progress aria-label="Update download progress" value={progress} max={100} />}
    </div>
    <div className="inline-actions">
      <button type="button" className="button secondary small-button" disabled={checking || offline || progress !== null} onClick={() => setAttempt(attempt + 1)}>{checking && !offline && <Spinner size={14} />} Check for updates</button>
      {version && !offline && <button type="button" className="button primary small-button" disabled={blocked || checking || progress !== null} onClick={() => void install()}><Download size={14} /> Download and install</button>}
    </div>
  </div>;
}
