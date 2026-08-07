# TIN Root, Branch Code, and Multi-Branch Filing Guide

**Research cutoff:** 2026-08-07

**Status:** official-source product guidance with explicit evidence gaps

**Implementation state:** research only; no application code or database schema changed

**Execution companion:** [TIN Root, Registration Units, and Filing Scope — Implementation Plan](TIN_BRANCH_IMPLEMENTATION_PLAN_2026-08-07.md)
**Canonical vocabulary:** [BIR Taxpayer, Registration, and Filing Context](CONTEXT.md)

This guide is for product design and engineering. It is not legal or tax advice,
does not replace a taxpayer's COR/eCOR or a current BIR issuance, and does not
certify any editor as fileable. The app's current catalog, calendar, and screen
availability are product capabilities—not proof that a taxpayer must file a
return.

The two supplied ChatGPT answers were treated as hypotheses. Their core
income/VAT/percentage/withholding distinction is supported, but several detailed
claims were downgraded or rejected after checking primary BIR sources.

---

## Executive verdict

### Identifier model

Use these as separate domain values:

```text
Taxpayer TIN root:      000-000-000
Head-office code:       00000
Full filing display:    000-000-000-00000
```

The nine-digit root identifies one taxpayer. The five-digit suffix identifies a
registered head office or branch under that taxpayer. It does not create a
second taxpayer.

The base taxpayer profile should therefore own the nine-digit TIN root. Creating
the taxpayer should also create its system-reserved head-office registration
unit `00000`, so the UI has a complete hierarchy without storing `00000` inside
the taxpayer identity.

### Branch creation

The app may display a convenience suggestion such as `00001`, but it must not
claim to assign that code. No reviewed official source supports app-authoritative
sequential assignment, incrementing, gap filling, or reuse after closure.

A new branch should begin as `PendingRegistration`. It becomes a confirmed
filing identity only when the code is recorded from COR, eCOR, ORUS, or another
reviewed BIR-issued registration record. Existing taxpayers can have gaps,
closed units, or codes assigned outside the app.

### Facility is not branch

Current Forms 1901/1903 distinguish Head Office, Branch Office, and Facility.
Form 1905 also has a separate Facility Code table and enumerates facility types
such as plant, storage place, warehouse, showroom, garage, bus terminal, and real
property for lease.

Do not treat a Facility Code as the five-digit TIN branch suffix without an
official mapping. Model registered facilities separately and link them to a
responsible office only from registration evidence. This distinction is
especially important for excise, premises, production, storage, and removal
rules.

### Filing model

A branch switch is a workspace choice, not a legal filing decision.

For each exact form revision and period, the app must resolve:

- the taxpayer;
- the filing unit and branch code printed/transmitted on the return;
- the exact source units covered by the return;
- the effective tax registrations that support that result;
- effective Large Taxpayers Service status when it changes the rule;
- any property, instrument, facility, recipient, employee, or parent-return
  context;
- the official policy revision and source evidence.

Unknown or contradictory inputs produce **Review Required**, never “use the
selected branch.”

---

## What the official sources establish

### One taxpayer, one TIN

RR 11-2008, pages 1–2, says only one TIN is assigned to one taxpayer and includes
branches for purposes of securing a branch code. This supports a taxpayer-first
aggregate rather than independent legal profiles for each branch.

### Current TIN + branch-code display

RR 7-2024 section 3(B)(1), PDF page 2, gives the invoice identity example
`123-456-789-00000`. The April 2024 Form 2550Q and January 2018 Form 2551Q
instructions both say the last five digits of the 14-digit TIN refer to the
branch code.

Current Forms 1901 and 1903, page 1, show an existing taxpayer's nine-digit TIN
with `00000` in the branch segment. The separate “TIN to be issued” area is
marked for BIR completion. Together with RR 11-2008's “securing branch code”
language, this is strong support for recording the BIR result rather than having
the app authoritatively generate branch codes.

### Registration evidence belongs to each registered place

RR 7-2024 requires branch registration before commencement, location-RDO or
applicable Large Taxpayer office registration, COR/eCOR for each head
office/branch/facility, tax-type registration, and explicit updates for
transfers/cancellations. RR 15-2024 similarly requires branch registration and
COR/eCOR display and says an online store managed by a head office or branch is
an added business name, not automatically a branch.

The product must therefore store effective registration evidence, not infer
tax types or unit status from form checkboxes.

