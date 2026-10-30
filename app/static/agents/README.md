# Agent icons

The agents' and their makers' brand icons, bundled so the app makes no third-party request for them. Each is the
brand's own published icon, unmodified except scaled down to 64px, and always shown as published (`CompanyMark` in
`$lib/ui`; DESIGN.md, Company logos). Fetched 2026-10-14. `$lib/agents` names which is which:

- `claude.png`, `openai.png`, `antigravity.png`: Claude Code, Codex and Antigravity, in Settings → Agents, the setup's
  Agents and Ready steps, the sidebar's foot (`AgentMark`, the agent model calls run on) and the chat's composer.
- `claude.png`, `openai.png`, `gemini.png`: the maker of the model that answered, beside its name in a review's chat
  and an entry's dig (`makerIcon`); a model from any other maker shows the agent's icon instead.

| File | Source |
|---|---|
| `claude.png` | https://claude.ai/apple-touch-icon.png (claude.ai's `apple-touch-icon`: the spark on Claude's orange) |
| `openai.png` | https://github.com/openai.png (OpenAI's GitHub organization avatar; openai.com refuses scripted requests) |
| `gemini.png` | https://www.gstatic.com/lamda/images/gemini_sparkle_4g_512_lt_f94943af3be039176192d.png (gemini.google.com's `icon` and `apple-touch-icon-precomposed`) |
| `antigravity.png` | https://antigravity.google/apple-touch-icon.png (antigravity.google's `apple-touch-icon`: Google Antigravity's arch) |

These icons are trademarks of their owners, used only to identify each agent; they are not covered by gliide's
license, and gliide is not affiliated with, sponsored or endorsed by any of them.
