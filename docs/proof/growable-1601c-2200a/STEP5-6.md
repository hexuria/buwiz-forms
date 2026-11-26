# Steps 5-6 proof

## Scratch print proof

`print-proof.log` (`rc:0`) is a local scratch merge of this branch with
`try/forms-band-flow` `31d3b7d765593627f5e36acb00264dc2e2c77c98`. The scratch
merge was never pushed. Its test builds real 1601C and 2200A drafts at 5 and 20
rows, fills the frozen print documents, and asserts every distinct row marker
prints exactly once in order and the printed total digit stream equals the row
sum.

## Page test

`nsh/screens/form/growableRows.test.tsx` adds rows 4 through 7 on both form pages and deletes row 7. `nsh-growable.log` records the result.

The page implementation still exposes generated schema rows through 20; validation/upload are uncapped.