RR 7-2024 section 5(B), page 8, also records repeal of the annual ₱500
registration fee effective 2024-01-22. The app must not generate a recurring
Form 0605 branch obligation for that repealed fee. Form 0605 remains a general
payment workflow only when an actual underlying liability exists.

### Core tax-family registration matrix

RR 11-2008, PDF pages 13–14, establishes this baseline:

| Tax family | Registration/return baseline |
| --- | --- |
| Income tax | Registered at head office only. |
| VAT | Registered at head office only. |
| Percentage tax | Head-office-only consolidated treatment or registration/filing at the respective branches. |
| Withholding on compensation | Head-office-only or branch registration. |
| Creditable withholding | Head-office-only or branch registration. |
| Final withholding | Head-office-only or branch registration. |
| Periodic DST | Head-office-only or branch registration. |
| Excise | Head-office-only or branch registration, with specialist product/site rules still relevant. |

For taxpayers under the Large Taxpayers Service rule described there, income
tax, VAT, percentage tax, and withholding are consolidated. DST and excise may
remain non-consolidated. The app must store effective LTS registration/office
evidence; it must not infer this override from the EOPT micro/small/medium/large
sales-size classification.

### Exact corroborating return instructions

- Form 2550Q April 2024: a taxpayer with branches files only one consolidated
  return for the principal place/head office and all branches.
- Form 2551Q January 2018: a taxpayer may file separate returns for head office
  and branches or one consolidated head-office return; a large taxpayer files
  one consolidated return.
- Form 1600-VT 2018: the same separate-office-or-consolidated alternative is
  stated, with mandatory consolidation for large taxpayers.

These exact instructions support the resolver shape. They do not make every
other code in the same family automatically current or fileable.

---

## Filing scope is not filing venue

RR 4-2024 allows electronic/manual filing and payment regardless of RDO venue or
jurisdiction and removes the civil penalty for filing at the wrong venue. That
change answers where or through which channel filing/payment can occur. It does
not answer:

- which office is the filer;
- which branch code appears on the return;
- which source units the return covers;
- whether a return is consolidated.

Older RR 4-2008 real-property venue rules may still help identify historical
transaction context, but they must not be encoded as current venue policy
without reconciling RR 4-2024. Scope, venue, and exact form representation need
separate effective-dated policies.

RR 4-2024 and RR 7-2024 use an effectivity clause tied to publication. RR
15-2024 also uses a publication-based clause. This research verified the clauses
and BIR posting metadata, not the final calendar effectivity date for every
possible filing period. Policy records must verify that date before using the
issuance for a historical boundary.

---

## Filing-scope vocabulary for the app

The minimum product policy states are:

| Policy | Meaning |
| --- | --- |
| `TaxpayerLevel` | One return/document for the taxpayer; ordinary branch operating coverage is not the deciding dimension. |
| `HeadOfficeConsolidated` | One return under `00000` with an explicit set of covered offices. |
| `RegistrationDriven` | Head-office-consolidated or one per registered unit, determined by effective tax registrations and any verified LTS override. |
| `TransactionSpecific` | Property, instrument, transfer, facility, or other transaction determines identity/context. |
| `AdministrativeRegistration` | Updates the exact BIR registration record; not a periodic tax return. |
| `InheritLiability` | Payment follows the return, assessment, installment, penalty, or other liability being paid. |
| `SourceRecipientDocument` | Certificate/evidence follows the responsible source unit, employee, payee, or payment. |
| `InheritParent` | Attachment follows the exact parent return and cannot broaden its coverage. |
| `HistoricalOnly` | Supported only for verified historical periods/revisions. |
| `NotApplicable` | Evidence proves no obligation for this taxpayer/period. |
| `ReviewRequired` | The safe filing unit or coverage cannot be proven. |

“Consolidated” must never be a boolean on the form catalog. A resolved return
stores both:

```text
filing_unit_id
covered_source_unit_ids[]
```

The sales, purchases, expenses, employees, payments, credits, and other facts
inside a consolidated return retain their true source-unit IDs.

---

## Current 51-form catalog policy matrix

This is a product seed matrix, not a legal filing opinion. “Editor” means the
repository has a routed UI editor; “Calendar” means catalog/calendar presence
only. Neither means fileable.

Evidence labels:

