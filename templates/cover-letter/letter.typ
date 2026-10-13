// A cover letter (ATS Fit's `cover.typ`; JSON in `sys.inputs.data`: the resume's `name` and `contact`, then `date`,
// `company`, `role`, `greeting`, `paragraphs` and `closing`) under the resume's letterhead: the same name, contact line
// and rule, Times New Roman, one page. Liberation Serif, metric-compatible with Times New Roman, stands in where it
// isn't installed. Records the page count (`<pages>`).

#let data = json(bytes(sys.inputs.at("data")))
#let blue = rgb("#1155cc")

#set document(title: data.name + " · Cover letter")
#set page(paper: "us-letter", margin: (x: 0.75in, y: 0.7in))
#set text(font: ("Times New Roman", "Liberation Serif"), size: 11.5pt, top-edge: "ascender", bottom-edge: "descender", lang: "en", hyphenate: false)
#set par(leading: 0.3em, spacing: 0.95em, justify: false)

#let linked(url, body) = if url == "" { body } else { link(url, underline(text(fill: blue, body))) }

// The resume's letterhead.
#block(text(size: 18pt, strong(data.name)))
#block(above: 0.25em, {
  let parts = data.contact.map(c => if c.url == "" or c.url.starts-with("mailto:") { c.text } else { linked(c.url, c.text) })
  parts.join(" | ")
})
#block(above: 3pt, line(length: 100%, stroke: 0.9pt))

#v(1.2em)
#data.date

#v(0.4em)
#data.company \
Re: #data.role

#v(0.6em)
#data.greeting

#for p in data.paragraphs [#p #parbreak()]

#data.closing

#v(1.6em)
#data.name

#context [#metadata(counter(page).final().first())<pages>]
