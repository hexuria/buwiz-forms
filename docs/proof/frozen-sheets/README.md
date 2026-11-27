# Frozen-sheet proof: 1800, 1801, 2200S

Each form directory has comparison images and PDFs printed through the repository's frozen-HTML print path in the mandated test environment. Values are invented demo data. The “before” PDFs were produced from main commit `75bd78b8`; the “after” PDFs from this branch.

The comparison images show the official BIR PDF page beside the printed page, then a close crop of the official cell, the before-main print, and the after-fixed print. The full pages establish page context; only the cropped cell rows are evidence for the fault being fixed. They do not prove BIR acceptance, filing behavior, or the Windows HTA's runtime print behavior.

- `1800/`: branch `00001` at `p1c18` and page-2 `p2c5`; item 14 centavos at `p1c64` in separate `50` and `05` returns.
- `1801/`: branch `00001` at `p1c17` and page-2 `p2c5`.
- `2200S/`: both Others descriptions at `p2c101` and `p2c108` (`CUSTOM SYRUP MIX`, `HERBAL INFUSION`).

PDFs are binary proof artifacts and are force-added because the repository ignores `*.pdf`. The recording is a short visual pass over these proof images, not a filing workflow; no Queue control is used.

Method note: print HTML was generated from draft print-field maps with `filled_document`, then printed to PDF headlessly. Before-main files necessarily use the same invented return data but the unmodified main sheet/joins, so the fault cells remain blank or malformed there.

## Follow-up corrections after visual review

The official PDFs do print five zeros in the branch-code strip on both pages of 1800 and 1801. Those are paper artwork, not input values. In the first fixed sheets, two frozen zero text layers remained: 1801 page 1 (`p1t30`) and 1800 page 2 (`p2t8`). They have been removed upstream and in this copy; the other two positions already had no frozen zero layer. Since the print inputs are transparent, any remaining layer would have printed through underneath a nonzero branch code. A branch `00000` therefore prints the taxpayer's five zeros once, in the same positions as the official paper artwork.

The v7.9.6.2.1 package's extracted 1800/1801 HTA control inventory has editable five-character branch-code inputs and profile/load code for them; it does not contain evidence of a constant-zero print-only value. This is evidence that the official program prints the taxpayer's branch code, not permanent zeros, although the Windows print path itself was not executed here.

The 1801 proof was regenerated with the estate TIN filled (`000-123-453-00001`) after joining `tinA_1`, `tinB_1`, and `tinC_1` to the three editable groups on page 1. Page 1 and page 2 crops now show the complete TIN plus branch. The 1800 page-2 crop was regenerated from the same corrected sheet.

## Same-shape cells that are not this fault

The same-shape scan found five other text cells with two one-character inputs. Main-print crops are in `same-shape/`:

| Cell | Item | Main filled value | Finding |
|---|---|---:|---|
| `2550m-2007 p1c3` | item 3, number of sheets attached | `12` | digits sit on either side of the center divider tick, not on it |
| `1600wp-2010 p1c62` | attachment period FROM month | `03` | same centered two-slot layout; no divider collision |
| `1600wp-2010 p1c63` | attachment period FROM day | `04` | same centered two-slot layout; no divider collision |
| `1600wp-2010 p1c65` | attachment period TO month | `11` | same centered two-slot layout; no divider collision |
| `1600wp-2010 p1c66` | attachment period TO day | `30` | same centered two-slot layout; no divider collision |

Unlike 1800 `p1c64`, these inputs are positioned as a left/right pair around the tick, so they are listed here rather than changed in this branch.
