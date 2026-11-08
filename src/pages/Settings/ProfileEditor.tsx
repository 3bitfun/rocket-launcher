import { useEffect, useState, type PointerEvent } from 'react';
import { errorMessage, native } from '../../services/native';
import type { AccountsSummary, ProfileAppearance } from '../../types';

type Profile = AccountsSummary['profiles'][number];
type UV = [number, number];
const blank: ProfileAppearance = { skin: null, cape: null, slim: false };
const step = 5;

function Cube({ width, height, depth, at, uv, texture, sheet = [64, 64], layer = 0, color, flip = false }: {
  width: number; height: number; depth: number; at: [number, number, number]; uv: UV;
  texture: string | null; sheet?: UV; layer?: number; color?: string; flip?: boolean;
}) {
  const [u, v] = uv;
  const w = width * step + layer * 2, h = height * step + layer * 2, d = depth * step + layer * 2;
  const faces = [
    [width, height, u + depth, v + depth, w, h, `translateZ(${d / 2}px)`],
    [width, height, u + 2 * depth + width, v + depth, w, h, `rotateY(180deg) translateZ(${d / 2}px)`],
    [depth, height, u + depth + width, v + depth, d, h, `rotateY(90deg) translateZ(${w / 2}px)`],
    [depth, height, u, v + depth, d, h, `rotateY(-90deg) translateZ(${w / 2}px)`],
    [width, depth, u + depth, v, w, d, `rotateX(90deg) translateZ(${h / 2}px)`],
    [width, depth, u + depth + width, v, w, d, `rotateX(-90deg) translateZ(${h / 2}px)`],
  ] as const;
  return <div className="skin-cube" style={{ transform: `translate3d(${at[0] * step}px,${at[1] * step}px,${at[2] * step}px)${flip ? ' scaleX(-1)' : ''}` }}>
    {faces.map(([tw, th, sx, sy, fw, fh, transform], index) => <div className="skin-face" key={index} style={{
      width: fw, height: fh, marginLeft: -fw / 2, marginTop: -fh / 2, transform,
      backgroundColor: texture ? undefined : color,
      backgroundImage: texture ? `url("${texture}")` : undefined,
      backgroundSize: `${sheet[0] * fw / tw}px ${sheet[1] * fh / th}px`,
      backgroundPosition: `${-sx * fw / tw}px ${-sy * fh / th}px`,
      filter: `brightness(${[1, .81, .83, .89, 1.12, .66][index]})`,
    }} />)}
  </div>;
}

function allowedTexture(url: string | null) {
  return url && (/^https:\/\/textures\.minecraft\.net\/texture\/[a-f0-9]+$/i.test(url)
    || /^data:image\/png;base64,[A-Za-z0-9+/=]+$/.test(url)) ? url : null;
}

function SkinViewer({ appearance }: { appearance: ProfileAppearance }) {
  const [camera, setCamera] = useState({ yaw: -24, pitch: -9, zoom: 1 });
  const [idle, setIdle] = useState(false);
  const [layers, setLayers] = useState(true);
  const [cape, setCape] = useState(true);
  const [image, setImage] = useState<{ url: string; height: number }>();
  const skin = allowedTexture(appearance.skin);
  useEffect(() => {
    if (!skin) return;
    const texture = new Image();
    texture.onload = () => { if (texture.width === 64 && [32, 64].includes(texture.height)) setImage({ url: skin, height: texture.height }); };
    texture.src = skin;
    return () => { texture.onload = null; };
  }, [skin]);
  const texture = image?.url === skin ? skin : null;
  const legacy = texture !== null && image?.height === 32;
  const arm = appearance.slim && !legacy ? 3 : 4;
  const capeTexture = allowedTexture(appearance.cape);
  function rotate(event: PointerEvent<HTMLDivElement>) {
    if (!event.currentTarget.hasPointerCapture(event.pointerId)) return;
    setCamera((value) => ({ ...value, yaw: value.yaw + event.movementX, pitch: Math.max(-75, Math.min(75, value.pitch - event.movementY)) }));
  }
  const parts = [
    { size: [8, 8, 8], at: [0, -12, 0], uv: [0, 0], overlay: [32, 0], color: '#b77a54' },
    { size: [8, 12, 4], at: [0, -2, 0], uv: [16, 16], overlay: [16, 32], color: appearance.slim ? '#649457' : '#328d97' },
    { size: [arm, 12, 4], at: [-4 - arm / 2, -2, 0], uv: [40, 16], overlay: [40, 32], color: '#b77a54' },
    { size: [arm, 12, 4], at: [4 + arm / 2, -2, 0], uv: legacy ? [40, 16] : [32, 48], overlay: [48, 48], color: '#b77a54', flip: legacy },
    { size: [4, 12, 4], at: [-2, 10, 0], uv: [0, 16], overlay: [0, 32], color: '#454b86' },
    { size: [4, 12, 4], at: [2, 10, 0], uv: legacy ? [0, 16] : [16, 48], overlay: [0, 48], color: '#454b86', flip: legacy },
  ] as const;
  return <div className="skin-viewer">
    <div className="skin-stage" onPointerDown={(event) => { if (event.button === 0) event.currentTarget.setPointerCapture(event.pointerId); }} onPointerMove={rotate}>
      <div className="skin-camera" style={{ transform: `scale(${camera.zoom}) rotateX(${camera.pitch}deg) rotateY(${camera.yaw}deg)` }}>
        <div className={idle ? 'skin-model is-idle' : 'skin-model'}>
          {parts.map(({ size: [width, height, depth], at, uv, overlay, color, ...part }, index) => {
            const props = { width, height, depth, at: [...at] as [number, number, number], sheet: [64, legacy ? 32 : 64] as UV, texture };
            return <div className="skin-part" key={index}>
              <Cube {...props} uv={[uv[0], uv[1]]} color={color} flip={'flip' in part && part.flip} />
              {texture && layers && (!legacy || index === 0) && <Cube {...props} uv={[...overlay]} layer={index === 0 ? 2 : 1.3} />}
            </div>;
          })}
          {cape && capeTexture && <Cube width={10} height={16} depth={1} at={[0, 0, -3]} uv={[0, 0]} texture={capeTexture} sheet={[64, 32]} />}
        </div>
      </div>
    </div>
    <div className="inline-actions skin-controls">
      <button type="button" className="button secondary small-button" aria-label="Rotate left" onClick={() => setCamera((v) => ({ ...v, yaw: v.yaw - 30 }))}>←</button>
      <button type="button" className="button secondary small-button" aria-label="Rotate right" onClick={() => setCamera((v) => ({ ...v, yaw: v.yaw + 30 }))}>→</button>
      <label className="skin-zoom">Zoom <input type="range" min=".65" max="1.4" step=".05" value={camera.zoom} onChange={(e) => setCamera({ ...camera, zoom: +e.target.value })} /></label>
      <button type="button" className="button secondary small-button" onClick={() => setCamera({ yaw: -24, pitch: -9, zoom: 1 })}>Reset view</button>
      <label><input type="checkbox" checked={idle} onChange={(e) => setIdle(e.target.checked)} /> Idle</label>
      <label><input type="checkbox" checked={layers} onChange={(e) => setLayers(e.target.checked)} /> Skin layers</label>
      {capeTexture && <label><input type="checkbox" checked={cape} onChange={(e) => setCape(e.target.checked)} /> Cape</label>}
    </div>
  </div>;
}

