#let put(x, y, body) = place(top + left, dx: x * 1pt, dy: y * 1pt, body)
#let label(x, y, size, body) = put(x, y, text(font: "Arial", size: size * 1pt, body))
#let mark(x, y) = put(x, y, text(font: "Arial", size: 13pt, weight: "bold", "X"))
#let cells(x, y, cw, s) = {
  for (i, ch) in s.clusters().enumerate() {
    put(x + i * cw + cw / 2 - 2.3, y, text(font: "Arial", size: 8.5pt, ch))
  }
}
#let amount(x, y, cw, intcells, decx, s) = {
  let clean = if s == "" { "0.00" } else { s }
  let parts = clean.split(".")
  let int = parts.at(0, default: "0")
  let dec = parts.at(1, default: "00")
  let start = intcells - int.len()
  for (i, ch) in int.clusters().enumerate() {
    put(x + (start + i) * cw + cw / 2 - 2.3, y, text(font: "Arial", size: 8.5pt, ch))
  }
  for (i, ch) in dec.clusters().enumerate() {
    put(decx + i * cw + cw / 2 - 2.3, y, text(font: "Arial", size: 8.5pt, ch))
  }
}
