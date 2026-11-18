// A master or tailored resume (the v1 master JSON shape, as a JSON string in `sys.inputs.data`) in a contemporary
// single-column sans layout: Inter 9.5pt on a 12.8pt line pitch, 0.6in margins, a semibold name, section titles as
// small tracked capitals in one ink-teal accent followed by a hairline, entry names in semibold with the place and
// dates in gray (dates right-aligned, tabular figures), skills as a label column beside their items. Everything sits in
// one column in reading order, so a parser reads it top to bottom as written.
//
// For the fit check it records, as queryable metadata, each bullet's and skills line's wrapped line count (`<bullet>`,
// `<group>`: `id`, `lines`, and `orig`, the count of the master text it replaces when the data carries one) and the
// page count (`<pages>`). With `--input highlight=1` (the review's preview, never the export) the text the tailoring
// changed is highlighted: bullets and note lines marked `changed`, a skills line's `added` items.
//
// With `--input mode=measure` it reads no resume; it lays out each of `sys.inputs.candidates` (JSON:
// `[{id, kind: "bullet" | "note" | "group", depth, text, label}]`) at the width of the line it targets (a bullet's text
// column at its `depth`, a note line, a skills row: its label column beside the items) and records its wrapped line
// count as `<measure>` metadata (`id`, `lines`). One compile measures every candidate text of a fit check.

#let measuring = sys.inputs.at("mode", default: "") == "measure"
#let data = if measuring { (name: "", contact: (), sections: ()) } else { json(bytes(sys.inputs.at("data"))) }

#let ink = rgb("#16181d")
#let soft = rgb("#5c6270")
#let faint = rgb("#c9cdd4")
// The one accent: a deep teal that prints as a dark gray.
#let accent = rgb("#0b5d6b")
#let indent = 0.17in
#let label-col = 1.45in

#set document(title: data.name)
#set page(paper: "us-letter", margin: (x: 0.6in, top: 0.55in, bottom: 0.5in))
#set text(font: "Inter", size: 9.5pt, fill: ink, lang: "en", hyphenate: false)
#set par(leading: 0.62em, spacing: 0.62em, justify: false)
#set block(spacing: 0.62em)
#set strong(delta: 200)

// `body` in the accent, linking to `url`; plain without one.
#let linked(url, body) = if url == "" { body } else { link(url, text(fill: accent, body)) }
// An entry name stays ink; a link shows as a thin accent underline.
#let linked-name(url, body) = if url == "" { body } else {
  link(url, underline(stroke: 0.6pt + accent, offset: 2.2pt, body))
}

#let marking = sys.inputs.at("highlight", default: "") == "1"
#let mark(on, body) = if marking and on { highlight(fill: rgb("#ffeaa0"), extent: 1.2pt, body) } else { body }

// Lines `body` wraps to at `width`.
#let lines-of(body, width) = {
  let one = measure(block(width: width, [x])).height
  let two = measure(block(width: width, [x \ x])).height
  let h = measure(block(width: width, body)).height
  1 + calc.round((h - one) / (two - one))
}

// Text that records its line count (and its master text's, `orig`) under `label`.
#let counted(id, body, orig, tag) = layout(size => {
  let n = lines-of(body, size.width)
  let o = if orig == none { n } else { lines-of(orig, size.width) }
  [#metadata((id: id, lines: n, orig: o))#label(tag)]
  block(width: size.width, body)
})

#let dot = box(height: 0.7em, align(horizon, circle(radius: 1.25pt, fill: accent)))

#let bullet(b, depth, tag: "bullet") = pad(left: depth * indent, grid(
  columns: (indent, 1fr),
  dot,
  counted(b.id, mark(b.at("changed", default: false), b.text), b.at("orig", default: none), tag),
))

// A note line: its semibold label, when it has one, then its text.
#let note-line(label, body) = if label != "" [*#label:* #body] else [#body]

// A skills row: the label in its column, the items beside it. Measured whole, so a label that wraps counts too.
#let skills-row(label, items) = grid(
  columns: (label-col, 1fr),
  strong(label), items,
)

#let section-head(title) = block(above: 1.35em, below: 0.75em, sticky: true, grid(
  columns: (auto, 1fr),
  column-gutter: 0.7em,
  align: horizon,
  text(size: 8.25pt, weight: "semibold", fill: accent, tracking: 0.05em, title),
  line(length: 100%, stroke: 0.5pt + faint),
))

// An entry with no name, place or dates (a summary line) has no header row.
#let entry(e, depth) = {
  if (e.name, e.place, e.dates) != ("", "", "") {
    block(above: 0.95em, below: 0.5em, sticky: true, grid(
      columns: (1fr, auto),
      column-gutter: 1em,
      [#text(size: 10.25pt, linked-name(e.url, strong(e.name)))#if e.place != "" [#h(0.55em)#text(fill: soft, e.place)]],
      text(fill: soft, features: ("tnum",), e.dates),
    ))
  }
  if e.subtitle != "" { block(above: 0.5em, below: 0.6em, sticky: true, emph(e.subtitle)) }
  for n in e.notes {
    block(mark(n.at("changed", default: false), note-line(n.label, n.text)))
  }
  for b in e.bullets { bullet(b, depth) }
}

#let group(g) = {
  let added = g.at("added", default: ())
  let row(items) = skills-row(g.label, items.map(i => mark(i in added, i)).join(", "))
  counted(g.id, row(g.items), if "orig" in g { row(g.orig) } else { none }, "group")
}

#if not measuring {
  block(below: 0.55em, text(size: 23pt, weight: "semibold", tracking: -0.015em, data.name))
  block(text(size: 8.75pt, fill: soft, {
    // Items never break inside, so a URL wraps whole onto the next line.
    let parts = data.contact.map(c => box(if c.url == "" or c.url.starts-with("mailto:") { c.text } else { linked(c.url, c.text) }))
    parts.join(h(0.5em) + text(fill: faint, "|") + h(0.5em))
  }))
}

#for s in data.sections {
  section-head(s.title)
  for e in s.entries { entry(e, if e.notes.len() > 0 { 1 } else { 0 }) }
  for g in s.groups { group(g) }
}

#if measuring {
  for c in json(bytes(sys.inputs.at("candidates"))) {
    if c.kind == "bullet" {
      bullet((id: c.id, text: c.text), c.depth, tag: "measure")
    } else if c.kind == "note" {
      counted(c.id, note-line(c.at("label", default: ""), c.text), none, "measure")
    } else {
      counted(c.id, skills-row(c.label, c.text), none, "measure")
    }
  }
}

#context [#metadata(counter(page).final().first())<pages>]
