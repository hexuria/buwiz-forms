import { assertRenderEnvelope, type RenderEnvelope } from "@ebirforms/form-contracts";
import { getFormComponent } from "./forms/registry";
import "./print.css";

export function FormDocument({ envelope }: { envelope: RenderEnvelope }) {
  assertRenderEnvelope(envelope);
  const Component = getFormComponent(envelope.form.code, envelope.form.version);
  return <Component envelope={envelope} />;
}
