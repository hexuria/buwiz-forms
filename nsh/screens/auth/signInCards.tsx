// The pieces of the sign-in flow's cards (signInFlow.ts), drawn the same on the account gate and in Settings > Account: a value
// with Copy (the code, the short address), the card's actions (full width on a phone, a row with a pointer), and a waiting
// card's body. A later card (an email page, a code page) is built from the same pieces.
import { Clipboard, View } from "nsh";
import { toast } from "../../shell";
import { AppButton, Code, deviceCopy, Spinner, Text, TOUCH } from "../../ui";
import { TextLink } from "./parts";
import { type CardAction, type CardRow, type FlowCard, signInFlow, type SignInEntry } from "./signInFlow";

/** Copies `value` and says so (the Copy TIN pattern of the taxpayer page). */
function copy(value: string, what: string): void {
  Clipboard.writeText(value).then(
    () => toast(`Copied ${value}`, "copy"),
    () => toast(`The ${what} could not be copied.`, "triangle-alert"),
  );
}

/** A labelled value with Copy. The address is a link that opens the page (the same page, for the same code). */
export function CopyRow(props: { row: CardRow; testID: string }) {
  return (
    <View testID={props.testID} class="w-full flex-row items-center gap-3 rounded-(--radius-ctl) border border-border px-3 py-2">
      <View class="min-w-0 flex-1 gap-0.5">
        <Text variant="meta" tone="fg3">{props.row.label}</Text>
        <If
          when={props.row.opens}
          else={
            <Code testID={`${props.testID}-value`} class="text-base font-semibold tracking-widest">
              {props.row.value}
            </Code>
          }
        >
          <TextLink testID={`${props.testID}-value`} label={props.row.value} onPress={() => void signInFlow.reopen()} />
        </If>
      </View>
      <AppButton
        testID={`${props.testID}-copy`}
        label="Copy"
        icon="copy"
        size="sm"
        accessibilityLabel={`Copy the ${props.row.label.toLowerCase()}`}
        onPress={() => copy(props.row.value, props.row.label.toLowerCase())}
      />
    </View>
  );
}

/** A button that shows a spinner while its own request runs (never disabled for the wait on the person). */
export function FlowButton(props: { testID: string; label: string; primary?: boolean; busy?: boolean; onPress: () => void }) {
  return (
    <AppButton
      testID={props.testID}
      label={props.label}
      variant={props.primary ? "primary" : "secondary"}
      block={TOUCH}
      leading={props.busy ? <Spinner size={14} tone={props.primary ? "primaryFg" : "fg2"} label={`${props.label}…`} /> : undefined}
      onPress={props.onPress}
    />
  );
}

/** A card's actions: stacked full width on a phone, a row (main one last, on the right) with a pointer. */
export function FlowActions(props: { actions: CardAction[]; prefix: string }) {
  const press = (action: CardAction) => (action.id === "cancel" ? signInFlow.cancel() : void signInFlow.reopen());
  const shown = () => {
    const marked = props.actions.map((action, i) => ({ ...action, primary: i === 0 }));
    return TOUCH ? marked : marked.reverse();
  };
  return (
    <View class={TOUCH ? "w-full gap-2" : "w-full flex-row flex-wrap justify-end gap-2"}>
      <Each of={shown()} key={(action) => action.id}>
        {(action) => (
          <FlowButton
            testID={`${props.prefix}-${action().id}`}
            label={action().label}
            primary={action().primary}
            busy={action().id === "reopen" && signInFlow.reopening()}
            onPress={() => press(action())}
          />
        )}
      </Each>
    </View>
  );
}

/** A waiting card's body under its title: the sentence, the rows with Copy, the actions. */
export function FlowCardBody(props: { card: FlowCard; prefix: string; centred?: boolean }) {
  return (
    <View class="w-full gap-4">
      <Text variant="meta" tone="fg2" class={props.centred ? "text-center" : ""} testID={`${props.prefix}-sentence`}>
        {deviceCopy(props.card.sentence)}
      </Text>
      <View class="w-full gap-2">
        <Each of={props.card.rows} key={(row) => row.id}>
          {(row) => <CopyRow row={row()} testID={`${props.prefix}-${row().id}`} />}
        </Each>
      </View>
      <FlowActions actions={props.card.actions} prefix={props.prefix} />
    </View>
  );
}

/** Sign in or Create account on a start card: its spinner while its own request runs. */
export function StartButton(props: { entry: SignInEntry; testID: string; label: string; primary?: boolean; block?: boolean; onStart?: () => void }) {
  return (
    <AppButton
      testID={props.testID}
      label={props.label}
      variant={props.primary ? "primary" : "secondary"}
      block={props.block}
      size={props.block ? "md" : "sm"}
      leading={signInFlow.opening(props.entry) ? <Spinner size={14} tone={props.primary ? "primaryFg" : "fg2"} label="Opening the sign-in page" /> : undefined}
      onPress={() => (props.onStart ? props.onStart() : void signInFlow.begin(props.entry))}
    />
  );
}
