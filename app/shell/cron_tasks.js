// Background tasks: queued filing submissions and scheduled jobs in one
// feed, and a Logs tab tailing today's ebirforms.*.log. Built from the
// redesign's wrappers; the log wraps inside its box rather than running
// sideways (the only sideways scroller in the app is a schedule table).

import { div } from "gpui-kit";
import {
  tokens,
  SPACE,
  RADIUS,
  Text,
  AppButton,
  Segmented,
  Chip,
  ListGroup,
  ListRow,
  NameCell,
  EmptyRow,
  PageHeader,
} from "./ui.js";

const STATUS_FILTERS = ["All Jobs", "Queued", "Failed", "Completed", "Archived"];
/** Lines drawn from the tail of the log; older ones stay in the file. */
const LOG_LINES = 200;

function statusChip(cx, status) {
  if (status === "Queued" || status === "Running") return Chip(cx, status, "accent");
  if (status === "Failed") return Chip(cx, status, "danger");
  if (["Completed", "Submitted", "Confirmed", "Paid"].includes(status)) return Chip(cx, status, "success");
  return Chip(cx, status || "Unknown");
}

function jobRow(cx, app, job, width) {
  const schedule = job.cron_expr ? `Schedule: ${job.cron_expr}` : job.job_type === "System" ? "Runs when the queue wakes" : "Manual";
  const runs = [job.last_run_at ? `last ${job.last_run_at}` : null, job.next_run_at ? `next ${job.next_run_at}` : null]
    .filter(Boolean)
    .join(" · ");
  const row = ListRow(cx, `job-${job.kind}-${job.id}`)
    .child(NameCell(cx, job.name, [schedule, runs].filter(Boolean).join(" · ")))
    .child(div().flex().flex_row().flex_none().child(statusChip(cx, job.status)));
  const actions = div().flex().flex_row().flex_none().gap(SPACE[1]);
  if (job.kind === "submission" && job.status === "Queued") {
    actions.child(AppButton(cx, `job-${job.id}-cancel`, "Cancel queue", { size: "sm", onClick: (_e, cx) => app.cancelQueuedEntry(job, cx) }));
  }
  if (job.kind === "job" && job.id != null && job.status !== "Archived") {
    actions.child(AppButton(cx, `job-${job.id}-archive`, "Archive", { variant: "ghost", size: "sm", onClick: (_e, cx) => app.archiveJob(job.id, cx) }));
  }
  if (job.kind === "job" && job.id != null) {
    actions.child(AppButton(cx, `job-${job.id}-delete`, "Delete", { variant: "ghost", size: "sm", onClick: (_e, cx) => app.deleteJob(job.id, cx) }));
  }
  if (width >= 560) row.child(actions);
  return row;
}

function jobsTab(cx, app, width) {
  let jobs = [];
  let error = null;
  try {
    jobs = app.backgroundJobs();
  } catch (e) {
    error = String(e);
  }
  const filter = app.cronFilter || "All Jobs";
  const visible = filter === "All Jobs" ? jobs : jobs.filter((j) => j.status === filter);
  const page = div()
    .flex()
    .flex_col()
    .child(
      div()
        .flex()
        .flex_row()
        .flex_wrap()
        .items_center()
        .gap(SPACE[2])
        .mb(SPACE[4])
        .child(
          Segmented(
            cx,
            "jobs-filter",
            STATUS_FILTERS.map((x) => ({ value: x, label: x })),
            filter,
            (v, cx) => app.setCronFilter(v, cx),
            { label: "Status" },
          ),
        )
        .child(div().flex_1())
        .child(AppButton(cx, "jobs-purge", "Purge archived", { variant: "ghost", size: "sm", onClick: (_e, cx) => app.purgeArchivedJobs(cx) })),
    );
  if (error || app.cronError) page.child(Text(cx, "meta", String(error || app.cronError), "danger").mb(SPACE[3]).whitespace_normal());
  return page.child(
    ListGroup(
      cx,
      "jobs-list",
      visible.length ? visible.map((job) => jobRow(cx, app, job, width)) : [EmptyRow(cx, "jobs-empty", "No background jobs in this view.")],
    ),
  );
}

function logsTab(cx, app, width) {
  const t = tokens(cx);
  /** @type {{ file_name: string, lines: number, content: string, error?: string }} */
  let log = { file_name: "", lines: 0, content: "" };
  let error = null;
  try {
    log = app.jobLogTail();
    error = log.error || null;
  } catch (e) {
    error = String(e);
  }
  const lines = String(log.content || "").split("\n");
  const shown = lines.slice(-LOG_LINES);
  // One wrapped text block: a separate element per line costs a text layout
  // each and took render past gpui-shell's 2 s limit.
  const box = div()
    .id("log-box")
    .w(width)
    .p(SPACE[3])
    .rounded(RADIUS.card)
    .border(1)
    .border_color(t.border)
    .bg(t.bg)
    .overflow_hidden()
    .child(
      div()
        .w(width - 2 * SPACE[3] - 2)
        .text_size(11)
        .line_height("16px")
        .font_family(cx.theme().typography.mono)
        .text_color(t.fg2)
        .whitespace_normal()
        .child(shown.join("\n") || " "),
    );
  const head = div()
    .flex()
    .flex_row()
    .flex_wrap()
    .items_center()
    .gap(SPACE[3])
    .mb(SPACE[4])
    .child(
      Text(
        cx,
        "meta",
        `${log.file_name || "No log file yet"} · ${log.lines} lines${lines.length > LOG_LINES ? ` · showing the last ${LOG_LINES}` : ""}`,
        "fg2",
      ),
    )
    .child(div().flex_1())
    .child(AppButton(cx, "logs-refresh", "Refresh", { size: "sm", onClick: (_e, cx) => app.refreshJobLog(cx) }));
  const page = div().flex().flex_col().child(head);
  if (error) page.child(Text(cx, "meta", error, "danger").mb(SPACE[3]).whitespace_normal());
  return page.child(box);
}

/** Jobs and today's log, without a page title. Settings › Jobs draws this. */
export function renderJobsBody(cx, app, width) {
  const tab = app.cronTab || "jobs";
  return div()
    .flex()
    .flex_col()
    .child(
      div()
        .mb(SPACE[4])
        .child(
          Segmented(
            cx,
            "jobs-tabs",
            [
              { value: "jobs", label: "Jobs" },
              { value: "logs", label: "Logs" },
            ],
            tab,
            (v, cx) => app.setCronTab(v, cx),
            { label: "Background tasks view" },
          ),
        ),
    )
    .child(tab === "logs" ? logsTab(cx, app, width) : jobsTab(cx, app, width));
}

export function renderCronTasksPage(cx, app, width) {
  return div()
    .flex()
    .flex_col()
    .child(
      PageHeader(cx, {
        title: "Background tasks",
        lede: "Queued filings, scheduled jobs and today's log.",
      }),
    )
    .child(div().mt(28).child(renderJobsBody(cx, app, width)));
}
