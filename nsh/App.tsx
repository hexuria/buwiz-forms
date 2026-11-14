// The entry. NSH mounts the root once; every later screen hangs off the frame that replaces this preview (wave 2, B3).
// This wave: the theme page (default) and the UI kit's gallery, switched by the bar at the top.
import { mount, signal, View } from "nsh";
import "./theme/fonts";
import { KitPage } from "./screens/base/KitPage";
import { ThemePage } from "./screens/base/ThemePage";
import { Segmented } from "./ui";

function Preview() {
  const page = signal("theme");
  return (
    <View class="flex-1 bg-background">
      <View class="flex-row items-center justify-center border-b border-border bg-card px-4 py-2">
        <Segmented
          label="Preview"
          testID="preview-switch"
          value={page()}
          onChange={(v) => page.set(v)}
          options={[
            { value: "theme", label: "Theme" },
            { value: "kit", label: "UI kit" },
          ]}
        />
      </View>
      <If when={page() === "kit"} else={<ThemePage />}>
        <KitPage />
      </If>
    </View>
  );
}

mount(Preview);