- **Direct** — exact official instruction/form reviewed for the stated point.
- **Family** — RR 11-2008 directly establishes the tax-family rule, but the
  exact local code/revision still needs applicability checks.
- **Review** — current identity, title, revision, eligibility, or specialist rule
  is not sufficiently proven; new obligation generation must block.

| Local code | Current capability | Working policy | Evidence and required caution |
| --- | --- | --- | --- |
| `0605` | Editor | `InheritLiability` | Payment form has no independent branch scope; require the underlying liability. Do not generate the repealed annual registration-fee obligation after 2024-01-22. |
| `1905` | Calendar | `AdministrativeRegistration` | **Direct.** Target the exact head-office, branch, facility, or taxpayer registration record being changed. |
| `1600` | Calendar | `HistoricalOnly` / `ReviewRequired` | Withholding-family rule, but this legacy combined code is absent from the current official list reviewed. |
| `1600PT` | Calendar | `RegistrationDriven` | **Family.** Verify exact `1600-PT` revision/instructions before obligation generation. |
| `1600VT` | Calendar | `RegistrationDriven` | **Direct** 2018 instructions: separate unit returns or consolidated head-office return; LTS consolidated. |
| `1600WP` | Calendar | `RegistrationDriven` / `ReviewRequired` | Withholding family plus special winnings/prizes role; verify exact taxpayer role and revision. |
| `1601C` | Editor | `RegistrationDriven` | **Family.** Compensation withholding may be central or per registered unit; exact current revision still controls. |
| `1601E` | Calendar | `HistoricalOnly` / `ReviewRequired` | Absent from current list; current list uses `0619-E` and `1601-EQ`. |
| `1601F` | Calendar | `HistoricalOnly` / `ReviewRequired` | Absent from current list; current list uses `0619-F` and `1601-FQ`. |
| `0619F` | Editor | `RegistrationDriven` | **Family** and present in current list; verify exact period/revision. |
| `1601FQ` | Calendar | `RegistrationDriven` | **Family** and present in current list. |
| `1602` | Calendar | `HistoricalOnly` / `ReviewRequired` | Absent from current list; do not alias automatically to `1602Q`. |
| `1602Q` | Calendar | `RegistrationDriven` / `ReviewRequired` | Final-withholding family; special financial-institution applicability must be proven. |
| `1603` | Calendar | `HistoricalOnly` / `ReviewRequired` | Absent from current list; do not alias automatically to `1603Q`. |
| `1603Q` | Calendar | `RegistrationDriven` / `ReviewRequired` | Final-withholding family; fringe-benefit applicability and exact revision control. |
| `1604CF` | Calendar | `RegistrationDriven` / `ReviewRequired` | Current official list separates `1604-C` and `1604-F`; local combined code/meaning must be corrected or explicitly mapped. |
| `1604E` | Calendar | `RegistrationDriven` | Annual information return must reconcile with the effective expanded-withholding scope and monthly/quarterly records; verify exact annual instructions. |
| `0620` | Calendar | `RegistrationDriven` / `ReviewRequired` | Decedent-deposit withholding is a specialist financial workflow; the current list also contains quarterly `1621`, so cadence/effective transition needs proof. |
| `2316` | Calendar | `SourceRecipientDocument` | Per employee certificate tied to the responsible compensation-withholding unit, not an independent consolidated return. |
| `1700` | Calendar | `TaxpayerLevel` | Pure-compensation individual return; ordinary business-branch consolidation is not the selector. |
| `1701Q` | Editor | `HeadOfficeConsolidated` | **Family.** Income tax is head-office registered. Exact local mapper currently accepts only a 3-digit filer suffix, so five-digit representation remains blocked pending artifact proof. |
| `1701` | Editor | `HeadOfficeConsolidated` | **Family.** Branch count does not select the annual individual return variant. |
| `1701A` | Calendar | `HeadOfficeConsolidated` | **Family.** Eligibility/regime and exact period decide form choice, not branch count. |
| `1702Q` | Calendar | `HeadOfficeConsolidated` | **Family.** Corporate/non-individual income tax is head-office scoped. |
| `1702` | Calendar | `HistoricalOnly` / `ReviewRequired` | Generic local code is absent from the current official list; current variants are explicit. |
| `1702RT` | Editor | `HeadOfficeConsolidated` | **Family.** Verify official `1702-RT` revision and eligibility. |
| `1702EX` | Calendar | `HeadOfficeConsolidated` | **Family.** Verify official `1702-EX` revision and exempt status. |
| `1702MX` | Editor | `HeadOfficeConsolidated` | **Family.** Multiple regimes/rates do not turn branches into separate taxpayers. |
| `1704` | Calendar | `HistoricalOnly` / `ReviewRequired` | Absent from current official list reviewed; exact historical applicability is unresolved. |
| `2550M` | Calendar | `ReviewRequired` | Present in the current official list, but this research did not directly verify optional/current obligation rules. Never let it replace 2550Q from catalog presence alone. |
| `2550Q` | Editor | `HeadOfficeConsolidated` | **Direct** April 2024 instruction: one return for head office/principal place and all branches. |
| `2551Q` | Editor | `RegistrationDriven` | **Direct** January 2018 instruction: separate registered-office returns or one consolidated head-office return; LTS consolidated. |
| `2551M` | Calendar | `HistoricalOnly` / `ReviewRequired` | Absent from current official list; exact historical period must be proven. |
| `2552` | Calendar | `TransactionSpecific` / `ReviewRequired` | Securities/special percentage-tax context; do not inherit ordinary 2551Q behavior. |
| `2553` | Calendar | `TransactionSpecific` / `ReviewRequired` | Special-law/ATC context; do not inherit ordinary 2551Q behavior. |
| `2000` | Calendar | `RegistrationDriven` / `ReviewRequired` | Periodic DST follows effective DST registration under RR 11-2008. The local catalog says on-demand while the current official list says monthly; resolve cadence and exact-period rules before enabling. |
| `2000OT` | Calendar | `TransactionSpecific` / `ReviewRequired` | One-time instrument/transaction context; do not encode superseded venue assumptions. |
| `2200A` | Calendar | `ReviewRequired` | Excise requires registration plus alcohol/product, premises, and removal context. |
| `2200AN` | Calendar | `ReviewRequired` | Excise requires registration plus automobile/non-essential-goods, premises, and removal context. |
| `2200M` | Calendar | `ReviewRequired` | Excise requires mineral/product, premises, and removal context. |
| `2200P` | Calendar | `ReviewRequired` | Excise requires petroleum/product, premises, and removal context. |
| `2200T` | Calendar | `ReviewRequired` | Excise requires tobacco/product, premises, and removal context. |
| `2200C` | Calendar | `ReviewRequired` — blocked | **Severe identity drift:** current official list describes cosmetic procedures; the local catalog says coal and coke. Correct/prove the form identity before any obligation or editor work. |
| `2200S` | Calendar | `ReviewRequired` | Excise requires sweetened-beverage/product, premises, and removal context. |
| `0619E` | Editor | `RegistrationDriven` | **Family** and present in current list; verify exact period/revision. |
| `1601EQ` | Calendar | `RegistrationDriven` | **Family** and present in current list. |
| `1701MS` | Calendar | `HeadOfficeConsolidated` / `ReviewRequired` | Income-family scope is head office, but exact `1701-MS` eligibility and instructions were not independently proven here. |
| `1706` | Calendar | `TransactionSpecific` / `ReviewRequired` | Real-property transaction identity/context; current venue policy must be resolved separately. |
| `1707A` | Calendar | `TransactionSpecific` / `ReviewRequired` | Annual aggregation of applicable share transactions is not ordinary branch consolidation. |
| `1800` | Calendar | `TransactionSpecific` / `ReviewRequired` | Donor/donation identity and transfer context, not ordinary branch operations. |
| `1801` | Calendar | `TransactionSpecific` / `ReviewRequired` | Estate/decedent identity and transfer context; an estate may have its own taxpayer identity. |

