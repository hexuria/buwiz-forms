// The group "Filed in BIR eBIRForms" under Filings' board or list: the returns the import brought in from the old program for the
// taxpayer in view (every branch of it), newest period first. Records only: a row is not pressable, a return is no draft, cannot
// be sent and is never queued. Nothing to draw until there is a row: no empty box. Reads bir.list_legacy_filed_returns through
// the store, one read per branch.
import { computed, View } from "nsh";
import { app, store } from "../../shell";
import { Chip, Line, ListGroup, ListRow, Text } from "../../ui";
import { LEGACY_TITLE, type LegacyEntry, type LegacyLine, legacyLines } from "./legacy";

/** One imported return: its form and period, what kind of copy it is, the branch when it is not the head office, and when it came in. */
function LegacyRow(props: { line: LegacyLine; wide: boolean }) {
  return (
    <ListRow testID={`legacy-${props.line.id}`}>
      <View class="min-w-0 flex-1 gap-0.5">
        <View class="w-full flex-row flex-wrap items-center gap-x-2 gap-y-1">
          <Line testID={`legacy-${props.line.id}-title`} variant="emph">{props.line.title}</Line>
          <If when={props.line.amended}>
            <Chip testID={`legacy-${props.line.id}-amended`} label={props.line.amended} />
          </If>
          <If when={props.line.emailCopy}>
            <Chip testID={`legacy-${props.line.id}-email`} label="E-mailed copy" />
          </If>
        </View>
        <If when={props.line.branch}>
          <Line variant="meta" tone="fg3">{props.line.branch}</Line>
        </If>
        <If when={!props.wide}>
          <Text testID={`legacy-${props.line.id}-imported`} variant="cap" tone="fg3" class="tabular-nums">{props.line.imported}</Text>
        </If>
      </View>
      <If when={props.wide}>
        <View class="shrink-0" style={{ width: 168 }}>
          <Text testID={`legacy-${props.line.id}-imported`} variant="meta" tone="fg2" class="text-right tabular-nums">{props.line.imported}</Text>
        </View>
      </If>
    </ListRow>
  );
}

/** The group for the taxpayer in view, or nothing. `wide` is Filings' own test (a page 720 points wide or more). */
export function LegacyGroup(props: { wide: boolean }) {
  const entries = computed<LegacyEntry[]>(() => {
    const t = app.taxpayer();
    return t ? t.branches.flatMap((b) => store.legacyFiled(b.tin).map((row) => ({ row, branch: b.is_head_office ? "" : b.name }))) : [];
  });
  // The catalog (each form's frequency, which names a period) is not read while there is nothing to name.
  const lines = computed(() => (entries().length ? legacyLines(entries(), store.catalog()) : []));
  return (
    <If when={lines().length > 0}>
      <View testID="filings-legacy" class="mt-8 w-full">
        <View class="w-full flex-row items-center gap-2">
          <Text testID="filings-legacy-title" variant="title">{LEGACY_TITLE}</Text>
          <Chip testID="filings-legacy-count" label={String(lines().length)} />
        </View>
        <Text testID="filings-legacy-note" variant="meta" tone="fg2" class="mt-1 mb-3">
          {`Imported from the old program for ${app.taxpayer()?.name ?? ""}. Records only: they cannot be opened or sent.`}
        </Text>
        <ListGroup testID="filings-legacy-list">
          <Each of={lines()} key={(l) => l.id}>
            {(l) => <LegacyRow line={l()} wide={props.wide} />}
          </Each>
        </ListGroup>
      </View>
    </If>
  );
}
