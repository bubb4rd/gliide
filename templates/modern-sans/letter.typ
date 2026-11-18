// A cover letter (JSON in `sys.inputs.data`: the resume's `name` and `contact`, then `date`, `company`, `role`,
// `greeting`, `paragraphs` and `closing`) under modern-sans's letterhead: the same semibold name, gray contact line with
// teal links, then a hairline as its section titles have; Inter 10.5pt, one page. Records the page count (`<pages>`).

#let data = json(bytes(sys.inputs.at("data")))

#let ink = rgb("#16181d")
#let soft = rgb("#5c6270")
#let faint = rgb("#c9cdd4")
#let accent = rgb("#0b5d6b")

#set document(title: data.name + " · Cover letter")
#set page(paper: "us-letter", margin: (x: 0.75in, y: 0.7in))
#set text(font: "Inter", size: 10.5pt, fill: ink, lang: "en", hyphenate: false)
#set par(leading: 0.62em, spacing: 1.15em, justify: false)
#set strong(delta: 200)

#let linked(url, body) = if url == "" { body } else { link(url, text(fill: accent, body)) }

// The resume's letterhead.
#block(below: 0.55em, text(size: 23pt, weight: "semibold", tracking: -0.015em, data.name))
#block(text(size: 8.75pt, fill: soft, {
  // Items never break inside, so a URL wraps whole onto the next line.
  let parts = data.contact.map(c => box(if c.url == "" or c.url.starts-with("mailto:") { c.text } else { linked(c.url, c.text) }))
  parts.join(h(0.5em) + text(fill: faint, "|") + h(0.5em))
}))
#block(above: 0.9em, line(length: 100%, stroke: 0.5pt + faint))

#v(1.4em)
#text(fill: soft, data.date)

#v(0.3em)
#data.company \
Re: #data.role

#v(0.5em)
#data.greeting

#for p in data.paragraphs [#p #parbreak()]

#data.closing

#v(1.4em)
#strong(data.name)

#context [#metadata(counter(page).final().first())<pages>]