### Catalog and calendar gaps adjacent to the 51

The current codebase also contains deadline/options for forms outside the
generated 51-form catalog:

| Code | Current mismatch | Working disposition |
| --- | --- | --- |
| `1606` | Global/calendar domain only | Real-property/ordinary-asset transaction; `TransactionSpecific` and `ReviewRequired`. |
| `1621` | Global/calendar domain only | Decedent-deposit withholding; specialist registration/context review. |
| `2550DS` | Global/calendar domain only | Nonresident digital-services regime; do not apply physical-branch logic. |
| `0611A` | Calendar rule only | Payment/liability context; reconcile official identity before surfacing. |
| `0613` | Calendar rule only | Payment/liability context; reconcile official identity before surfacing. |
| `1707` | Calendar rule only | Per-transfer share transaction; `TransactionSpecific` and `ReviewRequired`. |

The code also maps `1604C`/`1604F` to local `1604CF`, and identifiers drift
between `1701MS` and official-style `1701-MS`. Aliases need an explicit,
revision-aware boundary; punctuation normalization must not merge different
forms.

---

## Attachments, certificates, and supporting artifacts

The supplied text broadly asserted that all attachments inherit parent scope.
That is a useful design hypothesis but not sufficiently sourced for every exact
artifact in this research. Use this conservative matrix:

