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
  const stage = phase === 'flight' ? 2 : phase === 'ignition' ? 1 : 0;

  return <div className="flight-scene" data-phase={phase} aria-hidden="true">
    <img className="play-scenery" src="/herobanner.png" alt="" fetchPriority="high" />
    <div className="flight-field">
      <svg className="flight-grid" viewBox="0 0 280 160" fill="none" shapeRendering="crispEdges">
        <path d="M24 135h22v-16h23v-16h23V87h23V71h23V55h23V39h23V23h46" stroke="currentColor" strokeDasharray="3 5" />
        <path d="M2 20V2h18m240 0h18v18M2 140v18h18m240 0h18v-18" stroke="currentColor" />
        <path d="M221 55v8m-4-4h8M60 36v8m-4-4h8m162 75v8m-4-4h8M41 88v4m-2-2h4" stroke="currentColor" />
      </svg>
      <div className="flight-rocket-track">
        <img className="flight-rocket" src="/rocket.svg" alt="" width="84" height="84" />
        <svg className="flight-exhaust" viewBox="0 0 24 24" shapeRendering="crispEdges">
          <path fill="#ffc185" d="M13 2h8v4h-4v4h-4v4H9v4H5v-6h4V8h4z" />
          <path fill="#fff1c6" d="M13 6h4v4h-4zm-4 4h4v4H9z" />
          <path fill="#ff896a" d="M4 18h4v4H4zm-4-4h3v3H0z" />
        </svg>
      </div>
    </div>
    <div className="flight-telemetry" key={phase}>
      <div className="flight-readout"><span>FLIGHT STATUS</span><strong><i />{caption}</strong></div>
      <div className="flight-stages">
        {['PAD', 'LIFTOFF', 'FLIGHT'].map((name, index) => <span key={name} data-reached={index <= stage}>{name}</span>)}
      </div>
    </div>
  </div>;
}
