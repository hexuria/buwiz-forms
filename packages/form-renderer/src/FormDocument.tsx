import { assertRenderEnvelope, type RenderEnvelope } from "@ebirforms/form-contracts";
import { Form2551Q } from "./forms/Form2551Q";
import "./print.css";

export function FormDocument({ envelope }: { envelope: RenderEnvelope }) {
  assertRenderEnvelope(envelope);
  switch (`${envelope.form.code}:${envelope.form.version}`) {
    case "2551Q:2018":
      return <Form2551Q envelope={envelope} />;
    default:
      throw new Error(
        `Unsupported HTML form ${envelope.form.code} revision ${envelope.form.version}`
      );
  }
}
