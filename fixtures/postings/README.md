# Posting fixtures

Synthetic job postings for the provider spike (M0), the extract prompt and the golden tests. The companies are
fictional and the text was written for these tests, modeled on the kinds of postings users paste: the same role
types, skill mixes, knockouts (sponsorship, onsite days, degree and graduation windows, background checks), salary
and location forms, and the markdown artifacts that pasting leaves behind. Each file is front matter (`id`,
`company`, `title`, `location`, `track`, `source`, `method`, `fetched`) and the posting text as the app would receive
it.

| id | Company | Title | Track | How it was read |
|---|---|---|---|---|
| `acme-swe-robotics` | Acme Robotics | Software Engineer, ROS | swe | pasted |
| `northwind-jr-swe` | Northwind Media | Junior Software Engineer | swe | Greenhouse board API |
| `initech-entry-swe` | Initech Consulting | Entry-Level Software Engineer | swe | Ashby posting API |
| `fabrikam-mobile-dev-1` | Fabrikam Health | Mobile Developer I - US Remote | swe | Greenhouse board API |
| `aperture-swe-ai-wearable` | Aperture Wearables | Software Engineer (AI Wearable Tech) | swe | LinkedIn guest API |
| `hooli-fullstack-swe` | Hooli Cloud | Software Engineer (Full-Stack) | swe | pasted |
| `contoso-dev-integrations-ai` | Contoso Learning | Developer - Integrations & AI | swe | pasted |
| `wayne-ui-developer` | Wayne Markets | UI Developer - Options Market Making | ui | pasted |
| `stark-ux-engineer` | Stark Medical Society | UX Engineer | ui | pasted |
| `wonka-frontend-ui-designer` | Wonka Public Schools contractor (unnamed) | Front-end UI designer | design | pasted |
| `vandelay-systems-automation` | Vandelay Realty & Development, LLC | Systems & Automation Engineer | it | ApplicantPro JSON-LD |
| `globex-swe` | Globex Bank | 2027 Software Engineer Program (February start) | swe | pasted, no front matter |

A LinkedIn search link (`https://www.linkedin.com/jobs/search-results/?currentJobId=…`) is not a posting fixture:
its recorded response is the ingest test `fixtures/ingest/linkedin-search/`, a synthetic "Analyst, Business Systems"
posting at Oscorp Biosciences.
