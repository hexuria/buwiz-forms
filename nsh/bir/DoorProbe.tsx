// A test fixture, not a screen: a tiny app that reaches the door the way a screen does, through ./index, from a
// mounted component. DoorProbe.test.tsx opens it. It proves that the door's wrappers bundle into an app, that a
// component can call a sync host function while it renders and an async one from a press, and that a host error
// reaches the screen as a message.
import { mount, signal, Text, View } from "nsh";
import { Button } from "nsh/base";
import { bir, doorStatus } from "./index";
import { createDemoTaxpayer, DEMO } from "./testing";

function DoorProbe() {
  const info = bir.system_info(); // once, while the component runs: a sync call needs no waiting
  const forms = signal("Forms: not counted");
  const opened = signal("Return: not opened");
  const failure = signal("No error");

  const count = () => forms.set(`Forms: ${bir.screen_forms_index().length}`);
  const open = async () => {
    createDemoTaxpayer();
    const form = await bir.screen_form_open(DEMO.tin, "2551Q", 2025, 1);
    opened.set(`Return: ${form.form_code} ${form.status}`);
  };
  const fail = () => {
    try {
      bir.set_global_hotkey(1 as unknown as string);
    } catch (error) {
      failure.set(`Error: ${(error as Error).message}`);
    }
  };

  return (
    <View class="flex-1 gap-2 p-4">
      <Text class="text-body">{`Host OS: ${info.os}`}</Text>
      <Text class="text-body">{`Sending: ${doorStatus().sending}`}</Text>
      <Text class="text-body">{forms()}</Text>
      <Text class="text-body">{opened()}</Text>
      <Text class="text-body">{failure()}</Text>
      <Button onPress={count} class="h-8 items-center justify-center rounded-(--radius-ctl) px-3">
        <Text class="text-body">Count forms</Text>
      </Button>
      <Button onPress={open} class="h-8 items-center justify-center rounded-(--radius-ctl) px-3">
        <Text class="text-body">Open return</Text>
      </Button>
      <Button onPress={fail} class="h-8 items-center justify-center rounded-(--radius-ctl) px-3">
        <Text class="text-body">Break it</Text>
      </Button>
    </View>
  );
}

mount(DoorProbe);
