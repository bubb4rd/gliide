// A master or tailored resume (ATS Fit's master.json shape, as a JSON string in `sys.inputs.data`) in the one-page
// Google Docs design: Times New Roman 11pt, 0.5in margins, sections between rules, bold names with italic dates at the
// right. Liberation Serif, metric-compatible with Times New Roman, stands in where it isn't installed.
//
// For the fit check it records, as queryable metadata, each bullet's and skills line's wrapped line count (`<bullet>`,
// `<group>`: `id`, `lines`, and `orig`, the count of the master text it replaces when the data carries one) and the
// page count (`<pages>`). With `--input highlight=1` (the review's preview, never the export) the text the tailoring
// changed is highlighted: bullets and note lines marked `changed`, a skills line's `added` items.
//
// With `--input mode=measure` it reads no resume; it lays out each of `sys.inputs.candidates` (JSON:
// `[{id, kind: "bullet" | "note" | "group", depth, text, label}]`) at the width of the line it targets (a bullet's text
// column at its `depth`, a note line, a skills line after its bold `label`) and records its wrapped line count as
// `<measure>` metadata (`id`, `lines`). One compile measures every candidate text of a fit check.

#let measuring = sys.inputs.at("mode", default: "") == "measure"
#let data = if measuring { (name: "", contact: (), sections: ()) } else { json(bytes(sys.inputs.at("data"))) }
#let blue = rgb("#1155cc")
#let indent = 0.25in

#set document(title: data.name)
#set page(paper: "us-letter", margin: 0.5in)
#set text(font: ("Times New Roman", "Liberation Serif"), size: 11pt, top-edge: "ascender", bottom-edge: "descender", lang: "en", hyphenate: false)
// Line pitch 12.62pt, the Docs original's: Times New Roman's ascender-to-descender box is 0.91em, plus this leading.
#set par(leading: 0.238em, spacing: 0.238em, justify: false)
#set block(spacing: 0.238em)

// `body` underlined in link blue, linking to `url`; plain without one.
#let linked(url, body) = if url == "" { body } else { link(url, underline(text(fill: blue, body))) }

#let marking = sys.inputs.at("highlight", default: "") == "1"
#let mark(on, body) = if marking and on { highlight(fill: rgb("#ffe58f"), extent: 1pt, body) } else { body }

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

#let bullet(b, depth, tag: "bullet") = pad(left: depth * indent, grid(
  columns: (indent, 1fr),
  text(size: 10pt, baseline: 0.05em)[●],
  counted(b.id, mark(b.at("changed", default: false), b.text), b.at("orig", default: none), tag),
))

// A note line: its bold label, when it has one, then its text.
#let note-line(label, body) = if label != "" [*#label:* #body] else [#body]

#let rule = block(above: 2pt, below: 2pt, line(length: 100%, stroke: 0.9pt))

#let section-head(title) = {
  rule
  block(above: 0.5pt, below: 0.5pt, text(size: 11.5pt, strong(title)))
  rule
}

// An entry with no name, place or dates (a summary line) has no header row.
#let entry(e, depth) = {
  if (e.name, e.place, e.dates) != ("", "", "") {
    grid(
      columns: (1fr, auto),
      [#linked(e.url, strong(e.name))#if e.place != "" [*,* #e.place]],
      emph(e.dates),
    )
  }
  if e.subtitle != "" { block(emph(e.subtitle)) }
  for n in e.notes {
    block(mark(n.at("changed", default: false), note-line(n.label, n.text)))
  }
  for b in e.bullets { bullet(b, depth) }
}

#let group(g) = {
  let added = g.at("added", default: ())
  let line(items) = [*#g.label:* #items.map(i => mark(i in added, i)).join(", ")]
  counted(g.id, line(g.items), if "orig" in g { line(g.orig) } else { none }, "group")
}

#v(2pt)
#block(text(size: 18pt, strong(data.name)))
#block({
  let parts = data.contact.map(c => if c.url == "" or c.url.starts-with("mailto:") { c.text } else { linked(c.url, c.text) })
  parts.join(" | ")
})
#v(1pt)

#for s in data.sections {
  section-head(s.title)
  // Bullets under notes (an education entry's honors) sit one step in, as in the Docs original.
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
      counted(c.id, [*#c.label:* #c.text], none, "measure")
    }
  }
}

#context [#metadata(counter(page).final().first())<pages>]
