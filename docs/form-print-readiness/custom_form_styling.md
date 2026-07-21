# Custom form styling — recurring patterns for every eBIRForms conversion

Read this **before building or reviewing any form**. Every item below is a
defect that was found by eye, one form at a time, during review — and every one
recurs across forms because they share primitives and the same official-form
conventions. Bake these in when generating a new form and it starts correct
instead of accumulating a review backlog.

Three provenance tags are used throughout:

- **[contract]** — verified against the form's geometry contract
  (`packages/form-specs/geometry-contracts/<stem>.json`). The contract is the
  arbiter; your eye finds candidates, the contract decides.
- **[owner]** — a deliberate product-owner decision that *diverges* from the
  official form. Do not "correct" these back to official.
- **[engineering]** — a CSS/React pitfall in our own code, not about the form.

The overriding rule: **SEE it.** `node scripts/make_section_crops.mjs --form
CODE --page N` cuts a page into official-on-top / ours-below bands at the form's
own black rules. Read them before and after any change. No pixel metric catches
most of what follows.

---

## 1. Borders and rules

### 1.1 Uniform thickness — 1 CSS px everywhere **[owner]**
All borders, rules, box outlines and comb dividers render at **1 CSS px**. No
2px/3px double-width boundaries. See
[border-thickness-decision.md](border-thickness-decision.md) — this deliberately
diverges from the official per-section weights (official boundaries are
1.44–1.92pt); do not re-derive heavier borders from the contract.

- At device scale 1.5, Chromium floors border-width to integer CSS px:
  `0.5–1.4pt → 1px`, `1.5–2pt → 2px`, `2.25pt+ → 3px`. To guarantee 1 px, use a
  value in `[0.5pt, 1.4pt]` (e.g. `0.75pt` or `1px`). Only a change that crosses
  a bucket renders differently — `1pt → 1.2pt` is a no-op.
- When you make a border thinner, **remove any displacement compensation** that
  was added to counter a thicker one (padding/height/caption nudges). Otherwise
  the border shrinks but the compensation still shifts content.

### 1.2 A rule's WIDTH is uniform; whether it EXISTS is still contract-driven **[contract]**
Uniform thickness (1.1) changes how thick a rule is, never whether we draw one.
A stroke with `gray ≈ 0.8509` (predicted tone ≈ 217) is near-invisible **grey
decoration**, not a black rule — we draw nothing there. Never paint a black
border where the contract shows only a grey fill. The raster localizer compares
ink *presence* and cannot tell grey decoration from a black rule; the contract's
`gray` field can. This trap produced false "missing rule" findings on 1701,
1601C and 1701Q. **Width tells you how thick; only grey tells you whether it's
there.**

### 1.3 Grey fill bands fill their whole row **[contract]**
An official grey band (`fill_regions`, `gray ≈ 0.85`) covers its row's full
height — no thin white stripe above or below. If our band is shorter than its
container, it reads as a stripe. Match the contract's band `y` extent; don't
just overflow. (2550Q Part I TIN row.)

---

## 2. Character-box combs (charboxes)

### 2.1 No divider on the right-most cell **[engineering]**
Draw comb dividers with `:not(:last-child)`:
```css
.<form> .comb-value > span:not(:last-child)::after { /* divider */ }
```
Drawing `::after` on every span puts the last tick on the field's own box
border → a doubled, ugly line. **Exception:** keep the last divider only where
it is genuinely the boundary with a *following* element (a separator column, an
adjacent comb) rather than the box edge — e.g. the whole-peso comb's last cell
sits against the decimal-separator column. Check the render: flush-to-box-border
→ drop it; followed-by-element → keep it.

### 2.2 Empty guided cells still show their guides **[contract]**
CLAUDE.md: *"Empty, short, and exact-capacity values retain every official
guide."* An empty comb must still draw its divider ticks — an adaptive-guide
component that hides guides when the value is empty is a bug (2550Q Part III
Number/Date columns). Confirm the official draws guides in the empty cell first;
if it genuinely leaves it blank, render plain.

### 2.3 Cell count comes from the contract's measured dividers **[contract]**
Never guess a comb's capacity or copy another form's. Count
`interior_divider_x_pt` in the contract (N dividers → N+1 cells). Wrong counts
are taxpayer-facing (they decide how many characters the form accepts) and
invisible to every pixel metric. Corrected counts this project: 1702MX
Registered Name 48→24, 1601C Withholding Agent's Name 38→26, 1702MX Email
40→32, many contact numbers 12→11.

### 2.4 Overflow to a plain field, never truncate **[contract]**
When a value exceeds the cell count, fall back to a single plain text field in
the same footprint — never truncate. Reuse the existing charbox→plain adaptive
component; do not invent a new one. Test four states for every adaptive field:
**empty, short, exact-capacity, capacity-plus-one** (the last proves it degrades
to plain without truncating).

