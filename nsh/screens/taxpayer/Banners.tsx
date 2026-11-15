// The two bands the Profile and Forms tabs share: "Replace what is set?" after a setup or copy kept set things, and the
// warning for two annual income tax returns of one kind on a year's list.
import { View } from "nsh";
import { AppButton, Banner, Icon, Text } from "../../ui";
import { confirmReplace, keepSet, shared } from "./actions";
import { annualConflictSentence, replaceSentence } from "./logic";

/** "Replace what is set?": Keep mine or Replace. */
export function ReplaceBanner() {
  return (
    <If when={shared.replaceAsk()}>
      {(a) => (
        <View class="mb-4 w-full">
          <Banner testID="tp-replace">
            <Text class="min-w-0 flex-1" testID="tp-replace-text">{replaceSentence(a())}</Text>
            <AppButton testID="tp-replace-keep" label="Keep mine" variant="ghost" size="sm" onPress={keepSet} />
            <AppButton testID="tp-replace-yes" label="Replace" variant="primary" size="sm" onPress={confirmReplace} />
          </Banner>
        </View>
      )}
    </If>
  );
}

/** Two annual income tax returns of one kind: a warning, never a removal. */
export function AnnualConflictBanner(props: { group: string[]; year: number }) {
  return (
    <View class="mb-4 w-full">
      <Banner testID={`tp-annual-conflict-${props.group.join("-")}`}>
        <Icon name="triangle-alert" tone="warning" />
        <Text class="min-w-0 flex-1">{annualConflictSentence(props.group, props.year)}</Text>
      </Banner>
    </View>
  );
}
