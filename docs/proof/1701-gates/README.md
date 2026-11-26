# 1701 Schedule gates — visual proof

Form 1701 (TY 2025) for the demo taxpayer (TIN 123-456-789-00000), invented values
only, in the running desktop app on Linux with the four safety variables
(`BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1`; never Queue).
Desktop width (1440 pt). Two screenshots per state of Part V · Schedule 4 —
first the top of the part, then scrolled down — plus one recording.

## States

| file pair | state |
|---|---|
| s1-fresh-a/b.png | fresh return, no ATC chosen |
| s2-graditem-a/b.png | graduated rates with business income (ATC II012), itemized deduction |
| s3-gradosd-a/b.png | graduated rates with business income (ATC II012), optional standard deduction |
| s4-eight-a/b.png | 8% flat rate (ATC II015) |
| s5-comp-a/b.png | compensation income only (ATC II011) |

## What they show

- **s2 (graduated + itemized)**: Schedule 4 renders its 21 items and every entry
  box is open (white, editable) — items 1–7 in shot a, items 14–17d in shot b.
- **s1, s3, s4, s5**: the item rows of Schedule 4 do not render at all; the part
  holds only item 18, the computed total, whose boxes are drawn locked (greyed).
  There is no box a filer could type into, so nothing can be silently zeroed on
  compute. The same applies to Schedules 3.A and 6 (their rows follow the same
  gates; Schedule 4 is the one pictured, per the task).
- **1701-gates-recording.mp4** (19 s, under two minutes): switching the deduction
  method from itemized to OSD and the rate from graduated (II012) to 8% (II015)
  in the running app — the Schedule 4 rows open under itemized and are gone under
  OSD/8%, live.

## How the evidence was made (method honesty)

The app runs in Docker (`buwiz-nsh-2404-vk`) on an Xvfb display; on Linux the GTK
surface cannot be captured from the X framebuffer, so each still is the host's own
rendered pixels read back through the window-plane `screenshot` op of the running
app (gpui-agent), and the recording is the same capture repeated as a frame series
during the live switches, deduplicated (38 distinct frames, 2 fps) and encoded with
ffmpeg. The states were written through the data plane `form.fill` — the same write
path a filer's typing takes, gate refusals included — with invented values; the
"save income type to the tax profile" prompt was answered "Only this return" so the
demo profile is unchanged. The return was discarded afterwards.
