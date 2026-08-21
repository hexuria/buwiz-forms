# Frozen BIR HTML

Product fill/print sheets for the 43 inventory forms. The app loads this tree.

Generated (`forms/`) and corrected (`forms-corrected/`) HTML live in
https://github.com/hexuria/buwiz-forms-html (tag `v1-frozen`). Layout edits
belong there. XML `name=` stamps are fail-closed joins from
`rules/forms/*/fields.json` and the identity catalog; never invent a key.

2551Q (`2551q-2018`) has fail-closed TIN `name=` stamps from the identity catalog and `rules/forms/2551q-v2018/fields.json`. See `name-gaps.json` for writer keys that still sit on cell ids.
