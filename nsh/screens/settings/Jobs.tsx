// Settings > Jobs (old cron_tasks.js renderJobsBody): queued filing submissions and scheduled jobs in one feed, and a Logs tab that
// tails today's ebirforms.*.log. The old "Background tasks" page, and go("cron_tasks"), open this section.
import { signal, View } from "nsh";
import type { BackgroundJob } from "../../bir";
import { AppButton, Chip, EmptyRow, ListGroup, ListRow, Segmented, Text } from "../../ui";
import { archiveJob, cancelQueued, deleteJob, jobsUi, purgeArchived, refreshLog, setFilter, setLogFilter, setTab } from "./jobsState";
import { filterJobs, jobActions, jobKey, jobSub, jobTone, levelTone, LOG_FILTERS, logHeadline, logView, STATUS_FILTERS } from "./logic";

/** From this many points of section width the row's buttons sit beside its words; under it they go under them. */
const ROW_BUTTONS_BESIDE = 560;

function JobButtons(props: { job: BackgroundJob }) {
  const a = () => jobActions(props.job);
  const key = () => jobKey(props.job);
  return (
    <View class="shrink-0 flex-row flex-wrap gap-1">
      <If when={a().cancel}>
        <AppButton testID={`job-${key()}-cancel`} label="Cancel queue" size="sm" onPress={() => cancelQueued(props.job)} />
      </If>
      <If when={a().archive}>
        <AppButton testID={`job-${key()}-archive`} label="Archive" variant="ghost" size="sm" onPress={() => archiveJob(props.job.id as number)} />
      </If>
      <If when={a().remove}>
        <AppButton testID={`job-${key()}-delete`} label="Delete" variant="ghost" size="sm" onPress={() => deleteJob(props.job.id as number)} />
      </If>
    </View>
  );
}

/** One job: its name, schedule and runs, its status, and what can be done to it (beside the words when there is room). */
function JobRow(props: { job: BackgroundJob; beside: () => boolean }) {
  return (
    <ListRow testID={`job-${jobKey(props.job)}`}>
      <View class="min-w-0 flex-1 gap-2">
        <View class="flex-row items-center gap-3">
          {/* The words wrap (a job's name carries a TIN and a mailbox, its line a timestamp): never cut to one line with an ellipsis. */}
          <View testID={`job-${jobKey(props.job)}-words`} class="min-w-0 flex-1">
            <Text variant="control">{props.job.name}</Text>
            <Text variant="meta" tone="fg3">{jobSub(props.job)}</Text>
          </View>
          <Chip label={props.job.status || "Unknown"} tone={jobTone(props.job.status)} />
          <If when={props.beside()}>
            <JobButtons job={props.job} />
          </If>
        </View>
        <If when={!props.beside()}>
          <JobButtons job={props.job} />
        </If>
      </View>
    </ListRow>
  );
}

function JobsTab() {
  const width = signal(0);
  const beside = () => width() === 0 || width() >= ROW_BUTTONS_BESIDE;
  const visible = () => filterJobs(jobsUi.jobs(), jobsUi.filter());
  return (
    <View testID="jobs-tab" class="w-full gap-4" onLayout={(e) => e.bounds.size.width > 0 && width.set(Math.round(e.bounds.size.width))}>
      <View class="flex-row flex-wrap items-center gap-2">
        <View class="min-w-0 max-w-full">
          <Segmented
            testID="jobs-filter"
            label="Status"
            wrap
            options={STATUS_FILTERS.map((x) => ({ value: x, label: x }))}
            value={jobsUi.filter()}
            onChange={setFilter}
          />
        </View>
        <View class="flex-1" />
        <AppButton testID="jobs-purge" label="Purge archived" variant="ghost" size="sm" onPress={purgeArchived} />
      </View>
      <If when={jobsUi.failure()}>
        <Text testID="jobs-error" variant="meta" tone="danger">{jobsUi.failure() ?? ""}</Text>
      </If>
      <ListGroup testID="jobs-list">
        <If when={visible().length} else={<EmptyRow testID="jobs-empty" text="No background jobs in this view." />}>
          <Each of={visible()} key={(j) => jobKey(j)}>
            {(job) => <JobRow job={job()} beside={beside} />}
          </Each>
        </If>
      </ListGroup>
    </View>
  );
}

function LogsTab() {
  const log = () => jobsUi.log();
  const view = () => logView(log().content, jobsUi.logFilter());
  return (
    <View testID="logs-tab" class="w-full gap-4">
      <View class="flex-row flex-wrap items-center gap-3">
        <View class="min-w-0 max-w-full">
          <Segmented testID="logs-filter" label="Log level" wrap options={[...LOG_FILTERS]} value={jobsUi.logFilter()} onChange={setLogFilter} />
        </View>
        <Text testID="logs-headline" variant="meta" tone="fg2" class="min-w-0 shrink">{logHeadline(log().file_name, log().lines, view().matching)}</Text>
        <View class="flex-1" />
        <AppButton testID="logs-refresh" label="Refresh" size="sm" onPress={refreshLog} />
      </View>
      <If when={log().error}>
        <Text testID="logs-error" variant="meta" tone="danger">{log().error ?? ""}</Text>
      </If>
      <View testID="log-box" class="w-full gap-0.5 rounded-(--radius-card) border border-border bg-background p-3">
        <If
          when={view().shown.length}
          else={
            <Text testID="logs-empty" variant="meta" tone="fg2">
              {jobsUi.logFilter() === "All Logs" ? "No log entries yet today." : "No lines at this level."}
            </Text>
          }
        >
          <Each of={view().shown} key={(l) => l.index}>
            {(l) => (
              <Text testID={`log-line-${l().index}`} variant="cap" tone={levelTone(l().level)} class="font-mono font-normal leading-4">
                {l().line || " "}
              </Text>
            )}
          </Each>
        </If>
      </View>
    </View>
  );
}

/** The Jobs section: a Jobs / Logs switch over the tab. */
export function JobsSection() {
  return (
    <View testID="jobs-section" class="w-full gap-4">
      <Segmented
        testID="jobs-tabs"
        label="Background tasks view"
        options={[
          { value: "jobs", label: "Jobs" },
          { value: "logs", label: "Logs" },
        ]}
        value={jobsUi.tab()}
        onChange={(v) => setTab(v === "logs" ? "logs" : "jobs")}
      />
      <If when={jobsUi.tab() === "logs"} else={<JobsTab />}>
        <LogsTab />
      </If>
    </View>
  );
}
