// Spike C template. Data arrives via sys.inputs; the image via the static file resolver.
#import sys: inputs

#set page(paper: "a4", margin: 20mm, footer: context [
  #set text(9pt, fill: gray)
  Checkflat · #inputs.subtitle #h(1fr) #counter(page).display()
])
#set text(font: "Liberation Sans", size: 10.5pt, lang: "pt")
#set par(justify: true, leading: 0.65em)

#block(width: 100%, inset: (bottom: 6pt), stroke: (bottom: 1.5pt + rgb("#143c78")))[
  #text(size: 20pt, weight: "bold", fill: rgb("#143c78"))[#inputs.title]
  #v(2pt)
  #text(size: 10pt, fill: gray)[#inputs.subtitle]
]

#v(10mm)

#figure(
  image("photo.jpg", width: 120mm),
  caption: text(size: 9pt)[Fotografia 1 · 24/09/2026 10:42],
  supplement: none,
  numbering: none,
)

#v(6mm)

#text(size: 12pt, weight: "bold")[Descrição]
#v(2pt)
#inputs.paragraph

#v(4mm)
#box(fill: rgb("#fff3cd"), inset: 6pt, radius: 3pt)[
  #text(size: 9pt)[*Estado:* Em aberto]
]
