// Checkflat report template (Sprint 5; layout from the client's answers of 2026-10-13, D-025).
// Cover, then the open observations as a continuous list, newest first, several per page. Per
// observation: the header (project name, plan title), the ID and the plan row (crop with the
// labelled pin beside the overview with the crop outlined in red) stay together on one page; the
// description is never split; the photos follow one below the other, each with its caption kept
// together, and may flow to the next page. Page x/y in the footer.
// Content arrives as JSON in sys.inputs.data (see data.rs); images are virtual files under img/.
#import sys: inputs
#let data = json(bytes(inputs.data))
#let L = data.labels

#let blue = rgb("#143c78")
#let frame = rgb("#1a73c7")
#let W = 80mm // width of each snapshot and of a landscape photo
#let PHOTO_H = 70mm // tallest photo (portrait photos are narrower than W)
// A kept-together block taller than this may break after all: unbreakable, it would overflow the
// page (the body is 261 mm) and be clipped.
#let KEEP_MAX = 150mm

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

// Kept on one page, unless taller than KEEP_MAX.
#let keep(body) = layout(size => {
  let h = measure(width: size.width, body).height
  block(width: 100%, breakable: h > KEEP_MAX, body)
})

// Pin with the app's marker label ("3", "PC-01"), its tip on (px, py) inside a box W wide. The
// head is a stadium as wide as the label needs; it stays inside the box — centre clamped
// horizontally, hung below the point when there is no room above — so a label at the crop's edge
// is never cut off or drawn over the overview beside it.
#let pin(px, py, mark) = context {
  let r = 2.8mm
  let h = 2.2 * r // tip to head centre
  let t = text(size: 7pt, weight: "bold", fill: white, mark)
  let hw = calc.max(r, measure(t).width / 2 + 1.2mm)
  let cx = calc.max(hw, calc.min(W - hw, px)) // calc.clamp takes no lengths
  let up = py >= h + r
  let cy = if up { py - h } else { py + h }
  let base = if up { cy + 0.6 * r } else { cy - 0.6 * r } // where the tail leaves the head
  let (x0, y0) = (calc.min(px, cx - 0.8 * r), calc.min(py, base))
  place(top + left, dx: x0, dy: y0, polygon(fill: blue, (cx - 0.8 * r - x0, base - y0), (cx + 0.8 * r - x0, base - y0), (px - x0, py - y0)))
  place(top + left, dx: cx - hw, dy: cy - r, rect(width: 2 * hw, height: 2 * r, radius: r, fill: blue, stroke: 0.7pt + white, align(center + horizon, t)))
}

// The image box clips; the pin is placed in an outer box (it stays inside it, see `pin`).
#let crop(c, mark) = {
  let H = W * c.aspect
  box(width: W, height: H)[
    #place(top + left, box(width: W, height: H, clip: true, stroke: 0.5pt + luma(170), image(c.file, width: W, height: H)))
    #pin(c.pin_x * W, c.pin_y * H, mark)
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

// Frame + caption, never split.
#let photo(p) = block(breakable: false)[
  #let w = calc.min(W, PHOTO_H / p.aspect)
  #box(stroke: 1pt + frame, inset: 2pt, image(p.file, width: w - 4pt))
  #v(1mm)
  #text(size: 9pt, style: "italic", fill: luma(80))[#p.caption]
]

// ---- the observations, newest first, as a continuous list ----
// Spacing inside the list is explicit (`v`), with the implicit paragraph/block spacing off: the
// ≈ 4.4 mm Typst would add around every piece made the head (header + ID + plan row) ≈ 129 mm,
// just too tall to follow two photos on a page, so every observation took two pages (D-025).
#if data.observations.len() > 0 { pagebreak() }
#[
  #set par(spacing: 0pt)
  #set block(spacing: 0pt)
  #for (k, o) in data.observations.enumerate() [
    #if k > 0 { v(8mm) }
    // Header, ID and plan row on one page together (≈ 104 mm).
    #block(width: 100%, breakable: false)[
      #block(width: 100%, inset: (bottom: 5pt), stroke: (bottom: 0.5pt + luma(150)))[
        #grid(
          columns: (1fr, auto),
          align: (left + bottom, right + bottom),
          column-gutter: 4mm,
          text(size: 14pt)[#data.project.name],
          text(size: 11pt, fill: luma(90))[#o.plan_title],
        )
      ]
      #v(3mm)
      #text(weight: "bold")[#L.id:] #o.ref
      #v(2mm)
      #rule
      #v(2mm)
      #label(L.plan)
      #v(1.5mm)
      #grid(
        columns: (W, W),
        column-gutter: 6mm,
        if o.crop != none { crop(o.crop, o.marker) } else { missing },
        if o.overview != none { overview(o.overview) } else { missing },
      )
    ]
    #v(3mm)
    #keep[
      #rule
      #v(2mm)
      #label(L.description)
      #v(1mm)
      #text(hyphenate: true)[#o.description]
    ]
    #v(3mm)
    // The label stays with the first photo.
    #block(sticky: true)[
      #rule
      #v(2mm)
      #label(L.images)
    ]
    #v(2mm)
    #for (j, p) in o.photos.enumerate() {
      if j > 0 { v(4mm) }
      photo(p)
    }
  ]
]
