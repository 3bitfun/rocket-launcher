import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { InstanceEditor } from '../components/instances/InstanceEditor';
import { QuickSearch } from '../components/layout/QuickSearch';
import { Downloads } from '../pages/Downloads/Downloads';
import { LaunchPad } from '../pages/LaunchPad/LaunchPad';
import { ModrinthBrowser } from '../pages/Mods/ModrinthBrowser';
import { Settings } from '../pages/Settings/Settings';
import { errorMessage, native } from '../services/native';
import { snapshot, catalog } from './fixtures';
import type { Instance } from '../types';

it('opens a matching favorite instance from quick search with the keyboard', async () => {
  const favorite: Instance = {
    id: 'favorite', name: 'Survival', version: '1.21.1', versionType: 'release', loader: 'fabric',
    loaderVersion: '0.20.1', gameDirectory: 'C:\\test', ramMb: 4096, javaPath: null,
    createdAt: 0, updatedAt: 0, lastSelectedAt: 0, readiness: 'ready', favorite: true,
  };
  const recent = { ...favorite, id: 'recent', name: 'Creative', lastSelectedAt: 10, favorite: false };
  const onManage = vi.fn();
  render(<QuickSearch instances={[recent, favorite]} pages={[{ id: 'settings', title: 'Settings' }]}
    onManage={onManage} onNavigate={vi.fn()} onClose={vi.fn()} />);
  const user = userEvent.setup();
  await user.type(screen.getByRole('textbox', { name: 'Search launcher' }), 'fabric 1.21{Enter}');
  expect(onManage).toHaveBeenCalledWith(favorite);
  expect(screen.queryByRole('button', { name: 'Settings' })).not.toBeInTheDocument();
});

