#let mapping = json("mapping.json")
#let data = json("data.json")

#set page(
   width: 612pt,
   height: 1008pt,
   margin: 0pt,
   background: image("blank_2551Q_0.png", width: 100%, height: 100%)
)

// Use standard web font for eBIRForms HTML replica
#set text(font: ("Arial", "Helvetica", "sans-serif"), size: 10pt)

#let checkbox(val) = {
   if val == "true" {
       text(font: "Arial", size: 12pt)[X]
   }
}

#for (key, pos) in mapping.pairs() {
   if pos.page == 1 {
       let val = data.at(key, default: "")
       if val == "true" or val == "false" {
           val = checkbox(val)
           place(dx: pos.x * 1pt, dy: pos.y * 1pt)[#val]
       } else {
           // Right-align financial amounts
           let is_amount = key.starts-with("frm2551Qv2018:txt") and (val.contains(".") or key.len() <= 19)
           
           // DYNAMIC COVER-UP: We draw a white box with a black border exactly over the bounding box.
           // This completely covers and erases the underlying segmented boxes from the background PNG,
           // perfectly mimicking the native eBIRForms clean HTML output!
           place(dx: pos.x * 1pt, dy: (pos.y - 2) * 1pt)[
               #box(
                   width: pos.w * 1pt, 
                   height: pos.h * 1pt, 
                   fill: white, 
                   stroke: 0.5pt + black,
                   inset: (left: 2pt, right: 2pt, top: 2pt),
                   align(if is_amount { right } else { left })[
                       #val
                   ]
               )
           ]
       }
   }
}
