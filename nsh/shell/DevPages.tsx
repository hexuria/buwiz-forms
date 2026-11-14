// The two dev-only pages of the base wave, full window like the preview they came from: the theme page and the UI kit's
// gallery behind a switch. Reached from the palette ("UI kit", "Theme page") and the dev door on an empty database; only
// where the dev environment is on (EBIR_TEST_ENV). The kit's tests and shots open the gallery through them.
import { View } from "nsh";
import { KitPage } from "../screens/base/KitPage";
import { ThemePage } from "../screens/base/ThemePage";
import { AppButton, IconButton, Segmented, Text } from "../ui";
import { app } from "./app";

/** The dev pages with a bar to go back to the app. */
export function DevPages() {
  return (
    <View testID="dev-pages" class="min-h-0 flex-1 bg-background">
      <View class="flex-row items-center gap-2 border-b border-border bg-card px-4 py-2">
        <IconButton testID="dev-back" icon="arrow-left" tooltip="Back to app" onPress={() => app.back()} />
        <View class="flex-1 flex-row items-center justify-center">
          <Segmented
            label="Preview"
            testID="preview-switch"
            value={app.view()}
            onChange={(v) => app.replace(v === "kit" ? "kit" : "theme")}
            options={[
              { value: "theme", label: "Theme" },
              { value: "kit", label: "UI kit" },
            ]}
          />
        </View>
      </View>
      <If when={app.view() === "kit"} else={<ThemePage />}>
        <KitPage />
      </If>
    </View>
  );
}

/** A door to the dev pages on a database with no taxpayer (a fresh test or dev run), so they stay one press away from the app's start. */
export function DevDoor() {
  return (
    <View testID="dev-door" class="shrink-0 flex-row items-center gap-2 border-t border-border px-4 py-2">
      <Text variant="meta" tone="fg3">Dev</Text>
      <AppButton testID="dev-kit" label="UI kit" size="sm" onPress={() => app.go("kit")} />
      <AppButton testID="dev-theme" label="Theme page" size="sm" onPress={() => app.go("theme")} />
    </View>
  );
}