### 2.5 Numeric fields zero-pad to their cell count **[contract]**
The forms' own convention: RDO "018", month "06", sheets "02". A single-digit
value in a 2-cell comb renders "02", not "_2". Use `.padStart(cells, "0")`; keep
right alignment; the >count case still hits the overflow ladder (2.4).

### 2.6 A cell the official leaves un-divided stays un-divided **[contract]**
Some fields are one undivided box even if a value spans two character positions
(0619F item 5 Tax Type Code: one 28.8pt cell holding two chars). Keep it
undivided, no interior tick; center the glyphs per half-cell (see 4.3), do not
add a divider.

### 2.7 Merged / non-applicable cells **[contract]**
Where the official omits a column divider for a row and continues the grey band
through it, MERGE that cell into the neighbour (grid `span 2`, no divider) and
carry the grey — otherwise the label gets shrunk into a narrow cell (2550Q row
29 Tax Debit Memo Drawee cell merges into Particulars). Grey/merged
non-applicable cells never receive comb guides.

---

## 3. Grey fills, white knockouts, and check boxes

### 3.1 Bands are grey; entry boxes are white knockouts **[contract]**
Official forms paint grey **label/option bands** but knock the actual entry
elements — **check boxes and character cells** — out to **white**. Our elements
must not inherit the grey band. The base `.check-box` often sets only a border
and no background, so its interior shows the band through it. Give check-box
interiors and entry cells `background: #fff` **where the contract's
`knockout_regions` shows white**, and leave the surrounding band grey where
`fill_regions` shows grey. Fixed on 1601C (items 2/3/11/13), 0619F (items
3/4/11), 2551Q (item 24 overpayment). **Verify per element** — do not blanket
"make everything white": on 0619F the Part II ATC column is genuinely grey
(a `fill_region` with no knockout), so it stays grey.

### 3.2 The money comb's decimal-separator column is grey **[contract]**
The narrow column between the whole-peso cells and the centavos is a grey
`fill_region` (`gray ≈ 0.65`, `#a6a6a6`) on the official form. Draw it grey, full
height, matching the polished amount rows.

### 3.3 Check-box dimensions are not always square **[contract]**
Read the contract's `checkbox_candidates`. Several forms use wider-than-tall
boxes (1601C header boxes 13.4×12.2pt); a 14×14 square reads visibly too large.

---

## 4. Money / amount fields

### 4.1 The specificity trap — money grid silently collapses to flex **[engineering]**
**This bit us three times (0605, 1601C page 2, 2550Q Part III).** A row rule like
`.<row> > span { display: flex; padding: … }` (specificity 0,1,1) also matches
the money component's `<span class="money">` and beats `.money { display: grid }`
(0,1,0), turning the amount box into a flex row. The grid template goes inert,
the decimal point drifts to ~25% instead of ~88%, and the separator/centavos
cells get crushed. **Fix:** scope the row rule to the label only —
`.<row> > span:first-child { … }` or `.<row> > .money-cell:not(...)` — or reset
the money element explicitly. Whenever a money field looks misaligned, suspect a
`> span` rule beating the grid *first*.

### 4.2 Decimal point position and cents inset **[contract]**
The decimal separator sits at ~88% of the amount box on these forms (measure the
official). Two-digit centavos should be inset ~3.75pt (≈5px) from the box's right
border so they sit near the decimal, not flush to the edge — add the padding
*inside* the fixed fraction track so it doesn't move the separator.

### 4.3 Amount rows across a Part use one shared comb geometry **[contract]**
All amount combs in a section share the same cell widths (whole / separator /
centavos). If one row differs (usually via 4.1), it looks broken next to the
others. Make them identical; take pitch from the contract.

---

## 5. Static text and glyphs

### 5.1 No arrow / pointer glyphs as text **[contract]**
The official forms draw their `►`-style pointers as **vector marks**, not text.
`text_runs` in the contract contain **zero** arrow characters. Rendering `►` as a
character is our invention and pollutes the static-text inventory. Do not add
them; if inherited, remove them (0605 had nine). Keep any inline-block spacer
they occupied so label widths and grid tracks don't shift.

### 5.2 Pre-printed constants are static text, not taxpayer input **[contract]**
Some values live on the printed form before anyone fills it (1601C item 5 ATC
"WW010", 9.96pt bold in the contract's `text_runs`; the domain constrains it to
exactly that value). Render them as static text at the official size/weight —
**not** through the value/blanking path, or they vanish under blank comparison
and no content assertion binds them. Keep a dedicated
`PRE_PRINTED_CONSTANT_SELECTORS` list so blank comparison shows them (making the
comparison *stricter*, never weaker).

