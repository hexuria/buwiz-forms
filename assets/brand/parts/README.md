# The Buwiz Forms logo's parts

The logo, `assets/brand/buwiz-forms.svg`, is drawn as eight parts. Each part is a complete shape, and `merge.py`
stacks them into the one logo.

| File | Part |
| --- | --- |
| `1-sun.svg` | The sun behind the eagle |
| `2-eagle.svg` | The eagle |
| `3-form.svg` | The tablet with the form on it |
| `4-blue-swoosh.svg` | The blue swoosh |
| `5-red-swoosh.svg` | The red swoosh: its dark fold (group `red-back`) and its bright face (group `red-front`) |
| `6-ebirforms-text.svg` | The "eBIRForms" lettering on the tablet |
| `7-buwiz-forms-text.svg` | The "Buwiz Forms" lettering. The merge takes only its "Buwiz" (group `buwiz`). The file also keeps a spare copy of the white "Forms" letters, with the same outline and the same 10 px drop, so the lettering file is complete on its own |
| `8-forms-banner.svg` | The Forms banner, with its speed lines and the white "Forms" letters (group `forms-letters`); these are the letters the logo shows |

Back to front, the merge stacks:
1. the sun;
2. the red swoosh's dark fold;
3. the tablet;
4. the eBIRForms lettering;
5. the blue swoosh;
6. the red swoosh's bright face;
7. the eagle;
8. "Buwiz" on its white border;
9. the banner with its white letters.

## The owner's choices that the merge keeps

- No white outlines between the parts.
- "Buwiz" keeps a 17 px white border: a white stroke of its own outline, with the letter counters filled.
- The banner and its letters sit 10 px lower than in the original, so Buwiz's border has 8 px of space above the
  banner. The drop is drawn into `8-forms-banner.svg` itself.
- The banner's speed lines fade away (the owner's pick "A").
- The logo is never shown without the words "Buwiz Forms". There is no emblem-only version, and none is to be made.
- On dark surfaces the logo is used as drawn, with no outline and no tile. The owner picked this on 2026-11-26.

## Changing a part

1. Edit the part's file here.
2. `just logo` rebuilds `assets/brand/buwiz-forms.svg` from the eight parts.
3. `just icons` rebuilds every app icon from that logo (see `assets/brand/make_icons.py`).
4. Look at the result, run `just icons-check`, commit the part, the logo and the icons, and open a pull request.

`just icons-check` also checks that `buwiz-forms.svg` is exactly what the parts give. Both commands need `uv`, which
fetches Pillow and NumPy, and `rsvg-convert` (`brew install librsvg`).

Two cautions:

- **The viewBox is pinned in `merge.py`** (`VIEWBOX`), so the logo comes out the same on every machine. If a change moves
  the logo's outer edge, run `just logo --measure` and paste the viewBox it prints into `VIEWBOX`. A plain `just logo`
  warns when what is drawn no longer fits the pinned viewBox.
- **Keep the "Buwiz" paths in `7-buwiz-forms-text.svg` absolute.** The merge fills the letter counters white by
  splitting those paths at each absolute `M`. An editor that re-saves them with relative `m` commands would leave the
  B's and the other counters unfilled.

How each part was first drawn from the original artwork stays in the artwork session's working folder. It is not in the
repository.
