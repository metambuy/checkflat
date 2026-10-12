// Checkflat report template (Sprint 5). Layout follows the client's mockup: cover, then one
// block per open observation — header with the project name and ID, plan crop with the pin
// beside the overview with the crop outlined in red, description, captioned photos, page x/y.
// Content arrives as JSON in sys.inputs.data (see data.rs); images are virtual files under img/.
#import sys: inputs
#let data = json(bytes(inputs.data))
#let L = data.labels

#let blue = rgb("#143c78")
#let frame = rgb("#1a73c7")
#let W = 80mm // width of each snapshot and of a landscape photo
#let PHOTO_H = 70mm // tallest photo (portrait photos are narrower than W)

#set page(paper: "a4", margin: (x: 20mm, top: 18mm, bottom: 18mm), footer: context [
  #set text(9pt, fill: luma(110))
  #h(1fr) #counter(page).display("1/1", both: true)
])
#set text(font: "Liberation Sans", size: 10.5pt, lang: data.lang, region: data.region)
#set par(justify: true, leading: 0.65em)

// ---- cover (provisional: no logo, no attendees yet) ----
#v(70mm)
#text(size: 24pt, weight: "bold", fill: blue)[#data.title]
#v(8mm)
#text(size: 16pt)[#data.project.name]
#if data.project.address != "" [
  #v(1mm)
  #text(size: 12pt, fill: luma(90))[#data.project.address]
]
#v(6mm)
#text(size: 11pt)[#L.date: #data.date]
#if data.observations.len() == 0 [
  #v(12mm)
  #text(fill: luma(110))[#L.no_observations]
]

// ---- helpers ----
#let rule = line(length: 100%, stroke: 1pt + black)
#let label(s) = text(style: "italic", fill: luma(80))[#s:]

// Teardrop pin with its tip on the point (px, py) inside the enclosing box.
#let pin(px, py) = {
  let r = 2.6mm
  let h = 2.2 * r
  place(top + left, dx: px - 0.8 * r, dy: py - h, polygon(fill: blue, (0mm, 0mm), (1.6 * r, 0mm), (0.8 * r, h)))
  place(top + left, dx: px - r, dy: py - h - r, circle(radius: r, fill: blue, stroke: 0.7pt + white))
  place(top + left, dx: px - 0.35 * r, dy: py - h - 0.35 * r, circle(radius: 0.35 * r, fill: white))
}

// The image box clips; the pin is placed in an outer box so a pin at the edge keeps its head.
#let crop(c) = {
  let H = W * c.aspect
  box(width: W, height: H)[
    #place(top + left, box(width: W, height: H, clip: true, stroke: 0.5pt + luma(170), image(c.file, width: W, height: H)))
    #pin(c.pin_x * W, c.pin_y * H)
  ]
}

#let overview(o) = {
  let H = W * o.aspect
  box(width: W, height: H, clip: true, stroke: 0.5pt + luma(170))[
    #place(top + left, image(o.file, width: W, height: H))
    #place(top + left, dx: o.box_x * W, dy: o.box_y * H, rect(width: o.box_w * W, height: o.box_h * H, stroke: 1.2pt + red))
  ]
}

#let missing = box(width: W, height: 30mm, stroke: 0.5pt + luma(170), fill: luma(245))[
  #align(center + horizon)[#text(size: 9pt, fill: luma(110))[#L.no_snapshot]]
]

#let photo(p) = block(breakable: false)[
  #let w = calc.min(W, PHOTO_H / p.aspect)
  #box(stroke: 1pt + frame, inset: 2pt, image(p.file, width: w - 4pt))
  #v(1mm)
  #text(size: 9pt, style: "italic", fill: luma(80))[#p.caption]
]

// ---- one block per observation, newest first ----
#for o in data.observations [
  #pagebreak(weak: true)
  #block(width: 100%, inset: (bottom: 5pt), stroke: (bottom: 0.5pt + luma(150)))[
    #text(size: 14pt)[#data.project.name]
  ]
  #v(4mm)
  #text(weight: "bold")[#L.id:] #o.ref
  #v(2mm)
  #rule
  #v(3mm)
  #label(L.plan)
  #v(2mm)
  #grid(
    columns: (W, W),
    column-gutter: 6mm,
    if o.crop != none { crop(o.crop) } else { missing },
    if o.overview != none { overview(o.overview) } else { missing },
  )
  #v(3mm)
  #rule
  #v(3mm)
  #label(L.description)
  #v(1mm)
  #text(hyphenate: true)[#o.description]
  #v(3mm)
  #rule
  #v(3mm)
  #label(L.images)
  #v(2mm)
  #grid(columns: (W, W), column-gutter: 6mm, row-gutter: 4mm, ..o.photos.map(photo))
]