### 5.3 Column-header reference numbers go on their own line **[contract]**
Schedule/table column headers carry a small reference number (1,2,3…). Put it on
its **own line below** the header text, not crammed after the last word (1601C
page-2 Schedule I had a bold "3" landing on "Agency"). A two-row grid per header
cell (text row + number row) is robust inside a fixed-height band; check the
contract's `y` positions for the official separation and watch for clipping.

### 5.4 Static text is exhaustively pinned **[contract]**
Every printed string is asserted, in order, against the contract. Adding a
`<br/>` or wrapper can change the collapsed `innerText`; reconcile with the
official reading and update the inventory *with the reason*, never by weakening
or deleting an assertion.

---

## 6. Layout and captions

### 6.1 Absolutely-positioned labels overlap neighbours **[engineering]**
A caption at `top: -Npt` or `left: -Npt` can land outside its cell and over a
divider or the next column (0605 item 22B tag over a divider; item-20
"Add: Penalties" into "Surcharge"; item-5 "Attached" over a charbox). Position
against the contract's `text_run` bbox, and after moving a caption re-check the
band for a new overlap or a clip.

### 6.2 "(specify)" free-text fields use the available width **[contract]**
Widen a specify field to the official knockout extent — up to but not touching
the amount comb on its right — with ~2px left padding. Don't leave it a short box
drifting toward the comb (1601C items 20/29), and don't widen past the official
extent.

### 6.3 Paired captions wrap symmetrically **[contract]**
Where the form has a left/right caption pair (signature blocks), both use the
same line count and fill their cell — don't let one be two lines and the other
cramped (1601C "For Non-Individual" caption).

### 6.4 Signature areas have a rule to sign on **[contract]**
Add the signature line (a `border-top` on the caption strip) where the contract
shows a real black rule (`gray ≈ 0.0`); at the uniform 1px width.

### 6.5 Item numbers sit beside their amount box **[contract]**
Some forms print the item number immediately left of the amount box (0605 items
19, 21). Match the official small-caption style and position from `text_runs`.

---

## 7. TIN format **[owner]**
Render TIN as **14 character cells + 3 greyed separators** (`000-000-000-00000`,
the modern 5-digit-branch format), even where a pinned older revision shows 12.
This is a deliberate divergence from those revisions — record it in the form's
commit and any field-guide inventory so it isn't "corrected" back to 12.
Separators are greyed cells, not character positions, and the box is full width
with 2px side padding.

---

## 8. Verification discipline (applies to every change)

1. Confirm the candidate against the contract **before** editing.
2. Fix in the narrowest owning layer (form-scoped CSS/TSX).
3. Run the form's spec after **every** change. The only permitted failure is the
   known `matches the complete official page(s)` complete-page gate (unreachable
   by proof).
4. A **clipping** failure (`renders every fixture as stable unclipped pages`) is
   never acceptable — it cuts off a taxpayer's data. Revert that change.
5. Re-cut the crops and **look again**. If the defect isn't visibly gone, the fix
   didn't work regardless of the numbers.
6. Never weaken or delete an assertion to make it pass. Re-pinning an assertion
   to a deliberate design change (e.g. uniform border width per §1.1) is
   legitimate *only* with the reason recorded in the test.
7. One form at a time — the visual tooling shares a dev-server port; concurrent
   runs collide (this once took the suite from 99 passed to 74).

---

## Quick checklist for a newly generated form

- [ ] All borders 1 CSS px, no double-width, no leftover compensations (§1.1)
- [ ] No black rule where the contract shows grey decoration (§1.2)
- [ ] Grey bands fill their full row height (§1.3)
- [ ] Comb dividers use `:not(:last-child)`; last tick doesn't double a border (§2.1)
- [ ] Empty combs still show guides (§2.2); counts from the contract (§2.3)
- [ ] Charbox→plain overflow with 4-state tests (§2.4); numerics zero-padded (§2.5)
- [ ] Check-box and entry-cell interiors white per `knockout_regions`; bands grey (§3.1)
- [ ] Check-box dimensions from the contract, not assumed square (§3.3)
- [ ] Money boxes are grid, not collapsed to flex by a `> span` rule (§4.1)
- [ ] Decimal at official position, cents inset, separator column grey (§3.2, §4.2)
- [ ] No `►` glyphs as text (§5.1); pre-printed constants as static text (§5.2)
- [ ] Column-header reference numbers on their own line (§5.3)
- [ ] No caption overlaps; specify fields use available width (§6.1, §6.2)
- [ ] Signature rule present at 1px (§6.4); item numbers beside amount boxes (§6.5)
- [ ] TIN as 14 cells + 3 separators (§7)
