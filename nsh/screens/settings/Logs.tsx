// Settings > Logs (the Logs half of the old cron_tasks.js): a tail of today's ebirforms.*.log, its level filters and
// Refresh. Its own section beside Jobs: opening it reads the file afresh.
import { View } from "nsh";
import { AppButton, Segmented, Text } from "../../ui";
import { jobsUi, refreshLog, setLogFilter } from "./jobsState";
import { levelTone, LOG_FILTERS, logHeadline, logView } from "./logic";

/** The Logs section: the level filters, how much of the file shows, and the tail's lines. */
export function LogsSection() {
  // Opening Logs reads today's file afresh (the component runs once, as the section mounts).
  refreshLog();
  const log = () => jobsUi.log();
  const view = () => logView(log().content, jobsUi.logFilter());
  return (
    <View testID="logs-section" class="w-full gap-4">
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
