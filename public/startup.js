// Preserve an actionable screen if the module graph fails before React mounts.
window.addEventListener('error', (event) => {
  const root = document.getElementById('root');
  if (!root || root.hasChildNodes()) return;
  const screen = document.createElement('div');
  screen.className = 'startup-screen';
  screen.setAttribute('role', 'alert');
  const title = document.createElement('h1');
  title.textContent = 'Rocket Launcher could not load';
  const detail = document.createElement('p');
  detail.textContent = event.message || 'A required application resource failed to load. Restart the launcher.';
  screen.append(title, detail);
  root.append(screen);
});
