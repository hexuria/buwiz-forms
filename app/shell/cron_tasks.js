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

/**
 * Same markers painted bir classifies on (` ERROR ` / ` FATAL ` / ` WARN ` /
 * ` INFO `). See `bir_core::logging::log_line_level`.
 */
function logLineLevel(line) {
  if (line.includes(" ERROR ") || line.includes(" FATAL ")) return "Error";
  if (line.includes(" WARN ")) return "Warn";
  if (line.includes(" INFO ")) return "Info";
  return "Other";
}

const LOG_FILTERS = [
  { value: "All Logs", label: "All" },
  { value: "Error", label: "Error" },
  { value: "Warn", label: "Warn" },
  { value: "Info", label: "Info" },
];

function lineColor(level) {
  if (level === "Error") return "danger";
  if (level === "Warn") return "warning";
  if (level === "Info") return "fg";
  return "fg2";
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
  const filter = app.cronLogFilter || "All Logs";
  // `String#split` keeps a trailing empty piece when the file ends in a
  // newline. Painted bir uses `str::lines`, which drops that piece.
  const lines = String(log.content || "").split("\n");
  if (lines.length && lines[lines.length - 1] === "") lines.pop();
  const matching = lines.filter((line) => filter === "All Logs" || logLineLevel(line) === filter);
  const shown = matching.slice(-LOG_LINES);
  // The tail is capped (a separate element per line of the whole file took
  // render past gpui-shell's 2 s limit). Each kept line is its own element so
  // Error / Warn / Info can take the same colors as painted bir.
  const body = div().w(width - 2 * SPACE[3] - 2).flex().flex_col();
  if (!shown.length || (shown.length === 1 && !shown[0])) {
    body.child(Text(cx, "meta", filter === "All Logs" ? "No log entries yet today." : "No lines at this level.", "fg2"));
  } else {
    shown.forEach((line, index) => {
      body.child(
        div()
          .id(`log-line-${index}`)
          .w(width - 2 * SPACE[3] - 2)
          .text_size(11)
          .line_height("16px")
          .font_family(cx.theme().typography.mono)
          .text_color(t[lineColor(logLineLevel(line))] || t.fg2)
          .whitespace_normal()
          .child(line || " "),
      );
    });
  }
  const box = div()
    .id("log-box")
    .w(width)
    .p(SPACE[3])
    .rounded(RADIUS.card)
    .border(1)
    .border_color(t.border)
    .bg(t.bg)
    .overflow_hidden()
    .child(body);
  const head = div()
    .flex()
    .flex_row()
    .flex_wrap()
    .items_center()
    .gap(SPACE[3])
    .mb(SPACE[4])
    .child(
      Segmented(cx, "logs-filter", LOG_FILTERS, filter, (v, cx) => app.setCronLogFilter(v, cx), {
        label: "Log level",
      }),
    )
    .child(
      Text(
        cx,
        "meta",
        `${log.file_name || "No log file yet"} · ${log.lines} lines${matching.length > LOG_LINES ? ` · showing the last ${LOG_LINES}` : ""}`,
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
