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
`joins` onto those cell ids. Demo tax amounts use `money_joins`: one leftover
money key, peso comb + 2-slot cents, `{:.2}` split, pesos right-aligned. That
is not an `official_field_key` stamp. Other Part II / schedule rows stay
unjoined.
