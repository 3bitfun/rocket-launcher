import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { InstanceEditor } from '../components/instances/InstanceEditor';
import { Downloads } from '../pages/Downloads/Downloads';
import { LaunchPad } from '../pages/LaunchPad/LaunchPad';
import { Settings } from '../pages/Settings/Settings';
import { errorMessage } from '../services/native';
import { snapshot, catalog } from './fixtures';
import type { Instance } from '../types';

describe('instance editor', () => {
  it('uses the real catalogue result and submits a typed instance configuration', async () => {
    const save = vi.fn().mockResolvedValue(true);
    const close = vi.fn();
    render(<InstanceEditor settings={snapshot.settings} catalog={catalog} fetching={false} catalogError={null} busy={false} onRefresh={vi.fn()} onSave={save} onClose={close} />);
    const user = userEvent.setup();
    expect(screen.queryByRole('option', { name: /24w33a/ })).not.toBeInTheDocument();
    await user.type(screen.getByLabelText('Instance name'), 'Sunday survival');
    await user.click(screen.getByLabelText('Include snapshots'));
    await user.selectOptions(screen.getByLabelText('Minecraft version'), '24w33a');
    await user.click(screen.getByRole('button', { name: 'Create instance' }));
    expect(save).toHaveBeenCalledWith({ name: 'Sunday survival', version: '24w33a', ramMb: 4096, javaPath: null, parentDirectory: 'C:\\test\\instances' });
    await waitFor(() => expect(close).toHaveBeenCalled());
  });

  it('keeps creation unavailable without a fetched or cached version', () => {
    render(<InstanceEditor settings={snapshot.settings} catalog={null} fetching={false} catalogError="Mojang unavailable" busy={false} onRefresh={vi.fn()} onSave={vi.fn()} onClose={vi.fn()} />);
    expect(screen.getByRole('button', { name: 'Create instance' })).toBeDisabled();
    expect(screen.getByText('Mojang unavailable')).toBeInTheDocument();
  });

  it('keeps the editor open when native persistence fails', async () => {
    const close = vi.fn();
    render(<InstanceEditor settings={snapshot.settings} catalog={catalog} fetching={false} catalogError={null} busy={false} onRefresh={vi.fn()} onSave={vi.fn().mockResolvedValue(false)} onClose={close} />);
    const user = userEvent.setup();
    await user.type(screen.getByLabelText('Instance name'), 'Keep my draft');
    await user.click(screen.getByRole('button', { name: 'Create instance' }));
    expect(close).not.toHaveBeenCalled();
    expect(screen.getByLabelText('Instance name')).toHaveValue('Keep my draft');
  });

  it('renders native save errors inside the dialog top layer', () => {
    render(<InstanceEditor settings={snapshot.settings} catalog={catalog} fetching={false} catalogError={null} saveError="Could not create the instance game directory: Access denied" busy={false} onRefresh={vi.fn()} onSave={vi.fn()} onClose={vi.fn()} />);
    const error = screen.getByRole('alert');
    expect(error).toHaveTextContent('Access denied');
    expect(screen.getByRole('dialog')).toContainElement(error);
  });
});

it('shows actual catalogue failure and offers a working retry action', async () => {
  const retry = vi.fn();
  render(<Downloads catalog={null} error="Network request timed out" fetching={false} onRefresh={retry} />);
  expect(screen.getByRole('alert')).toHaveTextContent('Network request timed out');
  expect(screen.queryByRole('progressbar')).not.toBeInTheDocument();
  await userEvent.click(screen.getByRole('button', { name: 'Refresh catalogue' }));
  expect(retry).toHaveBeenCalledOnce();
});

it('offers installation, then keeps IGNITE disabled until authentication', async () => {
  const profile: Instance = { id: 'test', name: 'Test World', version: '1.21.1', versionType: 'release', loader: 'vanilla', gameDirectory: 'C:\\test', ramMb: 4096, javaPath: null, createdAt: 0, updatedAt: 0, lastSelectedAt: 0, readiness: 'requires_download' };
  const onInstall = vi.fn();
  const onIgnite = vi.fn();
  const props = { catalog, busy: false, onCreate: vi.fn(), onEdit: vi.fn(), onSelect: vi.fn(), onNavigate: vi.fn(), onInstall, onIgnite, onCheck: vi.fn() };
  const selected = { ...snapshot, activeInstanceId: profile.id, instances: [profile] };
  const { rerender } = render(<LaunchPad {...props} snapshot={selected} />);
  await userEvent.click(screen.getByRole('button', { name: 'Install Minecraft' }));
  expect(onInstall).toHaveBeenCalledWith(profile);
  rerender(<LaunchPad {...props} snapshot={{ ...selected, instances: [{ ...profile, readiness: 'authentication_required' }] }} status={{ instanceId: profile.id, readiness: 'authentication_required', message: 'Files verified; sign-in required.', exitCode: null }} />);
  expect(screen.getByRole('button', { name: 'IGNITE' })).toBeDisabled();
  expect(screen.getByText('Files verified; sign-in required.')).toBeInTheDocument();
  expect(onIgnite).not.toHaveBeenCalled();
});

it('shows measured download bytes and retries a failed installation', async () => {
  const onRetry = vi.fn();
  render(<Downloads catalog={null} error={null} fetching={false} onRefresh={vi.fn()} onRetry={onRetry} progress={{ instanceId: 'test', version: '1.21.1', stage: 'error', current: 'Installation failed', transferredBytes: 2097152, verifiedBytes: 1048576, totalBytes: 4194304, bytesPerSecond: 0, error: 'Wrong SHA-1' }} />);
  expect(screen.getByText('2.0 MiB transferred')).toBeInTheDocument();
  expect(screen.getByRole('alert')).toHaveTextContent('Wrong SHA-1');
  await userEvent.click(screen.getByRole('button', { name: 'Retry installation' }));
  expect(onRetry).toHaveBeenCalledOnce();
});

it('saves changed preferences and tracks unsaved state', async () => {
  const save = vi.fn().mockResolvedValue(true);
  const dirty = vi.fn();
  render(<Settings snapshot={snapshot} busy={false} onSave={save} onDirtyChange={dirty} />);
  const user = userEvent.setup();
  expect(screen.getByRole('button', { name: 'Save preferences' })).toBeDisabled();
  await user.click(screen.getByRole('button', { name: /^Light$/ }));
  await user.click(screen.getByRole('switch', { name: /Show snapshots/ }));
  await user.click(screen.getByRole('button', { name: 'Save preferences' }));
  expect(save).toHaveBeenCalledWith(expect.objectContaining({ theme: 'light', showSnapshots: true }));
  expect(dirty).toHaveBeenCalledWith(true);
  await waitFor(() => expect(dirty).toHaveBeenLastCalledWith(false));
});

it('preserves actionable Rust error messages', () => {
  expect(errorMessage({ code: 'FILESYSTEM', message: 'Access denied to the selected directory.' })).toBe('Access denied to the selected directory.');
  expect(errorMessage(new Error('Connection refused'))).toBe('Connection refused');
});
