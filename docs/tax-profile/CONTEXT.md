# BIR Taxpayer, Registration, and Filing Context

This context distinguishes the legal taxpayer from its BIR-registered offices
and from the returns filed for a period. The distinction is necessary because
one taxpayer can have several registered locations without becoming several
taxpayers.

## Identity and registration

**Taxpayer**:
The natural person, estate, trust, or legal entity recognized by BIR and
identified by one nine-digit TIN root.
_Avoid_: Branch profile, filing profile

**Taxpayer Profile**:
The reusable, effective-dated record of facts that belong to the Taxpayer as a
whole.
_Avoid_: Tax profile, branch profile

**TIN Root**:
The nine digits that identify the Taxpayer independently of any office or
branch suffix.
_Avoid_: Full TIN, branch TIN

**Registration Unit**:
A BIR-registered head office or branch office belonging to one Taxpayer and
identified by the taxpayer's TIN Root plus a Branch Code.
_Avoid_: Taxpayer profile, separate taxpayer

**Head Office**:
The principal Registration Unit whose normalized branch code is all zeroes.
_Avoid_: Base profile, first profile

**Branch**:
A non-head-office Registration Unit belonging to the same Taxpayer.
_Avoid_: Child taxpayer, branch taxpayer

**Registered Facility**:
A separately registered place such as a plant, storage place, warehouse,
showroom, garage, bus terminal, or real property for lease. A BIR Facility Code
is not assumed to be the five-digit Branch Code, and the facility is linked to
its responsible Registration Unit only from registration evidence.
_Avoid_: Branch, branch code

**Branch Code**:
The BIR registration suffix that distinguishes one Registration Unit under a
TIN Root; it is recorded from registration evidence, not treated as a second
taxpayer identifier.
_Avoid_: Generated TIN, profile number

**Branch Code Confirmation**:
A reviewed evidence decision that a five-digit Branch Code is the code recorded
by BIR for a Registration Unit. A UI suggestion or unverified user entry is not
a confirmation.
_Avoid_: Assigned by the app, auto-generated branch code

**Registration Unit Status**:
The effective-dated state of a Registration Unit, such as pending evidence,
confirmed active, confirmed closed, or legacy unresolved. It is independent of
whether a candidate Branch Code has been confirmed.
_Avoid_: Selected branch, branch-code status

**Tax-Type Registration**:
An effective-dated BIR registration connecting a tax type to a Registration
Unit, supported by a COR, eCOR, or another authoritative registration record.
_Avoid_: Enabled form, filing preference

**Large Taxpayer Service Registration**:
An effective-dated registration fact showing that the Taxpayer is administered
by the Large Taxpayers Service or an applicable Large Taxpayer office. It is not
the same as the taxpayer's EOPT micro/small/medium/large classification.
_Avoid_: EOPT tier, sales-size class

## Filing

**Form Capability**:
The degree to which the app supports a particular form revision, independently
of whether a Taxpayer must file it.
_Avoid_: Active form, filing obligation

**Filing Obligation**:
The duty to file a particular form revision for a period under a resolved
filing scope.
_Avoid_: Enabled form, calendar card

**Filing Unit**:
The Registration Unit whose TIN Root and branch code identify the filer on a
return.
_Avoid_: Selected profile, source branch

**Source Unit**:
The Registration Unit where a transaction, employee, payment, credit, asset,
or other reportable fact originated.
_Avoid_: Filing unit

**Source Attribution**:
The evidence-backed link between a reportable fact and its Source Unit,
including whether the link was entered, derived, or remains legacy unknown.
_Avoid_: Current branch, filing-unit assignment

**Return Coverage**:
The exact set of Source Units and effective registrations included in one
return.
_Avoid_: Selected branch, all data

**Filing Scope**:
The rule result that determines whether a return is head-office consolidated,
filed per registered unit, tied to a transaction, inherited from a parent
filing, historical-only, or blocked for review.
_Avoid_: Consolidated flag

**Resolved Filing Plan**:
The immutable result for one Taxpayer, exact form revision, and filing period:
zero or more Filing Obligations, not applicable, or Review Required, together
with the revisions and evidence used to decide it.
_Avoid_: Forms Set, selected profile, selected branch

**Form Workspace Preference**:
A user-owned choice to surface an optional form or workflow for a tax year. It
is not a Tax-Type Registration or proof of a Filing Obligation.
_Avoid_: Registered form, enabled obligation

**Head-Office-Consolidated Return**:
One return filed by the Head Office that covers the resolved set of applicable
Registration Units without erasing each fact's Source Unit.
_Avoid_: Head-office-only return

**Per-Registered-Unit Return**:
A separate return for each Registration Unit that holds the applicable
Tax-Type Registration for the period.
_Avoid_: Per-branch return

**Transaction-Specific Return**:
A return whose filing identity and venue follow a particular property,
instrument, transfer, or other transaction rather than the ordinary office
hierarchy.
_Avoid_: Branch return

**Filing Venue**:
The channel or place where a return or payment may be submitted. Venue is a
separate policy result and does not establish the Filing Unit or Return Coverage.
_Avoid_: Filing scope, RDO equals filing unit

## Evidence and decisions

**Registration Evidence**:
The COR, eCOR, BIR registration record, or reviewed source that substantiates a
Registration Unit and its Tax-Type Registrations.
_Avoid_: Uploaded file

**Filing Policy Evidence**:
An official issuance, exact form instruction, or other reviewed primary source
that supports an effective filing-scope rule.
_Avoid_: Catalog category, ChatGPT answer

**Policy Revision**:
An effective-dated, source-linked version of a filing rule used to resolve a
return's filing unit and coverage.
_Avoid_: Current rule, consolidated boolean

**Review Required**:
A fail-closed state used when the available registration evidence or effective
rule cannot establish one safe Filing Scope.
_Avoid_: Best guess, default branch

**Migration Decision**:
A reviewed, immutable disposition for legacy profile data that records whether
it maps to a Taxpayer and Registration Unit, remains legacy-only, or is blocked
with explicit reasons.
_Avoid_: Automatic merge, inferred branch mapping
