#let put(x, y, body) = place(top + left, dx: x * 1pt, dy: y * 1pt, body)
#let label(x, y, w, h, size, body) = put(x, y, box(width: w * 1pt, height: h * 1pt, align(horizon + left, text(font: "Arial", size: size * 1pt, body))))
#let mark(x, y, w, h) = put(x, y, box(width: w * 1pt, height: h * 1pt, align(center + horizon, text(font: "Arial", size: 11pt, weight: "bold", "X"))))

#let cells(x, y, w, h, count, s) = put(x, y, box(width: w * 1pt, height: h * 1pt, {
  if count <= 0 {
    align(horizon + left, text(font: "Arial", size: 8.5pt, s))
  } else {
    let cw = w / count
    for (i, ch) in s.clusters().enumerate() {
      place(dx: i * cw * 1pt, dy: 0pt, box(width: cw * 1pt, height: h * 1pt, align(center + horizon, text(font: "Arial", size: 8.5pt, ch))))
    }
  }
}))

#let cells_rtl(x, y, w, h, count, s) = put(x, y, box(width: w * 1pt, height: h * 1pt, {
  if count <= 0 {
    align(horizon + right, text(font: "Arial", size: 8.5pt, s))
  } else {
    let cw = w / count
    let start = count - s.clusters().len()
    for (i, ch) in s.clusters().enumerate() {
      place(dx: (start + i) * cw * 1pt, dy: 0pt, box(width: cw * 1pt, height: h * 1pt, align(center + horizon, text(font: "Arial", size: 8.5pt, ch))))
    }
  }
}))

#let amount(x, y, w, h, count, s) = put(x, y, box(width: w * 1pt, height: h * 1pt, {
  if s != "" {
    if count <= 2 {
      align(horizon + right, text(font: "Arial", size: 8.5pt, s))
    } else {
      let cw = w / count
      let parts = s.split(".")
      let int = parts.at(0, default: "0")
      let dec = parts.at(1, default: "00")
      let start = count - 2 - int.len()
      for (i, ch) in int.clusters().enumerate() {
        place(dx: (start + i) * cw * 1pt, box(width: cw * 1pt, height: h * 1pt, align(center + horizon, text(font: "Arial", size: 8.5pt, ch))))
      }
      for (i, ch) in dec.clusters().enumerate() {
        place(dx: (count - 2 + i) * cw * 1pt, box(width: cw * 1pt, height: h * 1pt, align(center + horizon, text(font: "Arial", size: 8.5pt, ch))))
      }
    }
  }
}))
