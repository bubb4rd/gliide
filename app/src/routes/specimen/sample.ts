// Synthetic content for the specimen: Jane Q. Doe (fixtures/banks/jane-doe.v1.json), never a real person.
// Pages stand in for the renderer's SVG: an SVG sheet (US Letter, 612×792) whose text flows in a foreignObject so
// the inserted keywords can carry the sheet's highlight. The specimen styles `.sheet`.

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