export function ProfileEditor({ profile, disabled, offline, onSaved }: { profile: Profile; disabled: boolean; offline: boolean; onSaved: (accounts: AccountsSummary) => void }) {
  const [name, setName] = useState(profile.name);
  const [appearance, setAppearance] = useState(profile.appearance ?? blank);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState('');
  const [error, setError] = useState('');
  useEffect(() => {
    if (offline || profile.type !== 'microsoft') return;
    let active = true;
    void native.profileAppearance(profile.id).then((value) => { if (active) setAppearance(value); }).catch(() => {});
    return () => { active = false; };
  }, [offline, profile.id, profile.type]);
  async function upload(file?: File) {
    if (!file) return;
    setError('');
    setMessage('');
    setBusy(true);
    try {
      if (file.type !== 'image/png' || file.size > 65536) throw new Error('Choose a PNG under 64 KiB.');
      const bitmap = await createImageBitmap(file);
      const valid = bitmap.width === 64 && [32, 64].includes(bitmap.height);
      const legacy = bitmap.height === 32;
      bitmap.close();
      if (!valid) throw new Error('A Minecraft skin must be 64×64 or 64×32 pixels.');
      const skin = await new Promise<string>((resolve, reject) => {
        const reader = new FileReader();
        reader.onload = () => resolve(String(reader.result));
        reader.onerror = () => reject(new Error('Could not read the PNG.'));
        reader.readAsDataURL(file);
      });
      setAppearance((value) => ({ ...value, skin, slim: !legacy && value.slim }));
    } catch (reason) { setError(errorMessage(reason)); }
    finally { setBusy(false); }
  }
  async function save() {
    setBusy(true);
    setError('');
    setMessage('');
    try {
      const result = await native.editOfflineProfile(profile.id, name, appearance.skin, appearance.slim);
      onSaved(result);
      setMessage('Profile saved.');
    } catch (reason) { setError(errorMessage(reason)); }
    finally { setBusy(false); }
  }
  return <div className="profile-editor">
    <SkinViewer appearance={offline && profile.type === 'microsoft' ? blank : appearance} />
    <div className="profile-fields">
      <h3>Player appearance</h3>
      {profile.type === 'offline' ? <fieldset disabled={disabled || busy} className="editor-fields">
        <label className="field"><span>Username</span><input maxLength={16} value={name} onChange={(e) => setName(e.target.value)} /></label>
        {name !== profile.name && <small>Changing your name changes your offline player ID. Some worlds and servers may treat it as a new player.</small>}
        <label className="field"><span>Model</span><select value={appearance.slim ? 'alex' : 'steve'} onChange={(e) => setAppearance({ ...appearance, slim: e.target.value === 'alex' })}><option value="steve">Steve · classic</option><option value="alex">Alex · slim</option></select></label>
        <label className="field"><span>Custom skin PNG</span><input type="file" accept="image/png,.png" onChange={(e) => { const file = e.currentTarget.files?.[0]; e.currentTarget.value = ''; void upload(file); }} /></label>
        <small>Local preview only. Minecraft’s offline session does not apply launcher skin files.</small>
        <div className="inline-actions"><button type="button" className="button secondary small-button" onClick={() => setAppearance(blank)}>Reset skin</button><button type="button" className="button primary small-button" onClick={() => void save()} disabled={!/^[A-Za-z0-9_]{3,16}$/.test(name)}>Save profile</button></div>
      </fieldset> : <p>{appearance.skin ? 'This is the skin from your Minecraft account.' : 'Sign in again to refresh the skin and cape from your Minecraft account.'}</p>}
      {error && <p role="alert" className="form-error">{error}</p>}
      {message && <p role="status">{message}</p>}
    </div>
  </div>;
}
