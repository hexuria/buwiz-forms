import { mount, Scroll, Text, View, signal, safeArea } from "nsh";
import { Input, Button } from "nsh/base";

function Row(props: { n: number }) {
  return <View style={{ padding: 16, borderBottomWidth: 1, borderColor: "#ddd" }}><Text>{`Row ${props.n}`}</Text></View>;
}

function App() {
  const a = signal(""), b = signal(""), c = signal("");
  return (
    <View style={{ flex: 1, paddingTop: safeArea().top, backgroundColor: "#ffffff" }}>
      <Text style={{ padding: 16, fontSize: 18 }}>Kit keyboard test</Text>
      <Scroll style={{ flex: 1 }} keyboardShouldPersistTaps="handled">
        <View style={{ padding: 16 }}><Input value={a()} onChange={({ value }) => a.set(value)} placeholder="Field A (top)" /></View>
        {Array.from({ length: 12 }, (_, i) => <Row n={i + 1} />)}
        <View style={{ padding: 16 }}><Input value={b()} onChange={({ value }) => b.set(value)} placeholder="Field B (middle)" /></View>
        {Array.from({ length: 12 }, (_, i) => <Row n={i + 13} />)}
        <View style={{ padding: 16 }}><Input value={c()} onChange={({ value }) => c.set(value)} placeholder="Field C (last)" /></View>
        <View style={{ padding: 16 }}><Button onPress={() => {}}><Text>Last button</Text></Button></View>
      </Scroll>
      <View style={{ padding: 12, paddingBottom: 12 + safeArea().bottom, borderTopWidth: 1, borderColor: "#ddd" }}><Text>Bottom bar (outside the scroll)</Text></View>
    </View>
  );
}

mount(App);
