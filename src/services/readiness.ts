import type { Readiness } from '../types';

export const readinessLabel: Record<Readiness, string> = {
  checking: 'Checking local files',
  requires_download: 'Installation required',
  missing_files: 'Repair required',
  downloading: 'Downloading',
  installing: 'Installing',
  java_required: 'Compatible Java required',
  authentication_required: 'Authentication required',
  ready: 'Ready to launch',
  launching: 'Launching',
  running: 'Running',
  error: 'Needs attention',
};
