// A page for a screen that is not built yet: its name, who builds it and when, and what the old app showed there.
// The frame maps every unbuilt view to one of these (shell/views.tsx); a screen agent replaces the mapping, not this file.
import { View } from "nsh";
import { app } from "../../shell/app";
import { Card, Chip, MetaLine, PageHeader, Text } from "../../ui";

/** Props of Placeholder. */
export interface PlaceholderProps {
  /** The page's title. */
  title: string;
  /** The view's name, for the test id ("placeholder-home"). */
  view: string;
  /** The wave and agent that build it ("Wave 3, C1"). */
  coming: string;
  /** What the old app showed here, in a sentence. */
  old: string;
  /** Show the taxpayer the frame has selected (the views that need one). */
  withTaxpayer?: boolean;
}

/** "Coming in wave N": a title, a note, and (for taxpayer views) whose page it will be. */
export function Placeholder(props: PlaceholderProps) {
  const tp = () => app.taxpayer();
  const br = () => app.branch();
  return (
    <View testID={`placeholder-${props.view}`} class="gap-4">
      <PageHeader title={props.title} meta={
          <View class="flex-row">
            <Chip label={`Coming in ${props.coming}`} tone="accent" />
          </View>
        } />
      <Card>
        <View class="gap-2 py-3">
          <Text>{props.old}</Text>
          <If when={props.withTaxpayer && tp()}>
            <MetaLine parts={[tp()?.name, br() ? `Branch ${br()?.code}` : null, app.year() ? `Tax year ${app.year()}` : null]} />
          </If>
        </View>
      </Card>
    </View>
  );
}
