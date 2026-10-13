# gliide

- Architecture and roadmap: `ARCHITECTURE.md`.
- Any UI work (screens, components, styles, copy): `design/DESIGN.md` is binding. Read it and
  `.agents/skills/better-ui/SKILL.md` before starting.
  - Values come from `design/tokens.css` only; compose from the components in `app/src/lib/ui/` (`$lib/ui`).
  - A new primitive goes into `app/src/lib/ui/` and the `/specimen` route (`app/src/routes/specimen/`, every state)
    before a screen uses it.
  - Before finishing: `node design/lint.mjs` exits 0, then the better-ui "Before you finish" review on the changed
    files, then a look in light, dark and reduced motion.
