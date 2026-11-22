# PR #260 real-window shots: gated boxes (PR 7) and TIN grid (PR 8)

268 PNGs, macOS windows via `just nsh-shots`, demo taxpayer, dry-run env.
BEFORE gated = `feat/form-fields-combined` (1399047b), BEFORE TIN = `feat/form-fields-gated` (28414cba), AFTER = `feat/form-fields-all` (da972662).
Name: `<area>-<before|after>-<scene>-<width>x<height>-<light|dark>.png`.

- Pictures are 1x (this machine's display is 1600x1200, not Retina); the evidence branch's are 2x.
- Taken with uncommitted copies of `ffgated.test.tsx` / `fftin.test.tsx` that only change the sizes and add scroll/waits; BEFORE gated also needed `FormAnyShot.tsx`'s `openParts`/`edits` params copied in. Every step ran on the BEFORE trees.
- Existing AFTER pictures on `evidence/form-fields-shots` no longer match da972662 (gated ones show the old dashed two-row TIN and old choice lists; TIN ones show gated rows still drawn), so the AFTER files here replace them.
- Not produced: the tall row-height shots (`g1h-*` 420x2000 / 640x1400, `rowh` x1400) because a window cannot be taller than this 1200 pt display.
- The floating pill covering the last row at the window's foot is in every picture and is not listed as a failure.

| File | Shows | Visual failure |
|---|---|---|
| gated-after-0605-17-approved-1280x900-dark.png | 0605 item 17 = Deficiency Tax: page top and the "Not on the printed sheet" approval pair | none |
| gated-after-0605-17-approved-1280x900-light.png | 0605 item 17 = Deficiency Tax: page top and the "Not on the printed sheet" approval pair | none |
| gated-after-0605-17-approved-1440x900-dark.png | 0605 item 17 = Deficiency Tax: page top and the "Not on the printed sheet" approval pair | none |
| gated-after-0605-17-approved-1440x900-light.png | 0605 item 17 = Deficiency Tax: page top and the "Not on the printed sheet" approval pair | none |
| gated-after-0605-17-approved-390x844-dark.png | 0605 item 17 = Deficiency Tax: page top and the "Not on the printed sheet" approval pair | window buttons sit on the page eyebrow text |
| gated-after-0605-17-approved-390x844-light.png | 0605 item 17 = Deficiency Tax: page top and the "Not on the printed sheet" approval pair | window buttons sit on the page eyebrow text |
| gated-after-0605-17-approved-420x900-dark.png | 0605 item 17 = Deficiency Tax: page top and the "Not on the printed sheet" approval pair | window buttons sit on the page eyebrow text |
| gated-after-0605-17-approved-420x900-light.png | 0605 item 17 = Deficiency Tax: page top and the "Not on the printed sheet" approval pair | window buttons sit on the page eyebrow text |
| gated-after-0605-17-closed-1280x900-dark.png | 0605 Part I, item 17 Manner of Payment not picked: Specify box and item 18 installments box — both boxes absent | none |
| gated-after-0605-17-closed-1280x900-light.png | 0605 Part I, item 17 Manner of Payment not picked: Specify box and item 18 installments box — both boxes absent | none |
| gated-after-0605-17-closed-1440x900-dark.png | 0605 Part I, item 17 Manner of Payment not picked: Specify box and item 18 installments box — both boxes absent | none |
| gated-after-0605-17-closed-1440x900-light.png | 0605 Part I, item 17 Manner of Payment not picked: Specify box and item 18 installments box — both boxes absent | none |
| gated-after-0605-17-closed-390x844-dark.png | 0605 Part I, item 17 Manner of Payment not picked: Specify box and item 18 installments box — both boxes absent | none |
| gated-after-0605-17-closed-390x844-light.png | 0605 Part I, item 17 Manner of Payment not picked: Specify box and item 18 installments box — both boxes absent | none |
| gated-after-0605-17-closed-420x900-dark.png | 0605 Part I, item 17 Manner of Payment not picked: Specify box and item 18 installments box — both boxes absent | none |
| gated-after-0605-17-closed-420x900-light.png | 0605 Part I, item 17 Manner of Payment not picked: Specify box and item 18 installments box — both boxes absent | none |
| gated-after-0605-17-others-1280x900-dark.png | 0605 item 17 = Others (Specify): Specify box open with its required-sentence — installments box absent | "Partial Payment" looks darker: pointer hover, nothing is picked |
| gated-after-0605-17-others-1280x900-light.png | 0605 item 17 = Others (Specify): Specify box open with its required-sentence — installments box absent | "Partial Payment" looks darker: pointer hover, nothing is picked |
| gated-after-0605-17-others-1440x900-dark.png | 0605 item 17 = Others (Specify): Specify box open with its required-sentence — installments box absent | "Partial Payment" looks darker: pointer hover, nothing is picked |
| gated-after-0605-17-others-1440x900-light.png | 0605 item 17 = Others (Specify): Specify box open with its required-sentence — installments box absent | "Partial Payment" looks darker: pointer hover, nothing is picked |
| gated-after-0605-17-others-390x844-dark.png | 0605 item 17 = Others (Specify): Specify box open with its required-sentence — installments box absent | none |
| gated-after-0605-17-others-390x844-light.png | 0605 item 17 = Others (Specify): Specify box open with its required-sentence — installments box absent | none |
| gated-after-0605-17-others-420x900-dark.png | 0605 item 17 = Others (Specify): Specify box open with its required-sentence — installments box absent | none |
| gated-after-0605-17-others-420x900-light.png | 0605 item 17 = Others (Specify): Specify box open with its required-sentence — installments box absent | none |
| gated-after-0619e-14-closed-1280x900-dark.png | 0619E Part II before Item 4 is answered: item 14 (Amount of Remittance) — row absent, part says 7 items | none |
| gated-after-0619e-14-closed-1280x900-light.png | 0619E Part II before Item 4 is answered: item 14 (Amount of Remittance) — row absent, part says 7 items | none |
| gated-after-0619e-14-closed-1440x900-dark.png | 0619E Part II before Item 4 is answered: item 14 (Amount of Remittance) — row absent, part says 7 items | none |
| gated-after-0619e-14-closed-1440x900-light.png | 0619E Part II before Item 4 is answered: item 14 (Amount of Remittance) — row absent, part says 7 items | none |
| gated-after-0619e-14-closed-390x844-dark.png | 0619E Part II before Item 4 is answered: item 14 (Amount of Remittance) — row absent, part says 7 items | none |
| gated-after-0619e-14-closed-390x844-light.png | 0619E Part II before Item 4 is answered: item 14 (Amount of Remittance) — row absent, part says 7 items | none |
| gated-after-0619e-14-closed-420x900-dark.png | 0619E Part II before Item 4 is answered: item 14 (Amount of Remittance) — row absent, part says 7 items | none |
| gated-after-0619e-14-closed-420x900-light.png | 0619E Part II before Item 4 is answered: item 14 (Amount of Remittance) — row absent, part says 7 items | none |
| gated-after-0619e-14-open-1280x900-dark.png | 0619E after Item 4 = Yes: item 14 open with 1,250.50 typed, totals computed | none |
| gated-after-0619e-14-open-1280x900-light.png | 0619E after Item 4 = Yes: item 14 open with 1,250.50 typed, totals computed | none |
| gated-after-0619e-14-open-1440x900-dark.png | 0619E after Item 4 = Yes: item 14 open with 1,250.50 typed, totals computed | none |
| gated-after-0619e-14-open-1440x900-light.png | 0619E after Item 4 = Yes: item 14 open with 1,250.50 typed, totals computed | none |
| gated-after-0619e-14-open-390x844-dark.png | 0619E after Item 4 = Yes: item 14 open with 1,250.50 typed, totals computed | none |
| gated-after-0619e-14-open-390x844-light.png | 0619E after Item 4 = Yes: item 14 open with 1,250.50 typed, totals computed | none |
| gated-after-0619e-14-open-420x900-dark.png | 0619E after Item 4 = Yes: item 14 open with 1,250.50 typed, totals computed | none |
| gated-after-0619e-14-open-420x900-light.png | 0619E after Item 4 = Yes: item 14 open with 1,250.50 typed, totals computed | none |
| gated-after-0619e-14-refused-1280x900-dark.png | 0619E refused write on shut item 14: box drawn with "Item 14 is used when Item 4 is Yes." | none |
| gated-after-0619e-14-refused-1280x900-light.png | 0619E refused write on shut item 14: box drawn with "Item 14 is used when Item 4 is Yes." | none |
| gated-after-0619e-14-refused-1440x900-dark.png | 0619E refused write on shut item 14: box drawn with "Item 14 is used when Item 4 is Yes." | none |
| gated-after-0619e-14-refused-1440x900-light.png | 0619E refused write on shut item 14: box drawn with "Item 14 is used when Item 4 is Yes." | none |
| gated-after-0619e-14-refused-390x844-dark.png | 0619E refused write on shut item 14: box drawn with "Item 14 is used when Item 4 is Yes." | none |
| gated-after-0619e-14-refused-390x844-light.png | 0619E refused write on shut item 14: box drawn with "Item 14 is used when Item 4 is Yes." | none |
| gated-after-0619e-14-refused-420x900-dark.png | 0619E refused write on shut item 14: box drawn with "Item 14 is used when Item 4 is Yes." | none |
| gated-after-0619e-14-refused-420x900-light.png | 0619E refused write on shut item 14: box drawn with "Item 14 is used when Item 4 is Yes." | none |
| gated-after-0619e-14-shut-1280x900-dark.png | 0619E after Item 4 back to No: item 14 shut again — row absent, part says 7 items | none |
| gated-after-0619e-14-shut-1280x900-light.png | 0619E after Item 4 back to No: item 14 shut again — row absent, part says 7 items | none |
| gated-after-0619e-14-shut-1440x900-dark.png | 0619E after Item 4 back to No: item 14 shut again — row absent, part says 7 items | none |
| gated-after-0619e-14-shut-1440x900-light.png | 0619E after Item 4 back to No: item 14 shut again — row absent, part says 7 items | none |
| gated-after-0619e-14-shut-390x844-dark.png | 0619E after Item 4 back to No: item 14 shut again — row absent, part says 7 items | none |
| gated-after-0619e-14-shut-390x844-light.png | 0619E after Item 4 back to No: item 14 shut again — row absent, part says 7 items | none |
| gated-after-0619e-14-shut-420x900-dark.png | 0619E after Item 4 back to No: item 14 shut again — row absent, part says 7 items | none |
| gated-after-0619e-14-shut-420x900-light.png | 0619E after Item 4 back to No: item 14 shut again — row absent, part says 7 items | none |
| gated-after-1700-part2-closed-1280x900-dark.png | 1700 Part II (spouse) on a fresh return, nothing answered — part folded to "0 items" (page top shown) | none |
| gated-after-1700-part2-closed-1280x900-light.png | 1700 Part II (spouse) on a fresh return, nothing answered — part folded to "0 items" (page top shown) | none |
| gated-after-1700-part2-closed-1440x900-dark.png | 1700 Part II (spouse) on a fresh return, nothing answered — part folded to "0 items" (page top shown) | none |
| gated-after-1700-part2-closed-1440x900-light.png | 1700 Part II (spouse) on a fresh return, nothing answered — part folded to "0 items" (page top shown) | none |
| gated-after-1700-part2-closed-390x844-dark.png | 1700 Part II (spouse) on a fresh return, nothing answered — part folded to "0 items" (page top shown) | window buttons sit on the page eyebrow text |
| gated-after-1700-part2-closed-390x844-light.png | 1700 Part II (spouse) on a fresh return, nothing answered — part folded to "0 items" (page top shown) | window buttons sit on the page eyebrow text |
| gated-after-1700-part2-closed-420x900-dark.png | 1700 Part II (spouse) on a fresh return, nothing answered — part folded to "0 items" (page top shown) | window buttons sit on the page eyebrow text |
| gated-after-1700-part2-closed-420x900-light.png | 1700 Part II (spouse) on a fresh return, nothing answered — part folded to "0 items" (page top shown) | window buttons sit on the page eyebrow text |
| gated-after-1700-part2-full-1280x900-dark.png | 1700 Part II after item 24 = Yes: item 25 open too | none |
| gated-after-1700-part2-full-1280x900-light.png | 1700 Part II after item 24 = Yes: item 25 open too | none |
| gated-after-1700-part2-full-1440x900-dark.png | 1700 Part II after item 24 = Yes: item 25 open too | none |
| gated-after-1700-part2-full-1440x900-light.png | 1700 Part II after item 24 = Yes: item 25 open too | none |
| gated-after-1700-part2-full-390x844-dark.png | 1700 Part II after item 24 = Yes: item 25 open too | none |
| gated-after-1700-part2-full-390x844-light.png | 1700 Part II after item 24 = Yes: item 25 open too | none |
| gated-after-1700-part2-full-420x900-dark.png | 1700 Part II after item 24 = Yes: item 25 open too | none |
| gated-after-1700-part2-full-420x900-light.png | 1700 Part II after item 24 = Yes: item 25 open too | none |
| gated-after-1700-part2-open-1280x900-dark.png | 1700 Part II after Married + spouse income Yes + Joint Filing: items 18-24 open, 25 still gated — item 25 absent, part says 7 items | none |
| gated-after-1700-part2-open-1280x900-light.png | 1700 Part II after Married + spouse income Yes + Joint Filing: items 18-24 open, 25 still gated — item 25 absent, part says 7 items | none |
| gated-after-1700-part2-open-1440x900-dark.png | 1700 Part II after Married + spouse income Yes + Joint Filing: items 18-24 open, 25 still gated — item 25 absent, part says 7 items | none |
| gated-after-1700-part2-open-1440x900-light.png | 1700 Part II after Married + spouse income Yes + Joint Filing: items 18-24 open, 25 still gated — item 25 absent, part says 7 items | none |
| gated-after-1700-part2-open-390x844-dark.png | 1700 Part II after Married + spouse income Yes + Joint Filing: items 18-24 open, 25 still gated — item 25 absent, part says 7 items | none |
| gated-after-1700-part2-open-390x844-light.png | 1700 Part II after Married + spouse income Yes + Joint Filing: items 18-24 open, 25 still gated — item 25 absent, part says 7 items | none |
| gated-after-1700-part2-open-420x900-dark.png | 1700 Part II after Married + spouse income Yes + Joint Filing: items 18-24 open, 25 still gated — item 25 absent, part says 7 items | none |
| gated-after-1700-part2-open-420x900-light.png | 1700 Part II after Married + spouse income Yes + Joint Filing: items 18-24 open, 25 still gated — item 25 absent, part says 7 items | none |
| gated-after-2551q-12-closed-1280x900-dark.png | 2551Q fresh return, page top (item 12 starts at the fold / under the pill) | none |
| gated-after-2551q-12-closed-1280x900-light.png | 2551Q fresh return, page top (item 12 starts at the fold / under the pill) | none |
| gated-after-2551q-12-closed-1440x900-dark.png | 2551Q fresh return, page top (item 12 starts at the fold / under the pill) | none |
| gated-after-2551q-12-closed-1440x900-light.png | 2551Q fresh return, page top (item 12 starts at the fold / under the pill) | none |
| gated-after-2551q-12-closed-390x844-dark.png | 2551Q fresh return, page top (item 12 starts at the fold / under the pill) | window buttons sit on the page eyebrow text |
| gated-after-2551q-12-closed-390x844-light.png | 2551Q fresh return, page top (item 12 starts at the fold / under the pill) | window buttons sit on the page eyebrow text |
| gated-after-2551q-12-closed-420x900-dark.png | 2551Q fresh return, page top (item 12 starts at the fold / under the pill) | window buttons sit on the page eyebrow text |
| gated-after-2551q-12-closed-420x900-light.png | 2551Q fresh return, page top (item 12 starts at the fold / under the pill) | window buttons sit on the page eyebrow text |
| gated-after-2551q-12-open-1280x900-dark.png | 2551Q item 12 = Yes: "If yes, specify" picker open with its required-sentence | none |
| gated-after-2551q-12-open-1280x900-light.png | 2551Q item 12 = Yes: "If yes, specify" picker open with its required-sentence | none |
| gated-after-2551q-12-open-1440x900-dark.png | 2551Q item 12 = Yes: "If yes, specify" picker open with its required-sentence | none |
| gated-after-2551q-12-open-1440x900-light.png | 2551Q item 12 = Yes: "If yes, specify" picker open with its required-sentence | none |
| gated-after-2551q-12-open-390x844-dark.png | 2551Q item 12 = Yes: "If yes, specify" picker open with its required-sentence | none |
| gated-after-2551q-12-open-390x844-light.png | 2551Q item 12 = Yes: "If yes, specify" picker open with its required-sentence | none |
| gated-after-2551q-12-open-420x900-dark.png | 2551Q item 12 = Yes: "If yes, specify" picker open with its required-sentence | none |
| gated-after-2551q-12-open-420x900-light.png | 2551Q item 12 = Yes: "If yes, specify" picker open with its required-sentence | none |
| gated-after-2551q-12-shut-1280x900-dark.png | 2551Q item 12 back to No: specify picker shut — specify box absent | none |
| gated-after-2551q-12-shut-1280x900-light.png | 2551Q item 12 back to No: specify picker shut — specify box absent | none |
| gated-after-2551q-12-shut-1440x900-dark.png | 2551Q item 12 back to No: specify picker shut — specify box absent | none |
| gated-after-2551q-12-shut-1440x900-light.png | 2551Q item 12 back to No: specify picker shut — specify box absent | none |
| gated-after-2551q-12-shut-390x844-dark.png | 2551Q item 12 back to No: specify picker shut — specify box absent | none |
| gated-after-2551q-12-shut-390x844-light.png | 2551Q item 12 back to No: specify picker shut — specify box absent | none |
| gated-after-2551q-12-shut-420x900-dark.png | 2551Q item 12 back to No: specify picker shut — specify box absent | none |
| gated-after-2551q-12-shut-420x900-light.png | 2551Q item 12 back to No: specify picker shut — specify box absent | none |
| gated-before-0605-17-approved-1280x900-dark.png | 0605 item 17 = Deficiency Tax: page top and the "Not on the printed sheet" approval pair | none |
| gated-before-0605-17-approved-1280x900-light.png | 0605 item 17 = Deficiency Tax: page top and the "Not on the printed sheet" approval pair | none |
| gated-before-0605-17-approved-1440x900-dark.png | 0605 item 17 = Deficiency Tax: page top and the "Not on the printed sheet" approval pair | none |
| gated-before-0605-17-approved-1440x900-light.png | 0605 item 17 = Deficiency Tax: page top and the "Not on the printed sheet" approval pair | none |
| gated-before-0605-17-approved-390x844-dark.png | 0605 item 17 = Deficiency Tax: page top and the "Not on the printed sheet" approval pair | window buttons sit on the page eyebrow text |
| gated-before-0605-17-approved-390x844-light.png | 0605 item 17 = Deficiency Tax: page top and the "Not on the printed sheet" approval pair | window buttons sit on the page eyebrow text |
| gated-before-0605-17-approved-420x900-dark.png | 0605 item 17 = Deficiency Tax: page top and the "Not on the printed sheet" approval pair | window buttons sit on the page eyebrow text |
| gated-before-0605-17-approved-420x900-light.png | 0605 item 17 = Deficiency Tax: page top and the "Not on the printed sheet" approval pair | window buttons sit on the page eyebrow text |
| gated-before-0605-17-closed-1280x900-dark.png | 0605 Part I, item 17 Manner of Payment not picked: Specify box and item 18 installments box — Specify box and installments box drawn locked | FAIL invisible: locked "No. of installments" box has no visible outline |
| gated-before-0605-17-closed-1280x900-light.png | 0605 Part I, item 17 Manner of Payment not picked: Specify box and item 18 installments box — Specify box and installments box drawn locked | none |
| gated-before-0605-17-closed-1440x900-dark.png | 0605 Part I, item 17 Manner of Payment not picked: Specify box and item 18 installments box — Specify box and installments box drawn locked | FAIL invisible: locked "No. of installments" box has no visible outline |
| gated-before-0605-17-closed-1440x900-light.png | 0605 Part I, item 17 Manner of Payment not picked: Specify box and item 18 installments box — Specify box and installments box drawn locked | none |
| gated-before-0605-17-closed-390x844-dark.png | 0605 Part I, item 17 Manner of Payment not picked: Specify box and item 18 installments box — Specify box and installments box drawn locked | FAIL overlap: hover tooltip "Needs item 18 = Installment" covers the item 18 label |
| gated-before-0605-17-closed-390x844-light.png | 0605 Part I, item 17 Manner of Payment not picked: Specify box and item 18 installments box — Specify box and installments box drawn locked | FAIL overlap: hover tooltip "Needs item 18 = Installment" covers the item 18 label |
| gated-before-0605-17-closed-420x900-dark.png | 0605 Part I, item 17 Manner of Payment not picked: Specify box and item 18 installments box — Specify box and installments box drawn locked | none |
| gated-before-0605-17-closed-420x900-light.png | 0605 Part I, item 17 Manner of Payment not picked: Specify box and item 18 installments box — Specify box and installments box drawn locked | none |
| gated-before-0605-17-others-1280x900-dark.png | 0605 item 17 = Others (Specify): Specify box open with its required-sentence — installments box still drawn locked | FAIL invisible: locked "No. of installments" box has no visible outline |
| gated-before-0605-17-others-1280x900-light.png | 0605 item 17 = Others (Specify): Specify box open with its required-sentence — installments box still drawn locked | none |
| gated-before-0605-17-others-1440x900-dark.png | 0605 item 17 = Others (Specify): Specify box open with its required-sentence — installments box still drawn locked | FAIL invisible: locked "No. of installments" box has no visible outline |
| gated-before-0605-17-others-1440x900-light.png | 0605 item 17 = Others (Specify): Specify box open with its required-sentence — installments box still drawn locked | none |
| gated-before-0605-17-others-390x844-dark.png | 0605 item 17 = Others (Specify): Specify box open with its required-sentence — installments box still drawn locked | none |
| gated-before-0605-17-others-390x844-light.png | 0605 item 17 = Others (Specify): Specify box open with its required-sentence — installments box still drawn locked | none |
| gated-before-0605-17-others-420x900-dark.png | 0605 item 17 = Others (Specify): Specify box open with its required-sentence — installments box still drawn locked | none |
| gated-before-0605-17-others-420x900-light.png | 0605 item 17 = Others (Specify): Specify box open with its required-sentence — installments box still drawn locked | none |
| gated-before-0619e-14-closed-1280x900-dark.png | 0619E Part II before Item 4 is answered: item 14 (Amount of Remittance) — drawn locked with "Needs item 4 = Yes" | none |
| gated-before-0619e-14-closed-1280x900-light.png | 0619E Part II before Item 4 is answered: item 14 (Amount of Remittance) — drawn locked with "Needs item 4 = Yes" | none |
| gated-before-0619e-14-closed-1440x900-dark.png | 0619E Part II before Item 4 is answered: item 14 (Amount of Remittance) — drawn locked with "Needs item 4 = Yes" | none |
| gated-before-0619e-14-closed-1440x900-light.png | 0619E Part II before Item 4 is answered: item 14 (Amount of Remittance) — drawn locked with "Needs item 4 = Yes" | none |
| gated-before-0619e-14-closed-390x844-dark.png | 0619E Part II before Item 4 is answered: item 14 (Amount of Remittance) — drawn locked with "Needs item 4 = Yes" | none |
| gated-before-0619e-14-closed-390x844-light.png | 0619E Part II before Item 4 is answered: item 14 (Amount of Remittance) — drawn locked with "Needs item 4 = Yes" | none |
| gated-before-0619e-14-closed-420x900-dark.png | 0619E Part II before Item 4 is answered: item 14 (Amount of Remittance) — drawn locked with "Needs item 4 = Yes" | none |
| gated-before-0619e-14-closed-420x900-light.png | 0619E Part II before Item 4 is answered: item 14 (Amount of Remittance) — drawn locked with "Needs item 4 = Yes" | none |
| gated-before-0619e-14-open-1280x900-dark.png | 0619E after Item 4 = Yes: item 14 open with 1,250.50 typed, totals computed | none |
| gated-before-0619e-14-open-1280x900-light.png | 0619E after Item 4 = Yes: item 14 open with 1,250.50 typed, totals computed | none |
| gated-before-0619e-14-open-1440x900-dark.png | 0619E after Item 4 = Yes: item 14 open with 1,250.50 typed, totals computed | none |
| gated-before-0619e-14-open-1440x900-light.png | 0619E after Item 4 = Yes: item 14 open with 1,250.50 typed, totals computed | none |
| gated-before-0619e-14-open-390x844-dark.png | 0619E after Item 4 = Yes: item 14 open with 1,250.50 typed, totals computed | none |
| gated-before-0619e-14-open-390x844-light.png | 0619E after Item 4 = Yes: item 14 open with 1,250.50 typed, totals computed | none |
| gated-before-0619e-14-open-420x900-dark.png | 0619E after Item 4 = Yes: item 14 open with 1,250.50 typed, totals computed | none |
| gated-before-0619e-14-open-420x900-light.png | 0619E after Item 4 = Yes: item 14 open with 1,250.50 typed, totals computed | none |
| gated-before-0619e-14-refused-1280x900-dark.png | 0619E refused write on shut item 14: box drawn with "Item 14 is used when Item 4 is Yes." | none |
| gated-before-0619e-14-refused-1280x900-light.png | 0619E refused write on shut item 14: box drawn with "Item 14 is used when Item 4 is Yes." | none |
| gated-before-0619e-14-refused-1440x900-dark.png | 0619E refused write on shut item 14: box drawn with "Item 14 is used when Item 4 is Yes." | none |
| gated-before-0619e-14-refused-1440x900-light.png | 0619E refused write on shut item 14: box drawn with "Item 14 is used when Item 4 is Yes." | none |
| gated-before-0619e-14-refused-390x844-dark.png | 0619E refused write on shut item 14: box drawn with "Item 14 is used when Item 4 is Yes." | none |
| gated-before-0619e-14-refused-390x844-light.png | 0619E refused write on shut item 14: box drawn with "Item 14 is used when Item 4 is Yes." | none |
| gated-before-0619e-14-refused-420x900-dark.png | 0619E refused write on shut item 14: box drawn with "Item 14 is used when Item 4 is Yes." | none |
| gated-before-0619e-14-refused-420x900-light.png | 0619E refused write on shut item 14: box drawn with "Item 14 is used when Item 4 is Yes." | none |
| gated-before-0619e-14-shut-1280x900-dark.png | 0619E after Item 4 back to No: item 14 shut again — drawn locked with "Needs item 4 = Yes" | none |
| gated-before-0619e-14-shut-1280x900-light.png | 0619E after Item 4 back to No: item 14 shut again — drawn locked with "Needs item 4 = Yes" | none |
| gated-before-0619e-14-shut-1440x900-dark.png | 0619E after Item 4 back to No: item 14 shut again — drawn locked with "Needs item 4 = Yes" | none |
| gated-before-0619e-14-shut-1440x900-light.png | 0619E after Item 4 back to No: item 14 shut again — drawn locked with "Needs item 4 = Yes" | none |
| gated-before-0619e-14-shut-390x844-dark.png | 0619E after Item 4 back to No: item 14 shut again — drawn locked with "Needs item 4 = Yes" | none |
| gated-before-0619e-14-shut-390x844-light.png | 0619E after Item 4 back to No: item 14 shut again — drawn locked with "Needs item 4 = Yes" | none |
| gated-before-0619e-14-shut-420x900-dark.png | 0619E after Item 4 back to No: item 14 shut again — drawn locked with "Needs item 4 = Yes" | none |
| gated-before-0619e-14-shut-420x900-light.png | 0619E after Item 4 back to No: item 14 shut again — drawn locked with "Needs item 4 = Yes" | none |
| gated-before-1700-part2-closed-1280x900-dark.png | 1700 Part II (spouse) on a fresh return, nothing answered — all 8 rows drawn locked, each with its "Needs ..." line | none |
| gated-before-1700-part2-closed-1280x900-light.png | 1700 Part II (spouse) on a fresh return, nothing answered — all 8 rows drawn locked, each with its "Needs ..." line | none |
| gated-before-1700-part2-closed-1440x900-dark.png | 1700 Part II (spouse) on a fresh return, nothing answered — all 8 rows drawn locked, each with its "Needs ..." line | none |
| gated-before-1700-part2-closed-1440x900-light.png | 1700 Part II (spouse) on a fresh return, nothing answered — all 8 rows drawn locked, each with its "Needs ..." line | none |
| gated-before-1700-part2-closed-390x844-dark.png | 1700 Part II (spouse) on a fresh return, nothing answered — all 8 rows drawn locked, each with its "Needs ..." line | none |
| gated-before-1700-part2-closed-390x844-light.png | 1700 Part II (spouse) on a fresh return, nothing answered — all 8 rows drawn locked, each with its "Needs ..." line | none |
| gated-before-1700-part2-closed-420x900-dark.png | 1700 Part II (spouse) on a fresh return, nothing answered — all 8 rows drawn locked, each with its "Needs ..." line | none |
| gated-before-1700-part2-closed-420x900-light.png | 1700 Part II (spouse) on a fresh return, nothing answered — all 8 rows drawn locked, each with its "Needs ..." line | none |
| gated-before-1700-part2-full-1280x900-dark.png | 1700 Part II after item 24 = Yes: item 25 open too | none |
| gated-before-1700-part2-full-1280x900-light.png | 1700 Part II after item 24 = Yes: item 25 open too | none |
| gated-before-1700-part2-full-1440x900-dark.png | 1700 Part II after item 24 = Yes: item 25 open too | none |
| gated-before-1700-part2-full-1440x900-light.png | 1700 Part II after item 24 = Yes: item 25 open too | none |
| gated-before-1700-part2-full-390x844-dark.png | 1700 Part II after item 24 = Yes: item 25 open too | none |
| gated-before-1700-part2-full-390x844-light.png | 1700 Part II after item 24 = Yes: item 25 open too | none |
| gated-before-1700-part2-full-420x900-dark.png | 1700 Part II after item 24 = Yes: item 25 open too | none |
| gated-before-1700-part2-full-420x900-light.png | 1700 Part II after item 24 = Yes: item 25 open too | none |
| gated-before-1700-part2-open-1280x900-dark.png | 1700 Part II after Married + spouse income Yes + Joint Filing: items 18-24 open, 25 still gated — item 25 drawn locked with its "Needs ..." line | none |
| gated-before-1700-part2-open-1280x900-light.png | 1700 Part II after Married + spouse income Yes + Joint Filing: items 18-24 open, 25 still gated — item 25 drawn locked with its "Needs ..." line | none |
| gated-before-1700-part2-open-1440x900-dark.png | 1700 Part II after Married + spouse income Yes + Joint Filing: items 18-24 open, 25 still gated — item 25 drawn locked with its "Needs ..." line | none |
| gated-before-1700-part2-open-1440x900-light.png | 1700 Part II after Married + spouse income Yes + Joint Filing: items 18-24 open, 25 still gated — item 25 drawn locked with its "Needs ..." line | none |
| gated-before-1700-part2-open-390x844-dark.png | 1700 Part II after Married + spouse income Yes + Joint Filing: items 18-24 open, 25 still gated — item 25 drawn locked with its "Needs ..." line | none |
| gated-before-1700-part2-open-390x844-light.png | 1700 Part II after Married + spouse income Yes + Joint Filing: items 18-24 open, 25 still gated — item 25 drawn locked with its "Needs ..." line | none |
| gated-before-1700-part2-open-420x900-dark.png | 1700 Part II after Married + spouse income Yes + Joint Filing: items 18-24 open, 25 still gated — item 25 drawn locked with its "Needs ..." line | none |
| gated-before-1700-part2-open-420x900-light.png | 1700 Part II after Married + spouse income Yes + Joint Filing: items 18-24 open, 25 still gated — item 25 drawn locked with its "Needs ..." line | none |
| gated-before-2551q-12-closed-1280x900-dark.png | 2551Q fresh return, page top (item 12 starts at the fold / under the pill) | none |
| gated-before-2551q-12-closed-1280x900-light.png | 2551Q fresh return, page top (item 12 starts at the fold / under the pill) | none |
| gated-before-2551q-12-closed-1440x900-dark.png | 2551Q fresh return, page top (item 12 starts at the fold / under the pill) | none |
| gated-before-2551q-12-closed-1440x900-light.png | 2551Q fresh return, page top (item 12 starts at the fold / under the pill) | none |
| gated-before-2551q-12-closed-390x844-dark.png | 2551Q fresh return, page top (item 12 starts at the fold / under the pill) | window buttons sit on the page eyebrow text |
| gated-before-2551q-12-closed-390x844-light.png | 2551Q fresh return, page top (item 12 starts at the fold / under the pill) | window buttons sit on the page eyebrow text |
| gated-before-2551q-12-closed-420x900-dark.png | 2551Q fresh return, page top (item 12 starts at the fold / under the pill) | window buttons sit on the page eyebrow text |
| gated-before-2551q-12-closed-420x900-light.png | 2551Q fresh return, page top (item 12 starts at the fold / under the pill) | window buttons sit on the page eyebrow text |
| gated-before-2551q-12-open-1280x900-dark.png | 2551Q item 12 = Yes: "If yes, specify" picker open with its required-sentence | none |
| gated-before-2551q-12-open-1280x900-light.png | 2551Q item 12 = Yes: "If yes, specify" picker open with its required-sentence | none |
| gated-before-2551q-12-open-1440x900-dark.png | 2551Q item 12 = Yes: "If yes, specify" picker open with its required-sentence | none |
| gated-before-2551q-12-open-1440x900-light.png | 2551Q item 12 = Yes: "If yes, specify" picker open with its required-sentence | none |
| gated-before-2551q-12-open-390x844-dark.png | 2551Q item 12 = Yes: "If yes, specify" picker open with its required-sentence | none |
| gated-before-2551q-12-open-390x844-light.png | 2551Q item 12 = Yes: "If yes, specify" picker open with its required-sentence | none |
| gated-before-2551q-12-open-420x900-dark.png | 2551Q item 12 = Yes: "If yes, specify" picker open with its required-sentence | none |
| gated-before-2551q-12-open-420x900-light.png | 2551Q item 12 = Yes: "If yes, specify" picker open with its required-sentence | none |
| gated-before-2551q-12-shut-1280x900-dark.png | 2551Q item 12 back to No: specify picker shut — specify box drawn locked with "Needs item 12 = Yes" | none |
| gated-before-2551q-12-shut-1280x900-light.png | 2551Q item 12 back to No: specify picker shut — specify box drawn locked with "Needs item 12 = Yes" | none |
| gated-before-2551q-12-shut-1440x900-dark.png | 2551Q item 12 back to No: specify picker shut — specify box drawn locked with "Needs item 12 = Yes" | none |
| gated-before-2551q-12-shut-1440x900-light.png | 2551Q item 12 back to No: specify picker shut — specify box drawn locked with "Needs item 12 = Yes" | none |
| gated-before-2551q-12-shut-390x844-dark.png | 2551Q item 12 back to No: specify picker shut — specify box drawn locked with "Needs item 12 = Yes" | FAIL overlap: hover tooltip "Needs item 12 = Yes" covers the item 12 label |
| gated-before-2551q-12-shut-390x844-light.png | 2551Q item 12 back to No: specify picker shut — specify box drawn locked with "Needs item 12 = Yes" | FAIL overlap: hover tooltip "Needs item 12 = Yes" covers the item 12 label |
| gated-before-2551q-12-shut-420x900-dark.png | 2551Q item 12 back to No: specify picker shut — specify box drawn locked with "Needs item 12 = Yes" | FAIL overlap: hover tooltip "Needs item 12 = Yes" covers the item 12 label |
| gated-before-2551q-12-shut-420x900-light.png | 2551Q item 12 back to No: specify picker shut — specify box drawn locked with "Needs item 12 = Yes" | FAIL overlap: hover tooltip "Needs item 12 = Yes" covers the item 12 label |
| tin-after-0605-locked-1280x900-dark.png | 0605 item 9, locked profile TIN (000-123-453-00000) — dashed fixed boxes, "From the tax profile" line | none |
| tin-after-0605-locked-1280x900-light.png | 0605 item 9, locked profile TIN (000-123-453-00000) — dashed fixed boxes, "From the tax profile" line | none |
| tin-after-0605-locked-320x700-dark.png | 0605 item 9, locked profile TIN (000-123-453-00000) — four boxes in one full row, no dashes, "From the tax profile" line | FAIL clipped: locked TIN boxes show "0…" and "4…" instead of 000 and 453; window buttons sit on the sticky part title |
| tin-after-0605-locked-320x700-light.png | 0605 item 9, locked profile TIN (000-123-453-00000) — four boxes in one full row, no dashes, "From the tax profile" line | FAIL clipped: locked TIN boxes show "0…" and "4…" instead of 000 and 453; window buttons sit on the sticky part title |
| tin-after-0605-locked-420x900-dark.png | 0605 item 9, locked profile TIN (000-123-453-00000) — four boxes in one full row, no dashes, "From the tax profile" line | none |
| tin-after-0605-locked-420x900-light.png | 0605 item 9, locked profile TIN (000-123-453-00000) — four boxes in one full row, no dashes, "From the tax profile" line | none |
| tin-after-1706-refusal-1280x900-dark.png | 1706 after Check: buyer TIN refusal ("12 items need attention") — dashed fixed boxes, "From the tax profile" line | toast covers part of the next row (item 11), TIN row readable |
| tin-after-1706-refusal-1280x900-light.png | 1706 after Check: buyer TIN refusal ("12 items need attention") — dashed fixed boxes, "From the tax profile" line | toast covers part of the next row (item 11), TIN row readable |
| tin-after-1706-refusal-320x700-dark.png | 1706 after Check: buyer TIN refusal ("12 items need attention") — four boxes in one full row, no dashes, "From the tax profile" line | FAIL clipped: locked TIN boxes show "0…" and "4…" instead of 000 and 453; FAIL overlap: the "12 items need attention" toast and the pill cover the buyer TIN row; its boxes and sentence are not readable; window buttons sit on the sticky part title |
| tin-after-1706-refusal-320x700-light.png | 1706 after Check: buyer TIN refusal ("12 items need attention") — four boxes in one full row, no dashes, "From the tax profile" line | FAIL clipped: locked TIN boxes show "0…" and "4…" instead of 000 and 453; FAIL overlap: the "12 items need attention" toast and the pill cover the buyer TIN row; its boxes and sentence are not readable; window buttons sit on the sticky part title |
| tin-after-1706-refusal-420x900-dark.png | 1706 after Check: buyer TIN refusal ("12 items need attention") — four boxes in one full row, no dashes, "From the tax profile" line | toast covers part of the next row (item 11), TIN row readable |
| tin-after-1706-refusal-420x900-light.png | 1706 after Check: buyer TIN refusal ("12 items need attention") — four boxes in one full row, no dashes, "From the tax profile" line | toast covers part of the next row (item 11), TIN row readable |
| tin-after-1801-typed-empty-1280x900-dark.png | 1801 item 5 locked TIN and item 11 typed TIN (executor), empty — dashed fixed boxes, "From the tax profile" line | none |
| tin-after-1801-typed-empty-1280x900-light.png | 1801 item 5 locked TIN and item 11 typed TIN (executor), empty — dashed fixed boxes, "From the tax profile" line | none |
| tin-after-1801-typed-empty-320x700-dark.png | 1801 item 5 locked TIN and item 11 typed TIN (executor), empty — four boxes in one full row, no dashes, "From the tax profile" line | FAIL clipped: locked TIN boxes show "0…" and "4…" instead of 000 and 453; window buttons sit on the sticky part title |
| tin-after-1801-typed-empty-320x700-light.png | 1801 item 5 locked TIN and item 11 typed TIN (executor), empty — four boxes in one full row, no dashes, "From the tax profile" line | FAIL clipped: locked TIN boxes show "0…" and "4…" instead of 000 and 453; window buttons sit on the sticky part title |
| tin-after-1801-typed-empty-420x900-dark.png | 1801 item 5 locked TIN and item 11 typed TIN (executor), empty — four boxes in one full row, no dashes, "From the tax profile" line | none |
| tin-after-1801-typed-empty-420x900-light.png | 1801 item 5 locked TIN and item 11 typed TIN (executor), empty — four boxes in one full row, no dashes, "From the tax profile" line | none |
| tin-after-1801-typed-filled-1280x900-dark.png | 1801 item 11 typed TIN filled 000 123 453 00001 — dashed fixed boxes, "From the tax profile" line | none |
| tin-after-1801-typed-filled-1280x900-light.png | 1801 item 11 typed TIN filled 000 123 453 00001 — dashed fixed boxes, "From the tax profile" line | none |
| tin-after-1801-typed-filled-320x700-dark.png | 1801 item 11 typed TIN filled 000 123 453 00001 — four boxes in one full row, no dashes, "From the tax profile" line | FAIL clipped: locked TIN boxes show "0…" and "4…" instead of 000 and 453; window buttons sit on the sticky part title |
| tin-after-1801-typed-filled-320x700-light.png | 1801 item 11 typed TIN filled 000 123 453 00001 — four boxes in one full row, no dashes, "From the tax profile" line | FAIL clipped: locked TIN boxes show "0…" and "4…" instead of 000 and 453; window buttons sit on the sticky part title |
| tin-after-1801-typed-filled-420x900-dark.png | 1801 item 11 typed TIN filled 000 123 453 00001 — four boxes in one full row, no dashes, "From the tax profile" line | none |
| tin-after-1801-typed-filled-420x900-light.png | 1801 item 11 typed TIN filled 000 123 453 00001 — four boxes in one full row, no dashes, "From the tax profile" line | none |
| tin-after-2000-onebox-1280x900-dark.png | 2000 item 9 one-box whole TIN | none |
| tin-after-2000-onebox-1280x900-light.png | 2000 item 9 one-box whole TIN | none |
| tin-after-2000-onebox-320x700-dark.png | 2000 item 9 one-box whole TIN | FAIL clipped: locked TIN boxes show "0…" and "4…" instead of 000 and 453; window buttons sit on the sticky part title |
| tin-after-2000-onebox-320x700-light.png | 2000 item 9 one-box whole TIN | FAIL clipped: locked TIN boxes show "0…" and "4…" instead of 000 and 453; window buttons sit on the sticky part title |
| tin-after-2000-onebox-420x900-dark.png | 2000 item 9 one-box whole TIN | none |
| tin-after-2000-onebox-420x900-light.png | 2000 item 9 one-box whole TIN | none |
| tin-before-0605-locked-1280x900-dark.png | 0605 item 9, locked profile TIN (000-123-453-00000) — dashed boxes | none |
| tin-before-0605-locked-1280x900-light.png | 0605 item 9, locked profile TIN (000-123-453-00000) — dashed boxes | none |
| tin-before-0605-locked-320x700-dark.png | 0605 item 9, locked profile TIN (000-123-453-00000) — dashed boxes, wrapped to two rows | window buttons sit on the sticky part title |
| tin-before-0605-locked-320x700-light.png | 0605 item 9, locked profile TIN (000-123-453-00000) — dashed boxes, wrapped to two rows | window buttons sit on the sticky part title |
| tin-before-0605-locked-420x900-dark.png | 0605 item 9, locked profile TIN (000-123-453-00000) — dashed boxes, wrapped to two rows | none |
| tin-before-0605-locked-420x900-light.png | 0605 item 9, locked profile TIN (000-123-453-00000) — dashed boxes, wrapped to two rows | none |
| tin-before-1706-refusal-1280x900-dark.png | 1706 after Check: buyer TIN refusal ("12 items need attention") — dashed boxes | toast covers part of the next row (item 11), TIN row readable |
| tin-before-1706-refusal-1280x900-light.png | 1706 after Check: buyer TIN refusal ("12 items need attention") — dashed boxes | toast covers part of the next row (item 11), TIN row readable |
| tin-before-1706-refusal-320x700-dark.png | 1706 after Check: buyer TIN refusal ("12 items need attention") — dashed boxes, wrapped to two rows | FAIL overlap: the "12 items need attention" toast and the pill cover the buyer TIN row; its boxes and sentence are not readable; window buttons sit on the sticky part title |
| tin-before-1706-refusal-320x700-light.png | 1706 after Check: buyer TIN refusal ("12 items need attention") — dashed boxes, wrapped to two rows | FAIL overlap: the "12 items need attention" toast and the pill cover the buyer TIN row; its boxes and sentence are not readable; window buttons sit on the sticky part title |
| tin-before-1706-refusal-420x900-dark.png | 1706 after Check: buyer TIN refusal ("12 items need attention") — dashed boxes, wrapped to two rows | toast covers part of the next row (item 11), TIN row readable |
| tin-before-1706-refusal-420x900-light.png | 1706 after Check: buyer TIN refusal ("12 items need attention") — dashed boxes, wrapped to two rows | toast covers part of the next row (item 11), TIN row readable |
| tin-before-1801-typed-empty-1280x900-dark.png | 1801 item 5 locked TIN and item 11 typed TIN (executor), empty — dashed boxes | none |
| tin-before-1801-typed-empty-1280x900-light.png | 1801 item 5 locked TIN and item 11 typed TIN (executor), empty — dashed boxes | none |
| tin-before-1801-typed-empty-320x700-dark.png | 1801 item 5 locked TIN and item 11 typed TIN (executor), empty — dashed boxes, wrapped to two rows | window buttons sit on the sticky part title |
| tin-before-1801-typed-empty-320x700-light.png | 1801 item 5 locked TIN and item 11 typed TIN (executor), empty — dashed boxes, wrapped to two rows | window buttons sit on the sticky part title |
| tin-before-1801-typed-empty-420x900-dark.png | 1801 item 5 locked TIN and item 11 typed TIN (executor), empty — dashed boxes, wrapped to two rows | none |
| tin-before-1801-typed-empty-420x900-light.png | 1801 item 5 locked TIN and item 11 typed TIN (executor), empty — dashed boxes, wrapped to two rows | none |
| tin-before-1801-typed-filled-1280x900-dark.png | 1801 item 11 typed TIN filled 000 123 453 00001 — dashed boxes | none |
| tin-before-1801-typed-filled-1280x900-light.png | 1801 item 11 typed TIN filled 000 123 453 00001 — dashed boxes | none |
| tin-before-1801-typed-filled-320x700-dark.png | 1801 item 11 typed TIN filled 000 123 453 00001 — dashed boxes, wrapped to two rows | window buttons sit on the sticky part title |
| tin-before-1801-typed-filled-320x700-light.png | 1801 item 11 typed TIN filled 000 123 453 00001 — dashed boxes, wrapped to two rows | window buttons sit on the sticky part title |
| tin-before-1801-typed-filled-420x900-dark.png | 1801 item 11 typed TIN filled 000 123 453 00001 — dashed boxes, wrapped to two rows | none |
| tin-before-1801-typed-filled-420x900-light.png | 1801 item 11 typed TIN filled 000 123 453 00001 — dashed boxes, wrapped to two rows | none |
| tin-before-2000-onebox-1280x900-dark.png | 2000 item 9 one-box whole TIN | none |
| tin-before-2000-onebox-1280x900-light.png | 2000 item 9 one-box whole TIN | none |
| tin-before-2000-onebox-320x700-dark.png | 2000 item 9 one-box whole TIN | window buttons sit on the sticky part title |
| tin-before-2000-onebox-320x700-light.png | 2000 item 9 one-box whole TIN | window buttons sit on the sticky part title |
| tin-before-2000-onebox-420x900-dark.png | 2000 item 9 one-box whole TIN | none |
| tin-before-2000-onebox-420x900-light.png | 2000 item 9 one-box whole TIN | none |
