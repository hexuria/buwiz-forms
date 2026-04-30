#set page(width: 612pt, height: 936pt, margin: 0pt)
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

#page(background: image("pages/page1.svg", width: 612pt, height: 936pt), foreground: {
  mark(85.48, 113.24, 13.14, 11.40)
  mark(157.26, 113.17, 12.97, 11.53)
  cells(136.07, 126.22, 84.59, 17.86, 6, "X")
  mark(243.89, 126.55, 12.85, 11.31)
  mark(286.53, 126.32, 12.48, 12.09)
  mark(328.41, 126.48, 13.29, 11.35)
  mark(370.88, 125.79, 12.65, 12.24)
  mark(425.11, 127.00, 13.05, 12)
  mark(467.08, 127.78, 12.36, 11.80)
  cells_rtl(560.62, 126.04, 27.82, 18.19, 2, "00")
  cells(220.12, 160.28, 42.45, 16.98, 3, "X")
  cells(277.59, 160.26, 42.30, 17.33, 3, "X")
  cells(334.16, 160.10, 42.30, 17.56, 3, "X")
  cells(390.60, 160.09, 70.66, 17.39, 5, "X")
  cells(546.78, 160.24, 42.05, 17.22, 3, "X")
  cells(23.96, 189.93, 564.54, 17.12, 40, "X")
  cells(23.87, 218.43, 564.87, 17.43, 40, "X")
  cells(532.46, 236.81, 55.75, 16.68, 4, "X")
  cells(24.01, 265.97, 167.41, 17.21, 12, "X")
  cells(192.17, 265.98, 395.63, 17.28, 28, "X")
  mark(191.58, 286.71, 13.97, 12)
  mark(234.71, 287.53, 13.30, 11.32)
  mark(181.28, 326.99, 12.80, 12.50)
  mark(348.14, 327.69, 12.66, 12)
  cells_rtl(376.72, 362.14, 169.32, 16.91, 12, "X")
  cells_rtl(376.53, 391.83, 169.57, 16.77, 12, "X")
  cells_rtl(376.82, 409.52, 169.27, 17.74, 12, "X")
  cells_rtl(376.66, 428.34, 169.07, 18.20, 12, "X")
  cells_rtl(376.98, 446.57, 169.19, 17.23, 12, "X")
  cells_rtl(376.81, 464.69, 169.01, 18.08, 12, "X")
  cells_rtl(376.76, 493.60, 169.75, 17.89, 12, "X")
  cells_rtl(376.86, 512.35, 169.44, 17.36, 12, "X")
  cells_rtl(376.74, 530.31, 169.30, 17.75, 12, "X")
  cells_rtl(377.10, 548.14, 168.97, 18.18, 12, "X")
  cells_rtl(376.44, 567, 169.73, 17.87, 12, "X")
  cells(560.59, 362.16, 28.50, 16.72, 2, "00")
  cells(560.97, 390.90, 27, 18.32, 2, "00")
  cells(561.00, 409.48, 27.40, 18.00, 2, "00")
  cells(561.27, 428.56, 27.55, 17.07, 2, "00")
  cells(561.37, 446.40, 27.38, 16.73, 2, "00")
  cells(561.20, 463.92, 26.71, 18.16, 2, "00")
  cells(561.32, 493.57, 27.00, 18.28, 2, "00")
  cells(561.06, 512.12, 27.11, 17.38, 2, "00")
  cells(560.87, 530.78, 27.70, 17.23, 2, "00")
  cells(560.82, 548.88, 27.58, 17.45, 2, "00")
  cells(561.10, 567.01, 27.72, 17.79, 2, "00")
  cells(223.70, 430.34, 151.24, 10.89, 1, "X")
})[]
#page(background: image("pages/page2.svg", width: 612pt, height: 936pt), foreground: {
  cells(23.72, 108.04, 41.00, 17.23, 3, "X")
  cells(65.88, 107.72, 41.48, 17.33, 3, "X")
  cells(108.51, 108.25, 40.95, 16.88, 3, "X")
  cells(151.28, 107.88, 69.53, 16.84, 5, "X")
  cells(221.69, 108.24, 366.39, 16.57, 26, "X")
  cells(37.67, 162.84, 69.88, 16.68, 5, "X")
  cells_rtl(108.48, 162.72, 169.54, 17.62, 12, "X")
  cells_rtl(320.66, 162.80, 27.46, 16.66, 2, "00")
  cells_rtl(363.47, 162.72, 169.46, 17.33, 12, "X")
  cells_rtl(362.81, 271.66, 169.81, 18.28, 12, "X")
  cells(292.03, 162.19, 28.05, 17.38, 2, "00")
  cells(546.85, 162.82, 28.55, 16.93, 2, "00")
  cells(546.97, 271.79, 27.56, 17.46, 2, "00")
})[]