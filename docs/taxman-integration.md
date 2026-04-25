# Future Taxman Integration Contract

Taxman integration is intentionally not implemented in the desktop MVP. The desktop app should remain usable offline and must not depend on `/Volumes/goldcoders/digits` or a Taxman server until Taxman exposes an authenticated API.

## Intended Flow

1. The user connects a Taxman account from the desktop app.
2. Taxman returns an organization-scoped token or grants access through an OAuth-style flow.
3. The desktop app requests a form-ready export for a taxpayer TIN, form code, year, and quarter.
4. The desktop app maps the response into a local 2551Q draft for review before submission.

## First 2551Q Payload

The first API should return a versioned JSON payload with:

- `schema_version`
- taxpayer identity: TIN, registered name, RDO, address, ZIP, phone, email, taxpayer type, VAT registration flag
- filing period: form code, year, quarter
- Schedule 1 rows: ATC code, taxable amount, optional source account IDs
- Part II inputs: creditable tax withheld, tax paid in previously filed amended return
- source metadata: Taxman organization ID, export timestamp, accounting date range, generated-by user ID

The desktop app remains the source of truth for final review, PDF generation, BIR submission status, and BIR receipt confirmation.

## Explicit Non-Goals For Current MVP

- No Taxman API calls.
- No localhost webhook listener.
- No Taxman database reads.
- No dependency on the Taxman repo layout.
