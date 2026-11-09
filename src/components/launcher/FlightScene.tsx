import type { Readiness } from '../../types';

export function FlightScene({ readiness }: { readiness?: Readiness }) {
  const phase = readiness === 'launching' ? 'ignition'
    : readiness === 'running' ? 'flight'
    : readiness === 'downloading' || readiness === 'installing' || readiness === 'checking' ? 'prep'
    : readiness === 'error' ? 'alert' : 'standby';
  const caption = phase === 'ignition' ? 'Liftoff'
    : phase === 'flight' ? 'In flight'
    : phase === 'prep' ? 'Preflight'
    : phase === 'alert' ? 'Check status' : 'On the pad';

  return <div className="flight-scene" data-phase={phase} aria-hidden="true">
    <img className="play-scenery" src="/herobanner.png" alt="" fetchPriority="high" />
    <div className="flight-crosshair" />
    <div className="flight-telemetry">
      <div className="flight-viewport">
        <svg className="flight-trail" width="44" height="44" viewBox="0 0 22 22" shapeRendering="crispEdges">
          <path fill="#709deb" d="M5 16h2v2H5zm4-4h2v2H9zm4-4h2v2h-2z" />
          <path fill="#a6c4f9" d="M8 17h1v1H8zm7-8h1v1h-1z" />
        </svg>
        <img className="flight-rocket" src="/rocket.svg" alt="" width="34" height="34" />
      </div>
      <div className="flight-readout">
        <span>FLIGHT STATUS</span>
        <strong><i />{caption}</strong>
      </div>
    </div>
  </div>;
}