describe('instance editor', () => {
  it('keeps a saved Fabric loader when the loader catalogue cannot be reached', async () => {
    const instance: Instance = {
      id: 'fabric-test', name: 'Fabric world', version: '1.21.1', versionType: 'release',
      loader: 'fabric', loaderVersion: '0.20.1', gameDirectory: 'C:\\test',
      ramMb: 4096, javaPath: null, createdAt: 0, updatedAt: 0, lastSelectedAt: 0, readiness: 'ready',
    };
    const request = vi.spyOn(native, 'fabricLoaders').mockRejectedValue(new Error('Fabric unavailable'));
    try {
      const save = vi.fn().mockResolvedValue(true);
      render(<InstanceEditor instance={instance} settings={snapshot.settings} catalog={catalog} fetching={false} catalogError={null} busy={false} onRefresh={vi.fn()} onSave={save} onClose={vi.fn()} />);
      await screen.findByText('Fabric unavailable');
      expect(screen.getByLabelText('Fabric loader')).toHaveValue('0.20.1');
      await userEvent.click(screen.getByRole('button', { name: 'Save changes' }));
      expect(save).toHaveBeenCalledWith(expect.objectContaining({ loader: 'fabric', loaderVersion: '0.20.1' }));
    } finally {
      request.mockRestore();
    }
  });

  it('uses the real catalogue result and submits a typed instance configuration', async () => {
    const save = vi.fn().mockResolvedValue(true);
    const close = vi.fn();
    render(<InstanceEditor settings={snapshot.settings} catalog={catalog} fetching={false} catalogError={null} busy={false} onRefresh={vi.fn()} onSave={save} onClose={close} />);
    const user = userEvent.setup();
    expect(screen.queryByRole('option', { name: /24w33a/ })).not.toBeInTheDocument();
    await user.type(screen.getByLabelText('Instance name'), 'Sunday survival');
    await user.click(screen.getByLabelText('Include snapshots'));
    await user.selectOptions(screen.getByLabelText('Minecraft version'), '24w33a');
    expect(screen.getByRole('button', { name: 'Create Rocket Pack instance' })).toBeDisabled();
    await user.click(screen.getByRole('button', { name: /Vanilla/ }));
    await user.click(screen.getByRole('button', { name: 'Create instance' }));
    expect(save).toHaveBeenCalledWith({ name: 'Sunday survival', version: '24w33a', loader: 'vanilla', loaderVersion: null, ramMb: 4096, javaPath: null, parentDirectory: 'C:\\test\\instances' });
    await waitFor(() => expect(close).toHaveBeenCalled());
  });

  it('keeps creation unavailable without a fetched or cached version', () => {
    render(<InstanceEditor settings={snapshot.settings} catalog={null} fetching={false} catalogError="Mojang unavailable" busy={false} onRefresh={vi.fn()} onSave={vi.fn()} onClose={vi.fn()} />);
    expect(screen.getByRole('button', { name: 'Create Rocket Pack instance' })).toBeDisabled();
    expect(screen.getByText('Mojang unavailable')).toBeInTheDocument();
  });

  it('keeps the editor open when native persistence fails', async () => {
    const close = vi.fn();
    const save = vi.fn().mockResolvedValue(false);
    render(<InstanceEditor settings={snapshot.settings} catalog={catalog} fetching={false} catalogError={null} busy={false} onRefresh={vi.fn()} onSave={save} onClose={close} />);
    const user = userEvent.setup();
    await user.type(screen.getByLabelText('Instance name'), 'Keep my draft');
    await user.click(screen.getByRole('button', { name: 'Create Rocket Pack instance' }));
    expect(save).toHaveBeenCalledWith(expect.objectContaining({ loader: 'fabric', loaderVersion: null }), expect.any(Function));
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

it('offers installation, then directs players to profile setup', async () => {
  const profile: Instance = { id: 'test', name: 'Test World', version: '1.21.1', versionType: 'release', loader: 'vanilla', loaderVersion: null, gameDirectory: 'C:\\test', ramMb: 4096, javaPath: null, createdAt: 0, updatedAt: 0, lastSelectedAt: 0, readiness: 'requires_download' };
  const onInstall = vi.fn();
  const onIgnite = vi.fn();
  const props = { catalog, busy: false, onCreate: vi.fn(), onEdit: vi.fn(), onSelect: vi.fn(), onNavigate: vi.fn(), onInstall, onIgnite, onCheck: vi.fn() };
  const selected = { ...snapshot, activeInstanceId: profile.id, instances: [profile] };
  const { rerender } = render(<LaunchPad {...props} snapshot={selected} />);
  await userEvent.click(screen.getByRole('button', { name: 'Install Minecraft' }));
  expect(onInstall).toHaveBeenCalledWith(profile);
  rerender(<LaunchPad {...props} snapshot={{ ...selected, instances: [{ ...profile, readiness: 'authentication_required' }] }} status={{ instanceId: profile.id, readiness: 'authentication_required', message: 'Files verified; sign-in required.', exitCode: null }} />);
  await userEvent.click(screen.getByRole('button', { name: 'Set up a player profile' }));
  expect(props.onNavigate).toHaveBeenCalledWith('settings');
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

it('installs the chosen Modrinth version into the selected Fabric instance', async () => {
  const instance: Instance = {
    id: 'fabric-test', name: 'Fabric world', version: '1.21.1', versionType: 'release',
    loader: 'fabric', loaderVersion: '0.20.1', gameDirectory: 'C:\\test',
    ramMb: 4096, javaPath: null, createdAt: 0, updatedAt: 0, lastSelectedAt: 0, readiness: 'ready',
  };
  const search = vi.spyOn(native, 'searchMods').mockResolvedValue({
    totalHits: 1,
    hits: [{ projectId: 'Project1', title: 'Example Mod', description: 'A test mod', author: 'Test author', downloads: 5 }],
  });
  const versions = vi.spyOn(native, 'modVersions').mockResolvedValue([
    { id: 'Version1', versionNumber: '1.1', versionType: 'release', fileName: 'example-1.1.jar', requiredDependencies: [] },
    { id: 'Version2', versionNumber: '1.0', versionType: 'release', fileName: 'example-1.0.jar', requiredDependencies: [] },
  ]);
  const install = vi.spyOn(native, 'installMod').mockResolvedValue('example-1.0.jar');
  const gallery = vi.spyOn(native, 'modrinthGallery').mockResolvedValue(['https://cdn.modrinth.com/data/Project1/gallery.png']);
  const curseSearch = vi.spyOn(native, 'searchCurseForge').mockResolvedValue({
    totalHits: 1,
    hits: [{ projectId: '42', title: 'CurseForge Example', description: 'Another mod', author: 'Test author', downloads: 10 }],
  });
  const keySaved = vi.spyOn(native, 'curseForgeKeySaved').mockResolvedValue(true);
  const refresh = vi.fn();
  try {
    render(<ModrinthBrowser instance={instance} onInstalled={refresh} />);
    const user = userEvent.setup();
    await user.type(screen.getByLabelText('Search mods'), 'example');
    await user.click(screen.getByRole('button', { name: /^Search$/ }));
    await waitFor(() => expect(search).toHaveBeenLastCalledWith(instance.id, 'example', 0, undefined));
    await user.selectOptions(screen.getByLabelText('Sort by'), 'updated');
    await waitFor(() => expect(search).toHaveBeenLastCalledWith(instance.id, 'example', 0, 'updated'));
    await user.click(await screen.findByRole('button', { name: 'View versions of Example Mod' }));
    expect(versions).toHaveBeenCalledWith(instance.id, 'Project1');
    expect(await screen.findByRole('img', { name: 'Project preview for Example Mod' }))
      .toHaveAttribute('src', 'https://cdn.modrinth.com/data/Project1/gallery.png');
    await user.selectOptions(await screen.findByLabelText('Compatible version'), 'Version2');
    await user.click(screen.getByRole('button', { name: 'Install mod' }));
    expect(install).toHaveBeenCalledWith(instance.id, 'Project1', 'Version2');
    expect(await screen.findByRole('status')).toHaveTextContent('Installed example-1.0.jar');
    expect(refresh).toHaveBeenCalledOnce();
    await user.click(screen.getByRole('button', { name: /^CurseForge$/ }));
    expect(await screen.findByText('CurseForge Example')).toBeInTheDocument();
    expect(curseSearch).toHaveBeenCalledWith(instance.id, 'example', 0, undefined);
    expect(screen.queryByText('Example Mod')).not.toBeInTheDocument();
  } finally {
    search.mockRestore();
    versions.mockRestore();
    install.mockRestore();
    gallery.mockRestore();
    curseSearch.mockRestore();
    keySaved.mockRestore();
  }
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

it('creates and selects a chosen offline username', async () => {
  const account = { id: 'offline-id', name: 'RocketPlayer', type: 'offline' as const };
  const accounts = vi.spyOn(native, 'accounts').mockResolvedValue({ profiles: [], activeId: null, offline: false });
  const create = vi.spyOn(native, 'createOfflineAccount').mockResolvedValue({ profiles: [account], activeId: account.id, offline: false });
  const updates = vi.spyOn(native, 'checkUpdate').mockResolvedValue(null);
  const changed = vi.fn();
  try {
    render(<Settings snapshot={snapshot} busy={false} onSave={vi.fn()} onDirtyChange={vi.fn()} onAuthStateChanged={changed} />);
    await screen.findByText('No Minecraft account connected');
    const user = userEvent.setup();
    await user.type(screen.getByLabelText('New offline username'), 'RocketPlayer');
    await user.click(screen.getByRole('button', { name: 'Use offline username' }));
    expect(create).toHaveBeenCalledWith('RocketPlayer');
    expect(await screen.findByText('Offline · RocketPlayer')).toBeInTheDocument();
    expect(changed).toHaveBeenCalledWith(expect.objectContaining({ activeId: account.id }));
  } finally {
    accounts.mockRestore();
    create.mockRestore();
    updates.mockRestore();
  }
});

it('preserves actionable Rust error messages', () => {
  expect(errorMessage({ code: 'FILESYSTEM', message: 'Access denied to the selected directory.' })).toBe('Access denied to the selected directory.');
  expect(errorMessage(new Error('Connection refused'))).toBe('Connection refused');
});