| Artifact | Candidate behavior | Safe product rule now |
| --- | --- | --- |
| SAWT | Usually follows the return claiming withholding credits. | `InheritParent` only after exact parent/revision instructions are linked; otherwise Review Required. |
| SLSP | Usually supports VAT reporting. | Must not exceed the exact 2550Q coverage; verify current submission rule. |
| QAP | Usually supports quarterly expanded withholding. | Candidate parent is the exact 1601-EQ obligation; verify current instructions. |
| MAP | Usually supports the corresponding monthly remittance. | Candidate parent is the exact monthly obligation; verify current instructions. |
| Annual employee/payee alphalists | Correspond to annual withholding information returns. | Scope must reconcile with monthly/quarterly/annual obligations; exact form pairing required. |
| AFS and annual ITR attachments | Support the taxpayer's annual income-tax return. | Cannot create one copy per branch merely because source schedules retain branch detail. |
| Form 1709 | Related-party attachment to an annual ITR where applicable. | Candidate `InheritParent`; exact eligibility/instructions required. |
| Forms 2304/2306/2307 | Recipient/payment evidence. | Use responsible withholding/source unit; do not invent a consolidated-certificate rule. |
| Form 2316 | Employee certificate. | Use responsible compensation-withholding/employer unit and reconcile with annual reporting. |
| Invoice/receipt identity | Identifies the seller office/source. | Retain its true TIN + branch code even when the later tax return is consolidated under `00000`. |

For any parent/attachment relationship, enforce:

```text
attachment.source_unit_set ⊆ parent_return.covered_source_unit_set
```

An attachment must never broaden coverage or permit the same credit/payment to
be claimed in multiple office returns.

---

## Inputs required before suggesting a form

Branch count alone is never enough. The resolver needs:

1. taxpayer ID, nine-digit TIN root, and legal-person class;
2. exact form code, revision, filing period, and policy effectivity;
3. confirmed head office and all relevant branch units with effective dates;
4. each unit's exact BIR branch code, RDO/address history, and COR/eCOR evidence;
5. effective tax-type registrations for the tax family;
6. effective LTS registration/office evidence, when the override matters;
7. source-unit identity for transactions, employees, payees, payments, and
   credits;
8. registered-facility, product, premises, or removal context for specialist
   taxes;
9. property, instrument, donor, estate, shares, or other transaction context for
   one-time/special returns;
10. parent return/liability identity for attachments and payment forms;
11. exact editor, artifact, validation, and submission capability separately.

If registration evidence changes inside a period, different sources conflict,
LTS status is unknown, or coverage cannot be partitioned without duplicates,
return Review Required.

---

## Product examples

### Income tax with two branches

```text
Taxpayer:       123-456-789
Head office:    00000
Cebu branch:    00001
Davao branch:   00004

Resolved annual/quarterly income-tax filing unit: 00000
Covered source units: [00000, 00001, 00004]
```

The form-family choice still depends on taxpayer kind, income regime, and exact
period—not on having three locations. Source records retain `00001` and `00004`;
only the return filing identity is `00000`.

### Percentage tax registered only at head office

```text
Effective percentage-tax registration: 00000 only
Resolved 2551Q obligations: one under 00000
Coverage: every applicable source office established by policy/evidence
```

### Percentage tax registered at offices

```text
Effective registrations: 00000, 00001, 00004
Resolved 2551Q obligations: three
Each obligation covers its own non-overlapping source-unit partition
```

If the evidence shows only an unexplained subset of offices, the resolver must
not guess whether unregistered branches belong in the head-office return. It
returns Review Required unless the effective policy defines that pattern.

### Branch creation suggestion

```text
Existing codes: 00000, 00004
UI suggestion: 00001
State: PendingRegistration
Required action: confirm the actual BIR branch code from registration evidence
```

