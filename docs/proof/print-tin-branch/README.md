# Print TIN branch padding proof

Each crop is the page-2 TIN row for a taxpayer with TIN `000-123-453` and a
three-digit branch `123`. In the official five-box branch field that branch is
`00123`: `before-*` shows the old right-padded print output (`12300`), and
`after-*` shows the corrected left-padded output (`00123`).

The source markup for every render was produced by filling the frozen form and
then applying the application's print path exactly as the print pipeline does:

```rust
staticize(prepare_html_for(slug, filled_html, true))
```

The full staticized print HTML was generated for every form; its page-2 sheet
was then isolated only for deterministic headless rasterization and cropped
around the TIN row. The raw frozen HTML was not used directly.

| Form | Before | After |
|---|---|---|
| 2200S | ![before 2200S](before-2200s-tin-row.png) | ![after 2200S](after-2200s-tin-row.png) |
| 2200AN | ![before 2200AN](before-2200an-tin-row.png) | ![after 2200AN](after-2200an-tin-row.png) |
| 1707A | ![before 1707A](before-1707a-tin-row.png) | ![after 1707A](after-1707a-tin-row.png) |
| 2200A | ![before 2200A](before-2200a-tin-row.png) | ![after 2200A](after-2200a-tin-row.png) |
| 2200P | ![before 2200P](before-2200p-tin-row.png) | ![after 2200P](after-2200p-tin-row.png) |
| 2200C | ![before 2200C](before-2200c-tin-row.png) | ![after 2200C](after-2200c-tin-row.png) |
| 2000 | ![before 2000](before-2000-tin-row.png) | ![after 2000](after-2000-tin-row.png) |
