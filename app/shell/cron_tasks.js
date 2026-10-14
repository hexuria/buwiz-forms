// Background Tasks: the painted CronTasksView — one merged feed of queued
// filing submissions + scheduled job_queue rows, and a Logs tab tailing
// today's rotated ebirforms.*.log.
import { div } from "gpui-kit";
import { v_flex, h_flex } from "gpui-base";
import { Button } from "gpui-component";
import { pageHeading, pageSub } from "./pages.js";

const STATUS_FILTERS = ["All Jobs", "Queued", "Failed", "Completed", "Archived"];

function statusColor(cx, status) {
  if (status === "Queued" || status === "Running") return [cx.theme().colors.accent, cx.theme().colors.accent_foreground];
  if (status === "Failed") return [cx.theme().colors.danger, cx.theme().colors.danger_foreground];
  if (status === "Completed" || status === "Submitted" || status === "Confirmed" || status === "Paid")
    return [cx.theme().colors.success, cx.theme().colors.success_foreground];
  return [cx.theme().colors.muted, cx.theme().colors.muted_foreground];
}

function jobRow(cx, state, job) {
  const [bg, fg] = statusColor(cx, job.status);
  const schedule = job.cron_expr ? `cron ${job.cron_expr}` : job.job_type === "System" ? "on queue" : "manual";
  const runs = [
    job.last_run_at ? `last ${job.last_run_at}` : null,
    job.next_run_at ? `next ${job.next_run_at}` : null,
  ]
    .filter(Boolean)
    .join(" · ");
  return h_flex()
    .w_full()
    .px(12)
    .py(8)
    .gap(10)
    .items_center()
    .rounded_md()
    .hover((s) => s.bg(cx.theme().colors.muted))
    .child(
      div()
        .w(90)
        .flex_none()
        .text_size(11)
        .font_semibold()
        .text_color(cx.theme().colors.muted_foreground)
        .child(job.kind === "submission" ? "SUBMISSION" : job.job_type.toUpperCase()),
    )
    .child(
      v_flex()
        .flex_1()
        .min_w_0()
        .gap(2)
        .child(
          div()
            .text_size(13)
            .text_color(cx.theme().colors.foreground)
            .child(job.name),
        )
        .child(
          div()
            .text_size(11)
            .text_color(cx.theme().colors.muted_foreground)
            .child([schedule, runs].filter(Boolean).join(" · ") || " "),
        ),
    )
    .child(
      div()
        .px(8)
        .py(2)
        .rounded_md()
        .text_size(11)
        .font_semibold()
        .bg(bg)
        .text_color(fg)
        .child(job.status),
    )
    .when(job.kind === "submission" && job.status === "Queued", (el) =>
      el.child(
        Button.new(`cron_cancel_${job.id}`)
          .label("Cancel")
          .outline()
          .compact()
          .on_click((_ev, clickCx) => state.cancelQueuedEntry(job, clickCx)),
      ),
    )
    .when(job.kind === "job" && job.id != null && job.status !== "Archived", (el) =>
      el.child(
        Button.new(`cron_archive_${job.id}`)
          .label("Archive")
          .ghost()
          .compact()
          .on_click((_ev, clickCx) => state.archiveJob(job.id, clickCx)),
      ),
    )
    .when(job.kind === "job" && job.id != null, (el) =>
      el.child(
        Button.new(`cron_delete_${job.id}`)
          .label("Delete")
          .ghost()
          .compact()
          .on_click((_ev, clickCx) => state.deleteJob(job.id, clickCx)),
      ),
    );
}

