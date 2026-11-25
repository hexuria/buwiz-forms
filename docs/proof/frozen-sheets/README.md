# Frozen-sheet proof: 1800, 1801, 2200S

Each form directory has comparison images and PDFs printed through the repository's frozen-HTML print path in the mandated test environment. Values are invented demo data. The “before” PDFs were produced from main commit `75bd78b8`; the “after” PDFs from this branch.

The comparison images show the official BIR PDF page beside the printed page, then a close crop of the official cell, the before-main print, and the after-fixed print. The full pages establish page context; only the cropped cell rows are evidence for the fault being fixed. They do not prove BIR acceptance, filing behavior, or the Windows HTA's runtime print behavior.

- `1800/`: branch `00001` at `p1c18` and page-2 `p2c5`; item 14 centavos at `p1c64` in separate `50` and `05` returns.
- `1801/`: branch `00001` at `p1c17` and page-2 `p2c5`.
- `2200S/`: both Others descriptions at `p2c101` and `p2c108` (`CUSTOM SYRUP MIX`, `HERBAL INFUSION`).

PDFs are binary proof artifacts and are force-added because the repository ignores `*.pdf`. The recording is a short visual pass over these proof images, not a filing workflow; no Queue control is used.

Method note: print HTML was generated from draft print-field maps with `filled_document`, then printed to PDF headlessly. Before-main files necessarily use the same invented return data but the unmodified main sheet/joins, so the fault cells remain blank or malformed there.
