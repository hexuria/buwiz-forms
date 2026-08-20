import type { RenderEnvelope } from "@ebirforms/form-contracts";

/** Development-only sample. Production startup never imports this module. */
export function devPreviewEnvelope(): RenderEnvelope {
  const rows = [
    ["PT010", "Persons exempt from VAT under Sec. 109(BB) [Sec. 116]", 10_000, 0.03, 300],
    ["PT040", "Domestic carriers and keepers of garages [Sec. 117]", 20_000, 0.03, 600],
    ["PT041", "International Carriers [Sec. 118]", 30_000, 0.03, 900],
    ["PT060", "Franchises on gas and water utilities [Sec. 119]", 40_000, 0.02, 800],
    ["PT070", "Franchises on radio/TV broadcasting companies [Sec. 119]", 50_000, 0.03, 1_500],
    ["PT090", "Overseas dispatch, message or conversation [Sec. 120]", 60_000, 0.10, 6_000],
    ["PT140", "Cockpits [Sec. 125]", 70_000, 0.18, 12_600],
    ["PT150", "Tax on amusement places [Sec. 125]", 80_000, 0.18, 14_400],
    ["PT160", "Boxing Exhibition [Sec. 125]", 90_000, 0.10, 9_000],
    ["PT170", "Professional Basketball Games [Sec. 125]", 100_000, 0.15, 15_000]
  ] as const;

  return {
    schema_version: "1.0",
    form: { code: "2551Q", version: "2018" },
    locale: "en-PH",
    taxpayer: {
      tin: "12345678900000",
      name: "Renderer Preview Corporation",
      rdo_code: "018",
      registered_address: "Olongapo City",
      zip_code: "2200",
      contact_number: "09123456789",
      email: "preview@example.com"
    },
    period: { taxable_year: 2026, month: 12, quarter: 1, label: "Q1 year ended 12/2026" },
    fields: {
      tax_period_basis: { type: "text", value: "calendar" },
      is_amended: { type: "boolean", value: false },
      number_of_attached_sheets: { type: "integer", value: 0 },
      tax_relief: { type: "boolean", value: false },
      tax_relief_specification: { type: "text", value: "" },
      item_13_election: { type: "text", value: "not_applicable" },
      schedule_1_page_2_subtotal: { type: "decimal", value: 10100 },
      total_tax_due: { type: "decimal", value: 61100 },
      creditable_tax_withheld: { type: "decimal", value: 0 },
      tax_paid_previous: { type: "decimal", value: 0 },
      other_tax_credit: { type: "decimal", value: 0 },
      other_tax_credit_description: { type: "text", value: "" },
      total_tax_credits: { type: "decimal", value: 0 },
      tax_payable: { type: "decimal", value: 61100 },
      surcharge: { type: "decimal", value: 0 },
      interest: { type: "decimal", value: 0 },
      compromise: { type: "decimal", value: 0 },
      total_penalties: { type: "decimal", value: 0 },
      total_amount_payable: { type: "decimal", value: 61100 },
      overpayment_disposition: { type: "text", value: "none" }
    },
    schedules: [
      {
        id: "schedule_1",
        columns: [
          { key: "atc", label: "ATC", alignment: "left" },
          { key: "description", label: "Tax Type", alignment: "left" },
          { key: "taxable_amount", label: "Taxable Amount", alignment: "right" },
          { key: "tax_rate", label: "Tax Rate", alignment: "right" },
          { key: "tax_due", label: "Tax Due", alignment: "right" }
        ],
        rows: rows.map(([atc, description, taxableAmount, taxRate, taxDue], index) => ({
          key: `preview-${index + 1}`,
          cells: {
            atc: { type: "text", value: atc },
            description: { type: "text", value: description },
            taxable_amount: { type: "decimal", value: taxableAmount },
            tax_rate: { type: "decimal", value: taxRate },
            tax_due: { type: "decimal", value: taxDue }
          }
        }))
      }
    ],
    validation: []
  };
}