The app accepts `00007` when that is what BIR evidence shows. It does not recycle
a closed `00001` merely because it is numerically available.

---

## Fact-check of the supplied ChatGPT guidance

### Accepted as supported direction

- one nine-digit TIN identifies one taxpayer;
- current full display uses a five-digit branch code;
- income tax and VAT use head-office scope;
- current 2550Q is one consolidated return;
- 2551Q and core withholding scope can depend on registration, with an LTS
  consolidation rule;
- source office and filing office must be stored separately;
- DST, excise, ONETT, transfer, and specialist forms must not inherit one generic
  consolidation boolean;
- attachment/payment scope should be linked to its parent/source context rather
  than the active branch.

### Rejected

- that the app may authoritatively assign `00001`, then `00002`, and so on;
- that a Facility Code is the same value as a branch suffix;
- that filing/payment venue establishes the filer or coverage;
- that EOPT size classification is equivalent to Large Taxpayers Service
  registration;
- that every branch owes a recurring annual Form 0605 registration-fee payment
  after the fee's 2024 repeal;
- that every catalog/calendar entry is a current legal obligation;
- that selecting a branch is enough to decide the return identity.

### Downgraded pending exact evidence

- current optional-use rules for 2550M;
- exact 1701-MS eligibility and instructions;
- detailed 1600-PT, 1621, and specialist withholding assertions not directly
  reviewed here;
- broad attachment inheritance without exact current instructions;
- current use of legacy codes absent from the reviewed official list;
- the `0620`/`1621` cadence transition and the current Form 2000 cadence;
- any real-property venue statement based only on RR 4-2008;
- the supplied time-specific 2025 annual-return deadline/extension statement,
  which is outside this architecture and was not verified here.

---

## Open evidence gaps and mandatory stop conditions

The next research/implementation pass must stop rather than guess when:

- an official source does not establish branch-code assignment or reuse;
- a Facility Code/branch-code relationship is unclear;
- a 3- or 4-digit legacy suffix has no exact historical mapping;
- the exact 1701Q artifact exposes three branch characters while the current
  filing identity contains five;
- a local code/title conflicts with the current official list, especially
  `2200C`;
- the form is absent from the current list and its historical interval is not
  proven;
- an issuance's publication/effectivity date for the period is unresolved;
- COR/eCOR, ORUS, and stored registration facts disagree;
- a tax registration changes during the return period;
- LTS status is unknown and would change consolidation;
- excise/site/product/removal or transaction/property context is incomplete;
- attachment/parent scope is not linked by exact instructions;
- a consolidated return's complete, non-duplicate source-unit coverage cannot
  be proven;
- an existing branch-coded income/VAT draft would be silently reinterpreted.

---

## Source register

All web sources below are BIR primary sources and were accessed 2026-08-07.

### S1 — Revenue Regulations No. 11-2008

- URL: <https://bir-cdn.bir.gov.ph/BIR/pdf/42151rr%2011-2008.pdf>
- Issued: 2008-09-24; BIR index publication metadata: 2008-09-26.
- Used: PDF pages 1–2 for one-TIN/branch-code identity; pages 13–14 for
  head-office, branch-registration, consolidation, and LTS family rules.

### S2 — Revenue Regulations No. 7-2024

- URL: <https://bir-cdn.bir.gov.ph/BIR/pdf/RR%20No.%207-%202024.pdf>
- Dated: 2024-04-11; BIR posting metadata: 2024-04-12.
- Used: section 3(B)(1), PDF page 2, TIN + branch-code example; section 5(A)(1),
  pages 5–6, branch registration; section 5(B), page 8, registration office and
  annual registration-fee repeal; section 5(F)–(K), pages 9–10, COR/eCOR, tax
  types, transfer/cancellation.
- Effectivity: use the issuance's publication-based clause; exact historical
  boundary still requires publication-date verification.

### S3 — Revenue Regulations No. 4-2024

- URL: <https://bir-cdn.bir.gov.ph/BIR/pdf/RR%20No.%204-%202024.pdf>
- Dated: 2024-04-11; BIR posting metadata: 2024-04-12.
- Used: section 1(a), PDF page 1, and section 3, pages 2–3, venue flexibility;
  section 4, page 3, wrong-venue penalty removal.
- Does not establish return consolidation or branch coverage.

