import { Check, Copy, Pencil, Trash2, Box, Puzzle, ArrowUpRight } from 'lucide-react';
import { motion } from 'motion/react';
import type { Instance } from '../../types';
import { loaderName } from '../../services/loaders';
import { formatRam } from '../ui/Primitives';
import { Landscape } from '../launcher/Landscape';

export function InstanceCard({ instance, active, busy, onSelect, onEdit, onDuplicate, onDelete, compact = false }: {
  instance: Instance; active: boolean; busy: boolean; onSelect: () => void; onEdit: () => void; onDuplicate?: () => void; onDelete?: () => void; compact?: boolean;
}) {
  return <motion.article layout className={`instance-card ${active ? 'selected' : ''} ${compact ? 'compact' : ''}`}>
    <button className="instance-select" onClick={onSelect} disabled={busy} aria-label={`Select ${instance.name}`} aria-pressed={active}>
      <div className="instance-art"><Landscape small /><span className="instance-loader">{instance.loader !== 'vanilla' ? <><Puzzle size={12} /> {loaderName(instance.loader)}</> : <><Box size={12} /> Vanilla</>}</span>{active && <span className="selected-check"><Check size={14} /></span>}</div>
      <div className="instance-details"><h3>{instance.name}</h3><p>Minecraft {instance.version} {instance.loaderVersion && <>· {loaderName(instance.loader)} {instance.loaderVersion}</>}<span>·</span>{formatRam(instance.ramMb)}</p><span className="readiness"><span className="status-dot" />{readinessLabel[instance.readiness]}</span></div>
    </button>
    <div className="instance-actions"><span>{active ? 'Selected instance' : `${loaderName(instance.loader)} · Java Edition`}</span><div>
      <button className="icon-button" onClick={onEdit} disabled={busy} aria-label={`Edit ${instance.name}`}>{compact ? <ArrowUpRight size={16} /> : <Pencil size={15} />}</button>
      {onDuplicate && <button className="icon-button" onClick={onDuplicate} disabled={busy} aria-label={`Duplicate ${instance.name}`} title="Duplicate settings into a new empty instance"><Copy size={15} /></button>}
      {onDelete && <button className="icon-button danger-hover" onClick={onDelete} disabled={busy} aria-label={`Remove ${instance.name}`}><Trash2 size={15} /></button>}
    </div></div>
  </motion.article>;
}
import { readinessLabel } from '../../services/readiness';
