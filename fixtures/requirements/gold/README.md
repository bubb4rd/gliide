# Gold requirements

Drafted 2026-11-16 by hand from the posting text; **awaiting the user's review**. Until reviewed, scores against
these files measure agreement with a draft, not correctness.

One `resume_core::Requirements` per posting in `fixtures/postings/`, the answer key that `ez eval read` grades every
Read route against (term recall and precision through `resume_core::matcher`, the priority-1 + required subset,
knockouts, seniority, years and degree). Each file fits `prompts/extract.schema.json` and the type, which
`crates/resume-llm/tests/requirements_fixtures.rs` checks with the other recorded outputs. The labels were read from
the postings only; `fixtures/requirements/sonnet/` was used for the JSON shape, not its judgments.

Revised 2026-11-17 after the first `ez eval` (`local/eval/2026-11-16`): ten terms every model left out were dropped
as the generic kind `prompts/extract.md` tells it to leave out: `software engineering` and `code quality` (acme),
`integration`, `automation` and `debug` (contoso), `documentation`, `debugging` and `code generation` (globex),
`solving problems` (initech), `troubleshoot` (vandelay). The domain terms they also left out (healthcare, education,
banking, trading applications) stay: the prompt now asks for the industry a posting wants experience in.

## Rules applied

The extract prompt (`prompts/extract.md`) is the rulebook; these are the readings it left open.

- **Terms** are the posting's own phrase, found word for word (checked case-sensitively). Aliases are only other
  forms of the same thing (acronym, spelling, plural, part of speech), never a broader, narrower or related skill, so
  that a model's term matches a gold term only when it names the same concept.
- **Required** means the qualifications list (Requirements, You Have, Must have, Qualifications). Duties and stack
  lists are `required: false`, as is anything the posting says it will teach.
- **Priority 1** is the required core stack only, at most a third of the list (most files have 3).
  **Priority 2** is the other required items, plus preferred items the posting stresses or repeats and the daily
  stack. **Priority 3** is nice-to-have or named in passing.
- **One-of lists** ("e.g. A, B", "A or B", "A and/or B", "such as A, B, or similar") are a `group` sharing
  `priority` and `required`. A single named example ("a frontend framework such as React") stands alone as
  required, since it is the only keyword an ATS can match; these are listed below for review. "X/Y" between
  languages ("JavaScript/TypeScript") is read as a one-of.
- **The 20-skill cap.** Several postings name more than 20 keywords. The list keeps the required ones first, then the
  most repeated; a one-of list may keep only some of its members (listed per posting below).
- **Soft skills** only from words that name a skill, at most 3, never priority 1. Person phrases ("Self-starter",
  "Problem solvers", "Clear communicators") give a soft skill only when the posting also names the skill itself.
- **Seniority**: title first, then years; `entry` when neither says. Students applying to full-time roles are
  `entry`.
- **Knockouts**: the posting's words, trimmed. Background checks, criminal-history reviews, start-date availability
  and stated physical requirements are `other`; application materials, coding challenges and "may require travel" are
  not knockouts. A remote role open nationwide has no `onsite` knockout, and a city in the location is not one unless
  the posting requires presence.
- **Degree** is set only when it is a requirement; `equivalent` is true only when experience or training may replace
  it ("or equivalent experience", "OR prior experience"), not for "or a related field".
- **Location** follows the prompt's short forms ("Remote (US)", "Hybrid - Austin"); it is not graded.

## Per posting: what to check

### acme-swe-robotics

- React stands alone as required priority 1 although the posting says "a frontend framework such as React".
- TypeScript and Python are a `language` group at priority 1 ("at least one modern language (e.g., …)").
- `software engineering` was dropped as generic (2026-11-17).
- Knockouts: one `authorization` (the visa line appears twice and is merged), one `onsite` (Tue–Thu).
- Degree: bachelor, `equivalent: true`.

### aperture-swe-ai-wearable

- Highest priority-1 share (6/20 = 0.30): JavaScript/TypeScript group, React/Angular/Vue group and Node.js. Angular
  and Vue are priority 1 only because group members share priority; lower the whole framework group to 2 if that
  reads wrong.
- Node.js required on its own ("Node.js or a similar backend framework").
- `relational or NoSQL databases` is one term with the parts as aliases (shared word).
- Of the web-performance examples, only `server-side rendering` is kept (it is also a nice-to-have); caching, lazy
  loading and image optimization are cut for the cap, as are accessibility, CI/CD, GraphQL, Stripe and Shopify.
- Seniority `mid` from "3+ years"; no degree; no knockouts.

### contoso-dev-integrations-ai

- PHP is `required: false` ("PHP experience also counts" reads as a plus, not a requirement).
- `AI or LLM-enabled development` kept whole (shared word); required priority 2.
- Oracle NetSuite and SQL are priority 3 ("a plus but not required").
- Seniority `entry`: no level or years, and the posting says the AI work is learned on the job.
- title_terms is only "Developer": "Integrations & AI" is the area, not the occupation.

### fabrikam-mobile-dev-1

- `mobile development` / `software development` are a `prior_experience` group, `required: false`, because a
  bachelor's degree can replace that experience; the degree is set with `equivalent: true` for the same reason.
