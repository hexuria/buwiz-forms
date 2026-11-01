# Frozen BIR HTML

Product fill/print sheets for the 43 inventory forms. The app loads this tree.

Generated (`forms/`) and corrected (`forms-corrected/`) HTML live in
https://github.com/hexuria/buwiz-forms-html (tag `v1-frozen`). Layout edits
belong there. XML `name=` stamps are fail-closed joins from
`rules/forms/*/fields.json` and the identity catalog; never invent a key.

2551Q (`2551q-2018`) and 1601C (`1601c-2018`) have fail-closed TIN `name=`
stamps from the identity catalog and `rules/forms/*/fields.json`. See
`name-gaps.json` for writer keys that still sit on cell ids. Identity leftovers
(name, address, RDO, ZIP, telephone, email) fill through `writer-cells.json`
`joins` onto those cell ids. Text joins ASCII-uppercase letters for BIR
CAPITAL LETTERS print; profile/DB values stay as stored. 2551Q Year Ended is
`rtnMonth` + `txtYear` (MM/YYYY combs). Quarter / Calendar / Fiscal radios use
`xbox_joins` (`X` when the writer is true). 1601C For the Month is `txtMonth` +
`txtYear` (MM/YYYY combs `p1c9`/`p1c10`). Amended Return and Any Taxes Withheld
Yes/No use `xbox_joins` on `p1c21`/`p1c22` and `p1c23`/`p1c20`. Tax
amounts use `money_joins`: one leftover money key, peso comb + 2-slot cents,
`{:.2}` split, pesos right-aligned. That is not an `official_field_key` stamp.

Every value a form's print map carries prints in its box or is listed in
`writer-cells.json` `not_joined` with the reason it has none (not on the paper
form, preprinted, printed from a `derived:` key, a row past the printed ones);
`not_joined_cells` lists the cells no key fills. The coverage gate
(`bir-shell` `views::print_coverage`) checks this for every form on the
draft a new return opens with and on one with every box filled. One key may
fill several cells (`html_ids`, `peso_html_ids`/`cent_html_ids`,
`decimal_html_ids`), so the name, TIN and period repeat in the later pages'
headers. `date_joins` split one writer date over MM / DD / YYYY combs.


## Regeneration and verification

The source bundle is the `v1-frozen` tag in `hexuria/buwiz-forms-html`. Regenerate the committed tree with `python3 .ci-html/tools/formgen/freeze_html.py --out html-frozen --index rules/index.json`, then run the frozen-name stamp checks used by CI. CI verifies that the committed tree is unchanged with `python3 .ci-html/tools/formgen/freeze_html.py --verify --out html-frozen --index rules/index.json`; do not omit `--verify` in the CI check.
