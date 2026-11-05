import { useId } from 'react';

/** Small, locally rendered voxel landscape. No downloaded artwork or tracking. */
export function Landscape({ small = false }: { small?: boolean }) {
  const id = useId().replaceAll(':', '');
  return <svg className={small ? 'landscape landscape-small' : 'landscape'} viewBox="0 0 680 400" aria-hidden="true" preserveAspectRatio="xMidYMid slice">
    <defs><linearGradient id={`sky${id}`} x2="0" y2="1"><stop stopColor="#1d2c31" /><stop offset="1" stopColor="#152224" /></linearGradient><linearGradient id={`ground${id}`} x2="0.3" y2="1"><stop stopColor="#52786b" /><stop offset="1" stopColor="#233c39" /></linearGradient></defs>
    <path fill={`url(#sky${id})`} d="M0 0h680v400H0z" />
    <g fill="#bed4cf" opacity=".35"><path d="M85 70h3v3h-3zM240 42h3v3h-3zM442 78h3v3h-3zM595 49h3v3h-3zM510 151h3v3h-3zM335 112h3v3h-3zM140 165h3v3h-3z" /></g>
    <circle cx="494" cy="93" r="45" fill="#f5ae7d" /><circle cx="478" cy="82" r="47" fill="#253736" />
    <path d="m0 239 80-45 73 17 88-68 87 53 73-27 96 60 84-38 99 40v169H0Z" fill="#304742" />
    <path d="m0 295 91-54 102 39 82-77 93 50 118-44 103 76 91-30v145H0Z" fill="#3d5b50" />
    <path d="m165 293 158-91 182 106-158 91Z" fill={`url(#ground${id})`} />
    <path d="m165 293 182 106v99L165 392Z" fill="#263d35" /><path d="m347 399 158-91v94l-158 96Z" fill="#1b302d" />
    <path d="m220 280 102-59 124 73-102 59Z" fill="#638879" /><path d="m220 280 124 73v17l-124-72Z" fill="#466454" /><path d="m344 353 102-59v17l-102 59Z" fill="#354e43" />
    <path d="m309 266 26-15 26 15-26 15Z" fill="#b9aa8a" /><path d="m309 266 26 15v35l-26-15Z" fill="#8a7b60" /><path d="m335 281 26-15v35l-26 15Z" fill="#6f6753" />
    <path d="m386 267 12-7v-71l-12 7Z" fill="#726853" /><path d="m398 260 10 6v-71l-10-6Z" fill="#4e4d3b" />
    <path d="m354 183 44-26 45 26-45 26Z" fill="#7b9a70" /><path d="m354 183 44 26v39l-44-26Z" fill="#4b7052" /><path d="m398 209 45-26v39l-45 26Z" fill="#3a5a42" />
    <path d="m365 159 33-19 34 19-34 20Z" fill="#8fa47a" /><path d="m365 159 33 20v29l-33-19Z" fill="#5a7f57" /><path d="m398 179 34-20v30l-34 19Z" fill="#476748" />
    <path d="m251 301 6-4v-10l-6 4Zm8 1 6-4v-15l-6 4Zm155 21 6-4v-10l-6 4Z" fill="#a4b781" />
    <path d="m273 324 5 3v-13l-5-3Z" fill="#e4a776" />
    <path d="m0 369 96-56 83 48 63 39H0Zm512 31 91-70 77 34v36Z" fill="#162c29" opacity=".8" />
  </svg>;
}