- SDLC and Agile are a group ("and/or"), required.
- iOS and Android are priority 2 but not required: the qualifications only ask for "an interest" in them.
- No knockouts: "Some assignments may require travel … or lifting" is read as boilerplate, not a stated condition.

### globex-swe

- No front matter, so no hints; role is "2027 Software Engineer Program" and location "Chicago, IL and 5 more".
- Priority 1: AI development tools (repeated four times across duties and requirements), testing and
  troubleshooting (from "designing, building, testing, and troubleshooting software"). The posting is light on named
  skills; check whether these are the right three.
- `refactoring` is required but priority 3 (an example inside a requirement).
- Knockouts: `enrollment` (graduation window), `other` (February 2027 start), `authorization` (no sponsorship
  including OPT/CPT), `other` (criminal-history review). The required coding challenge is an application step and is
  left out.
- Degree: bachelor, six fields, `equivalent: false` ("5th-year Master's" is the alternative, not experience).

### hooli-fullstack-swe

- Go Lang and Typescript are a `language` group at priority 1 ("and/or"). Python, which the duties call the main
  language, is priority 2 and not required: the requirements never ask for it.
- React required priority 1 ("JavaScript frameworks such as React"), single example.
- Express and FastAPI are a `backend_framework` group; Pydantic stands alone as required.
- cloud platforms / distributed computing / Docker are a preferred `infrastructure` group.
- Seniority `entry`: no level or years, "work next to experienced engineers".
- Knockouts: background check, no visa sponsorship.

### initech-entry-swe

- Only one priority 1 (`software development`): the long technology list is "work across many technologies" in the
  duties, so every item is priority 3 and not required. Cut for the cap: RAG, prompt engineering, LangChain, Data
  Engineering, DevOps.
- Soft skill from "communication skills" (in the duties), required because the We Look For list asks for it in a
  person phrase; "solving problems" was dropped as generic (2026-11-17).
- Knockouts: graduation window, no sponsorship, and `onsite` for "Work within a hybrid model".
- Degree: bachelor (pursuing), `equivalent: false`.
- title_terms leaves out "Consultant" ("As an Initech Consultant").

### northwind-jr-swe

- React, Next.js, Phoenix are a `web_framework` group at priority 1 (the only required stack item).
- Elixir is priority 2 and never required ("You don't need to know Elixir yet — we'll teach you").
- Git required priority 2 rather than 1.
- Knockout: background check only. No degree, no years ("first year or two" describes the target, not a minimum).

### stark-ux-engineer

- Priority 1: HTML5, CSS3, JavaScript, CMS, accessibility.
- Three one-of requirements, each cut to some members for the cap: `frontend_stack` keeps TypeScript, React,
  Angular (drops component-based architecture, Razor, C#, .NET, ASP.NET Core); `site_improvement` keeps WCAG 2.2 AA,
  Section 508 (drops analytics, accessibility testing, performance tuning, Core Web Vitals); `sector` keeps
  healthcare, education (drops medical society, nonprofit, publishing).
- Also cut: source control, pull requests, code reviews, issue tracking, visual design, organization, prioritization.
- Knockouts: `onsite` (twice a week), `authorization`. The cover letter with salary expectations is not a knockout.
- Degree: bachelor, five fields, `equivalent: true`; years 3 → `mid`.

### vandelay-systems-automation

- Python and JavaScript are a `language` group ("Working Python or JavaScript").
- Podio, n8n, Zapier are a required `low_code` group at priority 2, although Podio is most of the daily work.
- React / Next.js are a preferred group at priority 2 ("a strong plus").
- Knockouts: `onsite` (five days), `other` background check, `other` "Able to lift up to 50 lbs". Check whether the
  lifting line belongs.
- title_terms adds "Systems Engineer" and "Automation Engineer" by the prompt's shared-word rule.
- Cut for the cap: password managers, AppFolio, Supabase, systems administration, scripting, integrations.

### wayne-ui-developer

- Seniority `entry` by the prompt's default (no level or years). For a trading firm this may read low; check.
- JavaScript/TypeScript read as a `language` group.
- React is preferred only (Desirable list), though the duties build in it: priority 2, not required.
- No `onsite` knockout: Chicago is the location but no attendance rule is stated.
- Soft skills: problem-solving skills, attention to detail, manage multiple tasks ("meet deadlines" left out by the
  3-soft limit; "Self-starter" is a person phrase).

### wonka-frontend-ui-designer

- Company is "Wonka Public Schools", the only name given (the contractor is unnamed).
- Priority 1: Figma, JavaScript, TypeScript, WCAG 2.1 AA, Section 508; HTML5 and CSS3 are priority 2.
- JavaScript and TypeScript are separate required items (the Technical Skills list names both), not a group.
- Groups: React/Vue/Svelte, MapLibre GL/OpenLayers/Leaflet, both required.
- Cut for the cap: SASS, Tailwind CSS, Git, GitLab, GitHub, Jira, Agile, Jest, Cypress, screen-reader testing, user
  story mapping, usability tests, data visualization, design tokens.
- No knockouts: fully remote; the portfolio is application material. Years 3 → `mid`; degree `equivalent: false`.
