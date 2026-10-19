// Synthetic content for the specimen: Jane Q. Doe (fixtures/banks/jane-doe.v1.json), never a real person.
// Pages stand in for the renderer's SVG: an SVG sheet (US Letter, 612×792) whose text flows in a foreignObject so
// the inserted keywords can carry the sheet's highlight. The specimen styles `.sheet`.

import type { MapPlace } from "$lib/ui";

const SHEET = (body: string) =>
  `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 612 792" width="612" height="792">` +
  `<foreignObject x="0" y="0" width="612" height="792">` +
  `<div xmlns="http://www.w3.org/1999/xhtml" class="sheet">${body}</div>` +
  `</foreignObject></svg>`;

const mark = (s: string) => `<span class="sheet__mark">${s}</span>`;

const HEAD = `
  <h1>Jane Q. Doe</h1>
  <div class="sheet__contact">jane.doe@example.com | (555) 010-0199 | linkedin.com/in/janeqdoe-example | github.com/janeqdoe-example</div>`;

const WORK = `
  <h2>WORK EXPERIENCE</h2>
  <div class="sheet__entry"><span>Acme Widgets Co., Springfield, IL</span><em>June 2025 – Sep 2025</em></div>
  <div class="sheet__sub">Software Engineering Intern</div>
  <ul>
    <li>Built ${mark("REST APIs")} in a Rust service that batches widget telemetry into Postgres, cutting ingest latency from 900 ms to 120 ms</li>
    <li>Wrote integration tests in GitHub Actions ${mark("CI/CD")} for the order API, catching 14 regressions before release</li>
    <li>Paired with the design team to ship a React dashboard that plant managers now use daily</li>
  </ul>
  <div class="sheet__entry"><span>Springfield Public Library, Springfield, IL</span><em>Summer 2023</em></div>
  <div class="sheet__sub">IT Assistant</div>
  <ul>
    <li>Imaged and deployed 60 patron laptops with a scripted setup, halving setup time per machine</li>
    <li>Answered help-desk tickets for staff and patrons, documenting fixes in a shared knowledge base</li>
  </ul>`;

const PROJECTS = `
  <h2>PROJECTS</h2>
  <div class="sheet__entry"><span>Tidepool</span><em>Jan 2025 – Present</em></div>
  <div class="sheet__sub">Creator</div>
  <ul>
    <li>Designed an offline-first notes app in ${mark("TypeScript")} and SQLite with conflict-free sync between devices</li>
    <li>Grew it to 300 weekly users through a public beta and weekly release notes</li>
  </ul>`;

const EDUCATION = `
  <h2>EDUCATION</h2>
  <div class="sheet__entry"><span>Springfield State University, Springfield, IL</span><em>Aug 2022 – May 2026</em></div>
  <div class="sheet__sub">B.S. Computer Science</div>`;

const SKILLS = `
  <h2>SKILLS</h2>
  <div><b>Languages:</b> Rust, TypeScript, Python, SQL</div>
  <div><b>Tools &amp; Technologies:</b> React, Postgres, Git, GitHub Actions, ${mark("Unit testing")}, ${mark("Code review")}</div>`;

/** One page that fits. */
export const FITS = [SHEET(HEAD + WORK + PROJECTS + EDUCATION + SKILLS)];

/** Two pages: over a one-page limit. */
export const OVER = [SHEET(HEAD + WORK + PROJECTS + WORK.replace("WORK EXPERIENCE", "LEADERSHIP")), SHEET(EDUCATION + SKILLS)];

/** A made-up company's logo as it would publish it: its own colors, a square mark. */
export const LOGO = `data:image/svg+xml,${encodeURIComponent(
  `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><rect width="64" height="64" rx="14" fill="#0f766e"/>` +
    `<path d="M18 46V18h7l14 17V18h7v28h-7L25 29v17z" fill="#fff"/></svg>`,
)}`;

/** A wide logo (a wordmark-like bar): scaled to fit the square, never cropped. */
export const WIDE_LOGO = `data:image/svg+xml,${encodeURIComponent(
  `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 120 40"><rect width="120" height="40" rx="6" fill="#c2410c"/>` +
    `<circle cx="20" cy="20" r="9" fill="#fff"/><rect x="36" y="15" width="70" height="10" rx="2" fill="#fff"/></svg>`,
)}`;

/** Places as `places` gives them (OpenStreetMap's Nominatim, boundaries simplified), for the map. */
export const PLANO = {
  name: "Plano",
  region: "Texas, United States",
  lat: 33.0137,
  lon: -96.6925,
  bbox: [-96.86, 32.9971, -96.6129, 33.1252],
  boundary: JSON.stringify({
    type: "Polygon",
    coordinates: [
      [
        [-96.86, 33.042], [-96.8532, 33.042], [-96.8525, 33.0261], [-96.8353, 33.0274], [-96.8435, 33.0165],
        [-96.7944, 33.0131], [-96.7689, 33.0001], [-96.7063, 33.004], [-96.6651, 32.9971], [-96.665, 33.0039],
        [-96.613, 33.005], [-96.6129, 33.0095], [-96.631, 33.0104], [-96.6413, 33.019], [-96.6393, 33.0523],
        [-96.6563, 33.053], [-96.6558, 33.0749], [-96.6853, 33.0709], [-96.7048, 33.0887], [-96.7367, 33.0861],
        [-96.7328, 33.1252], [-96.8524, 33.0825], [-96.8572, 33.0607], [-96.8428, 33.0562], [-96.8584, 33.0531],
        [-96.86, 33.042],
      ],
    ],
  }),
} satisfies MapPlace;

export const RIVERWOODS = {
  name: "Riverwoods",
  region: "Illinois, United States",
  lat: 42.1676,
  lon: -87.8971,
  bbox: [-87.9231, 42.1529, -87.8742, 42.1929],
  boundary: JSON.stringify({
    type: "Polygon",
    coordinates: [
      [
        [-87.9231, 42.1712], [-87.9137, 42.1659], [-87.9191, 42.1606], [-87.8834, 42.1529], [-87.883, 42.1666],
        [-87.8742, 42.1736], [-87.8866, 42.1929], [-87.9063, 42.1915], [-87.8991, 42.1857], [-87.9018, 42.175],
        [-87.9231, 42.1712],
      ],
    ],
  }),
} satisfies MapPlace;

/** A place Nominatim knows only as a point. */
export const MCLEAN = {
  name: "McLean",
  region: "Virginia, United States",
  lat: 38.9367,
  lon: -77.1858,
  bbox: [-77.2878, 38.8932, -77.1198, 38.9799],
  boundary: null,
} satisfies MapPlace;