### S4 — Revenue Regulations No. 8-2024

- URL: <https://bir-cdn.bir.gov.ph/BIR/pdf/RR%20No.%208-%202024.pdf>
- Dated/indexed: 2024-04-11; BIR posting metadata: 2024-04-12.
- Used: section 2, page 1, for EOPT micro/small/medium/large classifications by
  gross sales. This does not by itself prove administration under the Large
  Taxpayers Service.

### S5 — Revenue Regulations No. 15-2024

- URL: <https://bir-cdn.bir.gov.ph/BIR/pdf/RR%20No.%2015-2024.pdf>
- Document date/metadata reviewed: PDF page 1 carries 2024-05-03; approval stamp
  2024-08-06; BIR index posting 2024-08-15.
- Used: section 5, pages 5–6, branch/location registration and online-store
  distinction; section 6, page 6, COR/eCOR at branches/facilities.
- Effectivity: section 16 uses publication-based language; exact calendar date
  not independently established here.

### S6 — Form 2550Q April 2024 guidelines

- URL: <https://bir-cdn.bir.gov.ph/BIR/pdf/2550Q%20guidelines%20April%202024_final.pdf>
- Used: page 1, one consolidated return for principal place/head office and all
  branches; last five digits of 14-digit TIN are branch code.

### S7 — Form 2551Q January 2018 guidelines

- URL: <https://bir-cdn.bir.gov.ph/local/pdf/2551Q_%20Jan%202018%20Guide.pdf>
- Used: page 1, separate office returns or one consolidated head-office return;
  LTS consolidated; last five digits of 14-digit TIN are branch code.

### S8 — Form 1600-VT 2018 guidelines

- URL: <https://bir-cdn.bir.gov.ph/local/pdf/BIR%20Form%20No.%201600-VT%202018%20Guidelines.pdf>
- Used: page 1, separate office returns or consolidated head-office return; LTS
  consolidated; last five digits of 14-digit TIN are branch code.

### S9 — Current registration forms, October 2025

- Form 1901: <https://bir-cdn.bir.gov.ph/BIR/pdf/1901%20October%202025%20ENCS%20Final.pdf>
- Form 1903: <https://bir-cdn.bir.gov.ph/BIR/pdf/1903%20October%202025%20ENCS%20-%20Final.pdf>
- Form 1905: <https://bir-cdn.bir.gov.ph/BIR/pdf/1905%20October%202025%20ENCS%20Final.pdf>
- Used: 1901/1903 page 1 for Head Office/Branch/Facility and existing TIN +
  `00000`; 1905 page 1 for distinct full TIN and Facility Code fields.

### S10 — BIR forms inventory

- Human entry point: <https://www.bir.gov.ph/bir-forms>
- Public dataset used:
  <https://bir-cms-ws.bir.gov.ph/api/pub/templates/1654/datasets?per_page=3000>
- Access note: the public API required the BIR site origin and
  `client-website-id: 2` headers during research.
- Used to compare current official identifiers/titles with the repository's
  51-form catalog. The inventory alone does not establish a taxpayer obligation.

### S11 — ORUS guide attached to RMC No. 54-2024

- URL: <https://bir-cdn.bir.gov.ph/BIR/pdf/RMC%20No.%2054-2024%20Attachment.pdf>
- Used as corroboration that ORUS displays TIN, Registered Name, Branch Code, and
  RDO Code as distinct profile/registration data.

### S12 — Revenue Regulations No. 4-2008

- URL: <https://bir-cdn.bir.gov.ph/BIR/pdf/39893rr%20no.%204-2008.pdf>
- Used only as historical evidence that certain real-property filings are tied
  to a property transaction. Its venue mechanics must be reconciled with RR
  4-2024 before current use.

---

## Knowledge-base conclusion

The safe product rule is:

```text
Identify one Taxpayer by Tin9.
Record head office and branches as Registration Units with BIR-evidenced codes.
Record facilities separately.
Resolve form scope from effective policy + registrations + context.
Keep filing unit, source units, venue, and artifact representation distinct.
Snapshot the exact decision in every draft.
Block when evidence cannot prove complete, non-duplicate coverage.
```

The companion implementation plan turns this knowledge base into a staged
domain, persistence, migration, resolver, calendar, and UI program. Until those
gates pass, the current application must retain its README warning that it is
not an authoritative filing plan.