function renderJobs(cx, state) {
  let jobs = [];
  let error = null;
  try {
    jobs = state.backgroundJobs();
  } catch (e) {
    error = String(e);
  }
  const filter = state.cronFilter || "All Jobs";
  const visible =
    filter === "All Jobs" ? jobs : jobs.filter((j) => j.status === filter);
  return v_flex()
    .gap(10)
    .child(
      h_flex()
        .gap(6)
        .flex_wrap()
        .children(
          STATUS_FILTERS.map((name) =>
            Button.new(`cron_filter_${name.replace(/\s/g, "_")}`)
              .label(name)
              .when(name === filter, (b) => b.secondary())
              .when(name !== filter, (b) => b.ghost())
              .compact()
              .on_click((_ev, clickCx) => state.setCronFilter(name, clickCx)),
          ),
        ),
    )
    .when(Boolean(error || state.cronError), (el) =>
      el.child(
        div()
          .text_size(13)
          .text_color("#EF4444")
          .child(String(error || state.cronError)),
      ),
    )
    .when(!error && visible.length === 0, (el) =>
      el.child(
        div()
          .text_size(13)
          .text_color(cx.theme().colors.muted_foreground)
          .child("No background jobs in this view."),
      ),
    )
    .children(visible.map((job) => jobRow(cx, state, job)));
}

function renderLogs(cx, state) {
  let log = { file_name: "", lines: 0, content: "" };
  let error = null;
  try {
    log = state.jobLogTail();
  } catch (e) {
    error = String(e);
  }
  return v_flex()
    .gap(10)
    .child(
      h_flex()
        .items_center()
        .gap(10)
        .child(
          div()
            .text_size(12)
            .text_color(cx.theme().colors.muted_foreground)
            .child(`${log.file_name || "no log file"} · ${log.lines} lines`),
        )
        .child(
          Button.new("cron_logs_refresh")
            .label("Refresh")
            .outline()
            .compact()
            .on_click((_ev, clickCx) => state.refreshJobLog(clickCx)),
        ),
    )
    .when(Boolean(error), (el) =>
      el.child(
        div()
          .text_size(13)
          .text_color("#EF4444")
          .child(String(error)),
      ),
    )
    .child(
      div()
        .w_full()
        .rounded_md()
        .border(1)
        .border_color(cx.theme().colors.border)
        .bg(cx.theme().colors.muted)
        .p(12)
        .text_size(11)
        .font_family(cx.theme().typography.mono)
        .whitespace_nowrap()
        .child(log.content || " "),
    );
}

export function renderCronTasksPage(cx, state) {
  const tab = state.cronTab || "jobs";
  return div()
    .size_full()
    .overflow_y_scroll()
    .p(24)
    .child(
      v_flex()
        .gap(12)
        .max_w_full()
        .child(
          h_flex()
            .items_center()
            .justify_between()
            .child(pageHeading("Background Tasks", cx))
            .child(
              h_flex()
                .gap(6)
                .child(
                  Button.new("cron_tab_jobs")
                    .label("Jobs")
                    .when(tab === "jobs", (b) => b.secondary())
                    .when(tab !== "jobs", (b) => b.ghost())
                    .compact()
                    .on_click((_ev, clickCx) => state.setCronTab("jobs", clickCx)),
                )
                .child(
                  Button.new("cron_tab_logs")
                    .label("Logs")
                    .when(tab === "logs", (b) => b.secondary())
                    .when(tab !== "logs", (b) => b.ghost())
                    .compact()
                    .on_click((_ev, clickCx) => state.setCronTab("logs", clickCx)),
                ),
            ),
        )
        .when(tab === "jobs", (el) =>
          el.child(
            h_flex()
              .justify_between()
              .items_center()
              .child(
                pageSub(
                  "Queued filings and scheduled jobs — same merged feed as the desktop app",
                  cx,
                ),
              )
              .child(
                Button.new("cron_purge")
                  .label("Purge archived")
                  .ghost()
                  .compact()
                  .on_click((_ev, clickCx) => state.purgeArchivedJobs(clickCx)),
              ),
          ),
        )
        .child(tab === "logs" ? renderLogs(cx, state) : renderJobs(cx, state)),
    );
}
