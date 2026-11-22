// Generated from the Rust modules (the declaration blocks of `bir_app_api::module()`).
// Do not edit. Regenerate with `nsh/bir/regenerate.sh`; a Rust test fails while this file is stale.

export interface HotkeyPreset {
    value: string;
    label: string;
}
export interface SystemInfo {
    os: string;
    arch: string;
    app_version: string;
    database_path: string;
    data_dir: string;
    logs_dir: string;
    /** Null when no global show chord is stored. */
    global_hotkey: string | null;
    hotkey_presets: HotkeyPreset[];
    log_retention_days: number;
    log_retention_choices: number[];
    runtime: string;
}
export interface ProfileSummary {
    id: number;
    tin: string;
    full_name: string;
    rdo_code: string;
    email: string;
    taxpayer_type: string;
    is_vat_registered: boolean;
    is_archived: boolean;
    default_form_type: string;
    applicable_forms: string[];
}
export interface FormCatalogEntry {
    code: string;
    title: string;
    category: string;
    frequency: string;
    requires_employees: boolean;
    is_deprecated: boolean;
}
export function system_info(): SystemInfo;
/** Empty string clears the chord. An unknown key is refused. */
export function set_global_hotkey(combo: string): null;
/** Days of daily log files to keep. Out of range is refused. */
export function set_log_retention(days: number): null;

export interface UpdateStatus {
    /** idle, checking, downloading, ready, failed, up_to_date, available, or installing. */
    phase: string;
    /** The release that was found, or null. */
    version: string | null;
    /** A sentence for a failure or "already current", or null. */
    message: string | null;
    /** Whether a newer release downloads without asking. */
    auto_download: boolean;
    /** The version compiled into this binary. */
    current_version: string;
}
export function update_status(): UpdateStatus;
/** Replace this install with the verified download and relaunch. */
export function install_update(): null;
/** Hide the update prompt until the next check. */
export function dismiss_update(): null;
/** Look for a release now, and download it even when auto-download is off. */
export function check_for_updates(): null;
/** `auto_update_download`. Off is `0`; anything else leaves downloading on. */
export function set_auto_update_download(enabled: boolean): null;
export function list_profiles(): ProfileSummary[];
export function forms_catalog(): FormCatalogEntry[];
export function get_setting(key: string): string | null;
export function set_setting(key: string, value: string): null;

export interface AuthState {
    /** null on a fresh install that has not chosen yet. */
    mode: "solo" | "helpdesk" | null;
    /** solo: the owner's TIN root; helpdesk: "000000000"; null if unset. */
    admin_root: string | null;
    /** solo: the owner taxpayer's PIN; helpdesk: the admin PIN setting. */
    has_admin_pin: boolean;
    has_admin_totp: boolean;
    has_recovery_key: boolean;
    /** Touch ID / account password (macOS) or Windows Hello can confirm the admin. */
    mac_login_available: boolean;
}
export interface ProfileAuth {
    has_pin: boolean;
    has_totp: boolean;
}
export interface TotpSecret {
    secret: string;
    qr_path: string;
}
export interface RecoveryKey {
    /** Shown once, e.g. "7K3M-Q9TB-0XWD-H2RE"; only its hash is kept. */
    recovery_key: string;
}
export function profile_detail(tin: string): object;
export function save_profile(profile: object): object;
export function set_profile_archived(tin: string, archived: boolean): object;
export function delete_profile(tin: string): null;
/** Sets the PIN of the branch's taxpayer (one PIN per TIN root). */
export function set_profile_pin(tin: string, pin: string | null): null;
/** Sets the authenticator of the branch's taxpayer; setting needs a current code. */
export function set_profile_totp(tin: string, secret: string | null, token?: string): null;
export function auth_state(): AuthState;
/** PIN and authenticator of a taxpayer (TIN root), shared by every branch. */
export function taxpayer_auth(tin_root: string): ProfileAuth;
/** pin: exactly 4 digits, or null to clear. */
export function set_taxpayer_pin(tin_root: string, pin: string | null): null;
/** Set (with a current code from the app) or clear (null) a taxpayer's authenticator. */
export function set_taxpayer_totp(tin_root: string, secret: string | null, code?: string): null;
export function verify_taxpayer_pin(tin_root: string, pin: string): boolean;
export function verify_taxpayer_totp(tin_root: string, code: string): boolean;
/** solo: the owner taxpayer's PIN; helpdesk: the admin PIN. false when not set up. */
export function verify_admin_pin(pin: string): boolean;
export function verify_admin_totp(code: string): boolean;
/** Help desk refuses to clear the PIN while there is no admin authenticator. */
export function set_admin_pin(pin: string | null): null;
/** Setting needs a current code; help desk refuses to clear it while there is no admin PIN. */
export function set_admin_totp(secret: string | null, code?: string): null;
/** First launch, after creating the owner's taxpayer. Refused once set up. */
export function setup_solo(tin_root: string): null;
/** First launch for a help desk: admin PIN plus a new recovery key. Refused once set up. */
export function setup_helpdesk(pin: string): RecoveryKey;
/** Admin only: make this taxpayer the owner (solo), from solo or help desk. Needs the current admin PIN or code; the taxpayer needs a PIN or authenticator and an active branch. Help desk's admin PIN and authenticator are removed. Answers a new recovery key. */
export function change_owner(tin_root: string, pin: string | null, code: string | null): RecoveryKey;
/** Admin only: switch from solo to help desk with a new admin PIN. Needs the current admin PIN or code. Answers a new recovery key. */
export function change_to_helpdesk(pin: string | null, code: string | null, new_pin: string): RecoveryKey;
/** Replace the admin recovery key. Needs the app set up. */
export function create_recovery_key(): RecoveryKey;
/** Single use: true (and the key is forgotten) on a match; case, dashes and spaces ignored. */
export function use_recovery_key(key: string): boolean;
/** Touch ID / account password (Windows Hello on Windows); false elsewhere or when cancelled. */
export function mac_login(reason: string): Promise<boolean>;
/** Archive or unarchive every branch. Archiving the admin taxpayer is refused. */
export function set_taxpayer_archived(tin_root: string, archived: boolean): null;
/** Delete the taxpayer and every branch. Refused for the admin taxpayer. */
export function delete_taxpayer(tin_root: string): null;
export function generate_totp_secret(account: string): TotpSecret;
export function delete_setting(key: string): null;

export interface DraftSummary {
    id: number;
    tin: string;
    form_code: string;
    taxable_year: number;
    quarter: number | null;
    month: number | null;
    status: "Draft" | "Queued" | "Submitted" | "Confirmed" | "Paid";
    updated_at: string;
}
export function list_drafts(tin: string, year: number): DraftSummary[];
export function get_form_draft(tin: string, form_code: string, year: number, quarter: number | null): object | null;
export function save_form_draft(tin: string, form_code: string, year: number, quarter: number | null, status: string, data: object): number;
export function list_queued(): DraftSummary[];
export function queue_form_draft(tin: string, form_code: string, year: number, quarter: number | null): number;
export function cancel_queued(tin: string, form_code: string, year: number, quarter: number | null): null;
export interface TaxDeadline {
    id: number | null;
    form_type: string;
    due_date: string;
    description: string;
}
export function list_deadlines(): TaxDeadline[];
export interface ImportResult {
    kind: "database" | "profile" | "invalid" | "cancelled";
}
export function import_archive(): Promise<ImportResult>;
export interface ExportResult {
    kind: "exported" | "cancelled";
    path?: string;
}
/** The taxpayer's profile archive under the host's save dialog. A taxpayer with a PIN needs it as `pin`; the page never passes a path — the host chooses the file. */
export function export_profile(tin: string, pin?: string | null): Promise<ExportResult>;
export interface PrintPreview {
    path: string;
}
export function form_print_html(tin: string, form_code: string, year: number, quarter: number | null): PrintPreview;
export interface BackgroundJob {
    kind: "job" | "submission";
    id: number | null;
    tin?: string;
    form_code?: string;
    taxable_year?: number;
    quarter?: number | null;
    month?: number | null;
    name: string;
    job_type: string;
    cron_expr: string | null;
    status: string;
    retries: number;
    last_run_at: string | null;
    next_run_at: string | null;
    created_at: string;
    output_log: string | null;
}
export function list_background_jobs(): BackgroundJob[];
export function delete_background_job(id: number): null;
export function archive_background_job(id: number): null;
export function purge_archived_jobs(): null;
export interface JobLogTail {
    file_name: string;
    lines: number;
    content: string;
}
export function job_log_tail(): JobLogTail;
/** A screen failure (`error` / `warn` / `info` / `debug`) for the Logs tab and the daily log file. Secrets are redacted. */
export function record_screen_log(level: string, message: string): null;
/** Next UI command from the gpui-agent host, or null when none is waiting. */
export function agent_take(): string | null;
/** Pick up filings an assistant queued on the account server. Null when there is nothing to say. */
export function pull_device_filings(): { message: string; icon: string } | null;
/** True once the main window is up. False through the gate, first launch, and sign-in. */
export function shell_set_chrome(open: boolean): null;

export interface CloudAccountStatus {
    server_url: string;
    server_url_source: string;
    signed_in: boolean;
    email: string | null;
    user_id: string | null;
    offline: boolean;
    detail: string;
}
export interface CloudSignInStart {
    user_code: string;
    verification_uri: string;
    verification_uri_complete: string;
    browser_opened: boolean;
}
export interface CloudAccountGate {
    unlocked: boolean;
    offline: boolean;
    server_url: string;
    email: string | null;
    detail: string;
}
export function cloud_account_status(): CloudAccountStatus;
export function cloud_account_gate(): CloudAccountGate;
export function cloud_account_begin_sign_in(): Promise<CloudSignInStart>;
export function cloud_account_begin_sign_up(): Promise<CloudSignInStart>;
export function cloud_account_finish_sign_in(): Promise<CloudAccountStatus>;
export function cloud_account_refresh(): Promise<CloudAccountStatus>;
export function cloud_account_sign_out(): Promise<CloudAccountStatus>;
export function cloud_account_session_opened(): null;
export function cloud_account_session_closed(): null;

export type BranchFieldName = "rdo_code" | "registered_address" | "zip_code" | "phone" | "email";
export interface BranchFacts {
    rdo_code: string;
    registered_address: string;
    zip_code: string;
    phone: string;
    email: string;
}
export interface Branch {
    tin: string;
    tin_display: string;
    tin_root: string;
    code: string;
    name: string;
    is_head_office: boolean;
    facts: BranchFacts;
    inherited: BranchFieldName[];
}
export interface Taxpayer {
    tin_root: string;
    tin_display: string;
    name: string;
    taxpayer_type: "Individual" | "Corporation" | "Partnership" | "Cooperative" | "Estate" | "Trust";
    birth_date: string | null;
    needs_grouping_review: boolean;
    /** Every branch is archived (set_taxpayer_archived). Archived taxpayers stay listed. */
    archived: boolean;
    branches: Branch[];
}
export interface NewTaxpayerInput {
    tin_root: string;
    name: string;
    taxpayer_type: Taxpayer["taxpayer_type"];
    birth_date?: string | null;
    head_office: Partial<BranchFacts>;
}
export interface TaxpayerUpdateInput {
    name?: string;
    taxpayer_type?: Taxpayer["taxpayer_type"];
    /** "YYYY-MM-DD", or null to clear. Omitted: unchanged. */
    birth_date?: string | null;
}
export interface NewBranchInput {
    branch_code: string;
    name: string;
    fields?: Partial<Record<BranchFieldName, string | null>>;
}
export interface BranchUpdateInput {
    name?: string;
    fields?: Partial<Record<BranchFieldName, string | null>>;
}
export interface TaxYearBounds { min: number; max: number; }
export interface AddedTaxYear { year: number; copied_from: number | null; forms: string[]; }
export interface YearlyFormsChange { added: string[]; removed: string[]; forms: string[]; }
/** A setup (decision 14): the forms of a year and the facts they read. Never a date, a line of business or anything personal. */
export interface Setup {
    /** A built-in id such as "non_vat_professional", or "user-<n>" for one the user saved. */
    id: string;
    name: string;
    taxpayer_types: Taxpayer["taxpayer_type"][];
    forms: string[];
    facts: {
        tax_classification: YearFactsView["facts"]["tax_classification"];
        income_tax_election: YearFactsView["facts"]["income_tax_election"];
        eopt_tier: YearFactsView["facts"]["eopt_tier"];
        business_start_date: null;
        line_of_business: null;
        is_government_withholding_entity: boolean | null;
        atc_codes: string[];
    };
    built_in: boolean;
    /** The forms are added to the year's list instead of replacing it (Employer). */
    adds_forms: boolean;
}
export interface SetupApplied {
    /** The year's forms list afterwards. */
    forms: string[];
    /** Facts written. */
    set: Fact[];
    /** Facts already set to something else and left as they are (without `replace`, or a locked 8% or graduated choice). */
    kept: Fact[];
    election_locked: boolean;
    /** Templates copied (copy_setup_from only). */
    templates_copied: string[];
    /** Templates the target already had and kept (copy_setup_from only). */
    templates_kept: string[];
    /** Forms that were on the list, are not in the setup, and stay on it: without `replace` a setup only adds forms. Empty after a replace. */
    forms_kept: string[];
}
export interface FormTemplateSummary {
    form_code: string;
    updated_at: string;
    /** How many answers the template fixes (box keys it pins). */
    answers_kept: number;
    /** How many "Stop here" switches are on. */
    marked: number;
    source: string;
}
export function list_taxpayers(): Taxpayer[];
export function get_taxpayer(tin_root: string): Taxpayer | null;
/** Refuses TIN root 000000000 (reserved for the help desk admin). */
export function create_taxpayer(input: NewTaxpayerInput): Taxpayer;
export function update_taxpayer(tin_root: string, update: TaxpayerUpdateInput): Taxpayer;
/** Refuses TIN root 000000000; moving the solo owner keeps it the admin. */
export function move_taxpayer_tin(old_root: string, new_root: string): Taxpayer;
export function create_branch(tin_root: string, input: NewBranchInput): Branch;
export function update_branch(tin: string, update: BranchUpdateInput): Branch;
export function move_branch_code(tin: string, new_code: string): Branch;
export function suggest_next_branch_code(tin_root: string): string;
export function tax_year_bounds(): TaxYearBounds;
export function list_tax_years(tin: string): number[];
export function add_tax_year(tin: string, year: number, copy_forms: boolean): AddedTaxYear;
export function yearly_forms(tin: string, year: number): string[];
export function set_yearly_forms(tin: string, year: number, form_codes: string[]): YearlyFormsChange;
/** The setups for a taxpayer type: the built-in ones, then the user's. */
export function list_setups(taxpayer_type: Taxpayer["taxpayer_type"]): Setup[];
/** Set the year's forms (replaced, or added to for an `adds_forms` setup) and the setup's facts. A fact already set to something else is kept unless `replace`; a locked 8% or graduated choice always is. Never touches name, TIN, address or contact. */
export function apply_setup(tin: string, year: number, setup_id: string, replace?: boolean): SetupApplied;
/** Save the branch's year as a setup: its forms and facts, no dates, no line of business, nothing personal. */
export function save_setup(tin: string, year: number, name: string): Setup;
/** Delete one of the user's setups; a built-in one is refused. */
export function delete_setup(id: string): boolean;
/** Start `to_tin`'s year from `from_tin`'s: templates of its forms, its forms list and the facts a setup would hold. What the target already has is kept unless `replace`. */
export function copy_setup_from(from_tin: string, to_tin: string, year: number, replace?: boolean): SetupApplied;
export function list_form_templates(tin: string): FormTemplateSummary[];
export function create_template_from_latest(tin: string, form_code: string): FormTemplateSummary;
export function create_template_from_return(draft_id: number): FormTemplateSummary;
export function delete_form_template(tin: string, form_code: string): boolean;
/** Older form page only (`screen_form_open` now carries `stop_edits` and `stoppable`): `stops` are the boxes switched on as a stop, `markable` the boxes that may be one, radios by each radio's key. */
export interface TemplateStops { stops: string[]; markable: string[]; }
/** null when the branch has no template for the form. */
export function template_stops(tin: string, form_code: string): TemplateStops | null;
/** "Stop here": on true = always a stop, false = never a stop, null = back to automatic. Creates the template row when the branch has none for the form. Works for every form; a radio is switched as its item's group box. Returns the new switches, a radio item's for each of its radios (and, for the older page, `stops` and `markable`). */
export function set_template_stop(tin: string, form_code: string, box_key: string, on: boolean | null): { stop_edits: Record<string, boolean> } & TemplateStops;
/** The annual income-tax election for one taxable year, on the head office's year facts; refused for a year whose income type is not self-employed or mixed income, and for a change once a return carrying it was filed. */
export function record_income_tax_election(tin: string, year: number, election: "graduated" | "eight_percent"): boolean;
export type IncomeTaxElectionGap = "ready" | "not_individual" | "not_business_income";
export type IndividualIncomeType = "PurelyCompensation" | "SelfEmployed" | "MixedIncome";
/** Whether record_income_tax_election would accept the year, and if not what the tax profile lacks. A read. */
export interface IncomeTaxElectionReadiness {
    taxable_year: number;
    gap: IncomeTaxElectionGap;
    taxpayer_type: Taxpayer["taxpayer_type"] | null;
    classification: string | null;
    /** Always null: the year facts hold no start date for the income type. */
    since: string | null;
    /** Always null: any date may be confirmed. */
    new_registration_after: string | null;
    /** Always empty. */
    issues: string[];
}
export function income_tax_election_readiness(tin: string, year: number): IncomeTaxElectionReadiness;
/** An individual's income type from `effective_from` (YYYY-MM-DD): the year facts' income type of that date's year, and of every later year that has a record without one. */
export function confirm_income_registration(tin: string, income_type: IndividualIncomeType, effective_from: string): boolean;
export type Fact = "birth_date" | "tax_classification" | "income_tax_election" | "eopt_tier"
    | "business_start_date" | "line_of_business" | "is_government_withholding_entity" | "atc_codes";
export interface YearFactsView {
    tin: string;
    year: number;
    facts: {
        tax_classification: "PurelyCompensation" | "SelfEmployed" | "MixedIncome" | "Corporation" | "CooperativeExempt" | "CooperativeTaxable" | "CooperativeMixed" | "EstateOrTrust" | null;
        income_tax_election: "EightPercent" | "GraduatedUnspecified" | "GraduatedOsd" | "GraduatedItemized" | null;
        eopt_tier: "Micro" | "Small" | "Medium" | "Large" | null;
        /** YYYY-MM-DD */
        business_start_date: string | null;
        line_of_business: string | null;
        is_government_withholding_entity: boolean | null;
        atc_codes: string[];
    };
    /** Which row holds each fact: the head office or this branch. */
    owner: Record<Exclude<Fact, "birth_date">, "head_office" | "branch">;
    /** Form codes on this branch's list for the year that read each fact. Empty list: not in use. */
    used_by: Record<Fact, string[]>;
    /** Facts in use that are not set. */
    not_set: Fact[];
    /** The 8% or graduated choice cannot change: a return carrying it was filed. */
    election_locked: boolean;
    /** Where the year's election was made, e.g. "2551Q Q1" or "Tax profile"; null when none is set. */
    election_source: string | null;
}
export function year_facts(tin: string, year: number): YearFactsView;
/** Sets only the keys present in `patch` (null clears). Head-office facts are written to the head office's record whichever branch `tin` names. Refuses a change to a locked election. */
export function set_year_facts(tin: string, year: number, patch: Partial<YearFactsView["facts"]>): YearFactsView;
export interface TaxTypeChip { type: string; label: string; forms: string[]; }
/** The chips: the tax types the year's forms list gives. */
export function tax_types_for_year(tin: string, year: number): TaxTypeChip[];
/** Groups of two or more annual income tax returns among `codes` that stand for one another (1700, 1701, 1701A, 1701MS; 1702, 1702RT, 1702EX, 1702MX): a taxpayer files one of a group for a year. The forms list keeps whatever the user picks; the page only warns. */
export function annual_return_conflicts(codes: string[]): string[][];
export interface RdoOption { code: string; description: string; }
export function list_rdos(): RdoOption[];
export function list_zip_codes(): string[];

export type InboxAuthMethod = "AppPassword" | "GoogleOAuth";
export interface InboxState {
    tin: string;
    /** Mailbox the poller keys on: imap_email, else the branch email. */
    inbox_email: string;
    auth_method: InboxAuthMethod;
    tracking_enabled: boolean;
    /** The stored IMAP login, "" when the mailbox falls back to the branch email. */
    imap_login: string;
    imap_host: string;
    /** An app password is stored — the secret itself never crosses the bridge. */
    has_app_password: boolean;
    /** A usable Google refresh token exists on the profile or the inbox row. */
    oauth_connected: boolean;
    oauth_inbox_email: string | null;
    oauth_updated_at: string | null;
    /** A sibling on this inbox still has a Submitted return awaiting its BIR receipt. */
    still_pending: boolean;
    matching_tins: string[];
    /** Last run of this inbox's `bir_poll_email` job, when one exists. */
    last_check_at: string | null;
    last_check_status: string | null;
    last_check_log: string | null;
}
export interface InboxCredentialsInput {
    imap_email: string;
    /** Blank means the default imap.gmail.com. */
    imap_host?: string | null;
    /** Omitted keeps the stored password; a string replaces it (spaces stripped). */
    app_password?: string | null;
}
export interface InboxVerifyResult { ok: boolean; message: string; state: InboxState; }
export interface InboxCheckResult {
    ok: boolean;
    still_pending: boolean;
    error: string | null;
    state: InboxState;
}
export function inbox_state(tin: string): InboxState;
/** Stores App Password credentials on the profile (encrypted DB); tracking stays off until Verify succeeds. */
export function set_inbox_app_password(tin: string, input: InboxCredentialsInput): InboxState;
export function set_inbox_tracking(tin: string, enabled: boolean): InboxState;
/** Clears the Google grant for this inbox across every profile sharing it. */
export function disconnect_inbox_oauth(tin: string): InboxState;
/** Runs the browser OAuth flow, then persists the grant to the shared inbox row. */
export function connect_inbox_oauth(tin: string): Promise<InboxState>;
/** Real IMAP login probe: success turns tracking on, failure turns it off. */
export function test_inbox_connection(tin: string): Promise<InboxVerifyResult>;
/** Runs the cron-side fetch→confirm poll for this inbox once, updating its job row. */
export function check_inbox_now(tin: string): Promise<InboxCheckResult>;
/** Standing OK for an assistant to file one form for this branch. The Queue button does not use it. */
export interface SendGrant { tin: string; form: string; granted_at: string; }
export function list_send_grants(tin: string): SendGrant[];
export function grant_send(tin: string, form: string): SendGrant;
export function revoke_send(tin: string, form: string): void;

export interface ScreenFormIndexEntry {
    c: string;
    t: string;
    rev: string;
    freq: string;
    pages: number;
    n: number;
    q: number;
}
export interface ScreenFormError {
    field: string;
    message: string;
    keys: string[];
    /** A notice the filer may mark checked without changing anything: the token `screen_form_check_notice` takes, and its button's label. */
    dismiss?: string;
    dismiss_label?: string;
}
export type ScreenFormStatus = "Draft" | "Queued" | "Submitted" | "Confirmed" | "Paid";
export interface ScreenFormProfilePrerequisite {
    field: "business_start_date" | "income_tax_election";
    label: string;
    message: string;
}
export interface ScreenFormOpened {
    form_code: string;
    year: number;
    period: number;
    status: ScreenFormStatus;
    editable: boolean;
    /** Missing profile context, decided by the form's own rules; advisory, not new Queue blocks. */
    profile_prerequisites?: ScreenFormProfilePrerequisite[];
    /** The period column the draft is stored under: pass it to queue and print. */
    stored_period: number | null;
    draft: object;
    values: Record<string, string>;
    options: Record<string, ScreenFormOption[]>;
    locks: Record<string, string>;
    /** Boxes the taxpayer profile filled (the page tucks and marks them); a box it left blank is not here and not locked. */
    profile: string[];
    /** Filled profile boxes the filer may open with Edit; pass the opened ones to `screen_form_fill` as `unlocked`. */
    profile_edit: string[];
    /** Opened from the Form library: a blank sample; saved once its period is set, it becomes that period's return. */
    sample: boolean;
    /** A sample's boxes shown blank while its period is not set: its period parts and everything that follows the period. */
    blank: string[];
    /** A sample's held box that cannot be typed: what it waits for (shown in the box). */
    needs: Record<string, string>;
    /** A printed box the filer cannot fill here, while it shows blank: why, in the filer's words (the schema's reviewed `why`; shown in the box). */
    whys: Record<string, string>;
    /** A blank box's reason, shown in the box: what it waits for (a rate its row's ATC, a tick box the amount that ticks it, a date the return it is started with). */
    waits: Record<string, string>;
    /** An untouched date whose message the page holds until it is edited or Check / Save is pressed (1800/1801 item 1). */
    quiet?: { field: string; keys: string[] } | null;
    /** Why each locked box is shut, in the filer's words (drawn under its item while the box shows blank). */
    lock_notes: Record<string, string>;
    /** A sample's period parts: page inputs (never draft writes) the filer sets the period with. */
    period_parts: ScreenFormPeriodPart[];
    /** A sample rebuilt for a period its filer completed. */
    period_set: boolean;
    /** An amended return's filed version, kept in its history (null when never amended): open it with `screen_form_filed_original`. */
    filed_original?: { id: number; status: string; filed_on: string | null } | null;
    /** Set on the read-only view of a filed original. */
    filed_original_view?: { status: string; filed_on: string | null };
    /** When a return was sent to BIR and confirmed (a read-only view says so). */
    submitted_at: string | null;
    /** The send `submitted_at` names was a dry run: the worker only simulated it and nothing reached BIR. False for a real send and for every return sent before this was recorded. */
    submitted_dry_run: boolean;
    /** That send went to the local mock relay (docs/relay.md), not to BIR. False for every other send. */
    submitted_via_relay: boolean;
    confirmed_at: string | null;
    /** A new amended return (`screen_form_amend`): the status of the filed return it amends. */
    amends?: ScreenFormStatus;
    /** Boxes another answer turns off right now, each with its tag ("Needs item 13 = Yes"; the page fades them). */
    off: Record<string, string>;
    /** Each party's Surcharge, Interest and Compromise boxes: locked at the computed amounts until Edit penalties opens them (sent as `unlocked`). */
    penalties: ScreenFormPenalties[];
    /** Each pooled address (by its first line) as the draft holds it: the page shows this, never the field map's cut lines. */
    pools: Record<string, string>;
    /** A stored Draft whose profile boxes the filer did not edit followed a later tax-profile change (saved): what changed ("RDO 050 → 043"). Empty when nothing moved. */
    profile_updates?: string[];
    /** A Draft saved before it recorded its profile baseline (or reverted to Draft): its unedited profile boxes that differ from the profile now ("RDO 039 (profile 043)"), not changed; `screen_form_use_profile` applies them. */
    profile_differs?: string[];
    /** One-time notes this opening offers (`key`, `text`); show the ones `screen_form_notes_seen` records. */
    notes?: { key: string; text: string }[];
    /** Boxes filled from the previous return (`last`) or from the template (`template`) when this return was created. Stored with the return. A radio item is named by each of its radios, the keys the page draws. */
    kept: Record<string, "last" | "template">;
    /** Automatic stops as box keys: boxes the user filled in the previous return that the carry rule does not copy (amounts and dates). Stored with the return when it is created. Empty for a first return. */
    auto_stops: string[];
    /** The user's own "Stop here" switches for this branch and form: true = always a stop, false = never. */
    stop_edits: Record<string, boolean>;
    /** Boxes that may carry a "Stop here" switch (user boxes: not computed, not locked, not profile), a radio item by each of its radios. */
    stoppable: string[];
    errors: ScreenFormError[];
    /** A Draft's tax facts that are not set and change what it files: messages on the row of the fact's card (Queue refuses while any is here). */
    fact_errors?: ScreenFormError[];
    /** A Draft's cards: one per fact its form reads that the tax profile has not set for its year. */
    asks?: ScreenFormAsk[];
    /** The filled profile boxes Edit may open, and the boxes that answer a fact with a writable box of its own (Q1 P1b), that "Save to profile" can write back, with the field, a note for the row, and `profile`: the value the profile makes for the box (the page asks only when the answer differs). */
    profile_save?: Record<string, { field: "name" | "rdo_code" | "registered_address" | "zip_code" | "phone" | "email" | "line_of_business" | ScreenFormFact; label: string; note: string; profile?: string }>;
    /** The profile field ids (`profile_save`'s `field`) whose "Save to profile?" answer is already remembered for this return's taxpayer and tax year (Q1·a): the page does not ask them again. */
    profile_asked?: string[];
    /** The host could not read remembered profile asks; the page does not ask again in this session. */
    profile_ask_unknown?: boolean;
    /** Boxes whose opening values are guesses for the filer to confirm (Draft only). */
    guessed?: string[];
    /** A return past Queued: what the tax profile changed after it was filed ("RDO 039 (profile now 043)", "EOPT tier"). The return keeps what was filed. */
    profile_changed?: string[];
}
export type ScreenFormFact = "birth_date" | "tax_classification" | "income_tax_election" | "eopt_tier"
    | "business_start_date" | "line_of_business" | "is_government_withholding_entity" | "atc_codes";
export interface ScreenFormAsk {
    fact: ScreenFormFact;
    /** The box the card sits at; null: the top of the first part. */
    key: string | null;
    label: string;
    control: "date" | "choice" | "text";
    /** A choice card's answers: `value` is what `set_year_facts` takes, `short` its chip label (F2). */
    choices: { value: unknown; label: string; short?: string }[] | null;
    /** The card's sentence: what is asked, and that the answer goes to the tax profile for every later return. */
    note: string;
}
export interface ScreenFormPeriodPart {
    key: string;
    part: "year" | "month" | "year_end_month" | "day" | "quarter" | "date";
    /** For a quarter's tick box: which quarter. */
    quarter: number | null;
}
export interface ScreenFormExisting {
    form_code: string;
    year: number;
    period: number;
    label: string;
    status: ScreenFormStatus;
    updated_at: string;
    total_payable: number | null;
    /** What the page may offer: `load` (a draft), `view` (read-only), `amend` (reached BIR, and the form has an amended-return answer), `keep`. */
    actions: ("load" | "view" | "amend" | "keep")[];
    submitted_at: string | null;
    confirmed_at: string | null;
}
export interface ScreenFormSamplePeriod {
    complete: boolean;
    /** A stored return of the form already has this period. */
    existing?: ScreenFormExisting;
    /** No stored return: a fresh sample built for exactly this period. */
    sample?: ScreenFormOpened;
    /** What `screen_form_open` takes for it: year, period, and the exact stored key (`key`, an existing answer) or the key a new return starts under (`new_key`, with `new_key_from`: the sample it was offered for, its `created_at`, which the open passes back); null: start it with New return. */
    opens?: { year: number; period: number; key: number | null; new_key?: number | null; new_key_from?: string | null } | null;
}
export interface ScreenFormPenalties {
    keys: string[];
    /** The filer entered the amounts: they file instead of the computed ones; blanks go back to the computed. */
    edited: boolean;
    /** The amounts are kept as entered before penalties were computed (1701Q, 1701): "Use the computed amounts" works once Edit opens them. */
    kept?: boolean;
    /** The party is on the return as it stands (a spouse's column with no spouse is not): only then is Edit offered. */
    applies: boolean;
}
export interface ScreenFormFilled {
    draft: object;
    profile_prerequisites?: ScreenFormProfilePrerequisite[];
    values: Record<string, string>;
    options: Record<string, ScreenFormOption[]>;
    rejected: Record<string, string>;
    penalties: ScreenFormPenalties[];
    /** Each pooled address (by its first line) as the draft holds it: the page shows this, never the field map's cut lines. */
    pools: Record<string, string>;
    /** A sample's boxes still blank (null for a return). */
    blank?: string[] | null;
    /** Rows the filer typed that this edit removed (an answer that closes a section, a cleared ATC): the page asks `message` first and offers Undo with `done`. */
    removes?: { message: string; done: string; rows: number } | null;
    /** Locks this fill opened or shut (a folded item, a box an answer shuts): the reason, or null to open the box; `relock_notes` holds each one's note. */
    relocks?: Record<string, string | null>;
    relock_notes?: Record<string, string | null>;
}
export interface ScreenFormValidated {
    draft: object;
    values: Record<string, string>;
    options: Record<string, ScreenFormOption[]>;
    errors: ScreenFormError[];
    /** Boxes another answer turns off right now, each with its tag. */
    off: Record<string, string>;
    penalties: ScreenFormPenalties[];
    /** Each pooled address (by its first line) as the draft holds it: the page shows this, never the field map's cut lines. */
    pools: Record<string, string>;
    /** A sample's boxes still blank (null for a return). */
    blank?: string[] | null;
    /** A Draft's tax facts that are not set and change what it files (see `ScreenFormOpened.fact_errors`). */
    fact_errors?: ScreenFormError[];
}
export interface ScreenFormOption {
    value: string;
    label: string;
    /** Selected by the typed draft when multiple choices share a code. */
    selected?: boolean;
}
export interface ScreenFormSaved {
    /** A Form library sample: false when its period already has a stored return (nothing saved; `existing` and `opens` are the offer). */
    saved?: boolean;
    existing?: ScreenFormExisting;
    /** A saved sample: open the return it became with these (`screen_form_open`'s year, period and key). */
    opens?: { year: number; period: number; key: number | null };
    id?: number;
    status?: ScreenFormStatus;
    stored_period?: number | null;
    draft?: object;
}
export function screen_form_schema(form_code: string): object;
export function screen_forms_index(): ScreenFormIndexEntry[];
/** `key`: the exact stored return (a transaction's MMDD, 2553's quarter; gone, the open fails), else the latest of the period. `new_key`: no stored return, a new one started under that key (the typed date); `new_key_from`: the sample the host offered `new_key` for (its `created_at`). */
export function screen_form_open(tin: string, form_code: string, year: number, period: number, key?: number | null, new_key?: number | null, new_key_from?: string | null): Promise<ScreenFormOpened>;
/** The Form library's blank sample of a form for this taxpayer: reads no stored return and computes no penalty (its draft is marked). Saved once its period is set, it becomes that period's return; queue and print take the saved return. */
export function screen_form_sample(tin: string, form_code: string): Promise<ScreenFormOpened>;
/** A sample's typed period parts (key -> value): incomplete, an existing stored return, or a fresh sample for that period. */
/** A NEW amended return copied from a stored one that reached BIR; nothing is stored until it is saved. The filed version is kept in this return's history. */
export function screen_form_amend(tin: string, form_code: string, year: number, period: number, key?: number | null): Promise<ScreenFormOpened>;
/** An amended return's filed version from its history, read-only. */
export function screen_form_filed_original(form_code: string, snapshot_id: number): Promise<ScreenFormOpened>;
export function screen_form_sample_period(tin: string, form_code: string, parts: Record<string, string>): Promise<ScreenFormSamplePeriod>;
/** `unlocked`: profile boxes the page has open (opened with Edit, or left blank by the profile); their edits stay on this return and never reach the profile. `forget`: choice boxes whose answer is dropped before the edits apply, so the form asks again (the agent's fill left them unanswered). */
export function screen_form_fill(form_code: string, draft: object, edits: Record<string, string | number | boolean | null>, unlocked?: string[], forget?: string[]): ScreenFormFilled;
/** "Use profile values" on a draft that differs from the tax profile (`profile_differs`), and "Update from tax profile": every profile box the filer did not edit takes the profile's value, and they follow later profile changes. `facts`: facts just answered from the return's cards; then only the boxes they fill move, and one the filer typed or picked on this return keeps its value. Nothing is saved; save the draft. `asks`: the cards still asked. */
export function screen_form_use_profile(form_code: string, year: number, period: number, draft: object, facts?: ScreenFormFact[]): ScreenFormFilled & { asks: ScreenFormAsk[] };
export function screen_form_validate(form_code: string, draft: object): Promise<ScreenFormValidated>;
/** Save a draft. A Form library sample saves only once its period is set, never over a stored return of that period (answering the offer instead); an entry-numbered form takes its next free entry. */
export function screen_form_save(form_code: string, draft: object, expected?: object | null): ScreenFormSaved;
/** Print preview of a saved draft (opens the filled official sheet in the in-app preview window). */
export function screen_form_print(form_code: string, draft: object): { path: string };
/** Print preview of a Form library sample as the page holds it, staged edits included and its held boxes blank. Nothing is saved and no period is needed; the preview then follows `screen_form_preview_sync`. */
export function screen_form_print_sample(form_code: string, draft: object, edits: Record<string, string | number | boolean | null>, unlocked?: string[]): { path: string };
/** Live sync: re-fill the open print preview for this form from the draft plus staged edits (in memory). No-op when no preview is open. */
export function screen_form_preview_sync(form_code: string, draft: object, edits: Record<string, string | number | boolean | null>, unlocked?: string[]): boolean;
/** What Queue does with the tax facts this return reads that are not set: `blocks` stop it (validation messages on the row of the fact's card), `warns` are said once in the Queue confirmation. Queue itself refuses while anything blocks. */
export function screen_form_queue_check(form_code: string, draft: object): { blocks: ScreenFormError[]; warns: string[] };
/** "Save to profile too": the boxes `keys` (`ScreenFormOpened.profile_save`) of a saved Draft go to the tax profile through the function owning each field; the profile boxes equal the profile again and the return is saved. A fact box's answer goes as the profile takes it; one with no profile meaning is refused ("That answer cannot be saved to the tax profile.") and nothing is written. `saved`: the fields' labels; `notes`: a branch that followed the head office now has its own value. */
export function screen_form_save_to_profile(form_code: string, year: number, period: number, draft: object, keys: string[]): { draft: object; values: Record<string, string>; pools: Record<string, string>; saved: string[]; notes: string[] };
/** Remember the "Save to profile?" answer for a taxpayer, tax year and profile field (`ProfileField::id()`, e.g. "line_of_business", "birth_date", "rdo_code"): `profile_ask:<tin>:<year>:<field>` in the host settings, so the page asks each field once (Q1·a). `answer` is "save" or "only"; the return itself is not changed and a later tax year asks again. */
export function screen_form_profile_asked(tin: string, year: number, field: string, answer: "save" | "only"): null;
/** Queue a saved draft; 1601C and 2551Q use queue_form_draft instead. */
export function screen_form_queue(form_code: string, draft: object): number;
/** Take a queued draft out of the queue; 1601C and 2551Q use cancel_queued. */
export function screen_form_cancel_queue(form_code: string, draft: object): null;
/** Record one-time notes (`ScreenFormOpened.notes` keys) as seen, each only if not yet recorded; the keys this call recorded, which are the notes to show. */
export function screen_form_notes_seen(keys: string[]): string[];
/** Mark a notice (`ScreenFormError.dismiss`) of a saved draft checked; the return is not changed. */
export function screen_form_check_notice(form_code: string, draft: object, token: string): null;

export interface DeadlineRow {
    tin: string;
    tin_root: string;
    branch_code: string;
    branch_name: string;
    taxpayer_name: string;
    branch_count: number;
    form_code: string;
    form_name: string;
    frequency: "Monthly" | "Quarterly" | "Annual" | "As needed";
    taxable_year: number;
    /** Month 1-12, quarter 1-4, 0 for an annual return, or an as-needed return's stored period. */
    period: number;
    period_label: string;
    due_date: string;
    /** "Not started" | "Draft" | "Queued" | "Submitted" | "Confirmed" | "Paid" */
    status: string;
    draft_id: number | null;
    /** The saved return's stored key (`screen_form_open`'s `key`), null when none is saved. */
    return_key: number | null;
}
export function deadlines_between(from: string, to: string): DeadlineRow[];
export interface AsNeededReturn {
    draft_id: number;
    period: number;
    return_key: number | null;
    status: string;
    updated_at: string;
}
export interface AsNeededForm {
    tin: string;
    tin_root: string;
    branch_code: string;
    branch_name: string;
    taxpayer_name: string;
    branch_count: number;
    form_code: string;
    form_name: string;
    taxable_year: number;
    /** What dates a return ("Removal"), null when the form has no trigger-date rule. */
    trigger: string | null;
    /** When it is due, as the form's guide says. */
    due_rule: string;
    /** Saved returns of the form that hold no trigger date yet. */
    undated: AsNeededReturn[];
}
export function as_needed_forms(year: number): AsNeededForm[];
export interface ReturnRow {
    id: number;
    tin: string;
    tin_root: string;
    branch_code: string;
    branch_name: string;
    taxpayer_name: string;
    branch_count: number;
    form_code: string;
    taxable_year: number;
    period: number;
    period_label: string;
    status: string;
    updated_at: string;
    /** The return's stored key (`screen_form_open`'s `key`): opens exactly this return. */
    return_key: number | null;
    /** The worker sent it to the local mock relay, not to BIR (stored at submission; false for drafts and real sends). */
    submitted_via_relay: boolean;
}
export function list_returns(which: "filed" | "saved" | "all"): ReturnRow[];

export interface LegacySkippedFile {
    /** The file's name, without its folder. */
    file: string;
    /** Why it was not read, in words ("not an XML file", "could not decrypt return" ...). */
    reason: string;
}
export interface LegacyScanSummary {
    /** "scanned": there is something to import and the host holds the scan for legacy_import; "empty": an eBIRForms folder with no
     *  taxpayer and no filed return in it; "not_ebirforms": the folder has no profile or IAF_RDO_Copy folder. Nothing is held unless "scanned". */
    kind: "scanned" | "empty" | "not_ebirforms";
    /** The number to hand legacy_import; 0 unless kind is "scanned". */
    scan: number;
    /** The folder that was read. */
    folder: string;
    /** Taxpayers (TIN and branch) the profiles and filed returns name. */
    taxpayers: number;
    /** Filed returns (Final Copies) that can be imported. */
    filed_returns: number;
    /** Saved drafts (savefile): counted, never imported. */
    drafts: number;
    /** Files that were not read, with the reason. */
    skipped: LegacySkippedFile[];
}
export type LegacyScanAnswer = LegacyScanSummary | { kind: "cancelled" };
export interface LegacyStandardFolder {
    index: number;
    path: string;
}
/** The standard places the old program installs to (C:\eBIRForms, also on the system drive) that exist on this computer: always empty off Windows. */
export function legacy_standard_folders(): LegacyStandardFolder[];
/** Opens the host's folder dialog and reads the eBIRForms folder the person picks (the installation, its profile or its IAF_RDO_Copy folder). Takes no path. */
export function legacy_scan_folder(): Promise<LegacyScanAnswer>;
/** Reads the standard folder at `index` of legacy_standard_folders, looked up again now. */
export function legacy_scan_standard(index: number): LegacyScanSummary;
export interface LegacyImportReport {
    taxpayers_created: number;
    taxpayers_updated: number;
    taxpayers_unchanged: number;
    /** Empty details of a taxpayer filled from the old program (what was already there is kept). */
    fields_filled: number;
    returns_imported: number;
    returns_already_present: number;
    /** Files the scan skipped. */
    skipped: number;
}
/** Imports the scan the person reviewed (`scan` from its summary). Refused when there is none, it is another one, or the folder or the open database changed since. */
export function legacy_import(scan: number): LegacyImportReport;
/** Lets go of the held scan (the review was cancelled). */
export function legacy_scan_cancel(): null;
/** One return that was filed in BIR's official eBIRForms and imported. Records only: no file bytes, no field values, nothing to send. */
export interface LegacyFiledReturn {
    /** The row's number, to key a list by. */
    id: number;
    /** The form as the old program named it: the id it matched to a form this app knows ("1601Cv2018"), else the name in the file's name. */
    form: string;
    /** This app's code for that form ("1601C"), or null when it has none. */
    form_code: string | null;
    /** The period as the file's name wrote it: "062025" (month), "2026Q1" (quarter), "122026Q1" (fiscal quarter), "2025" (year), "03152026" (a date). */
    period: string;
    /** The amendment's number ("V2" is 2), or null for an original. */
    amendment_version: number | null;
    /** True for the copy that was e-mailed to BIR. */
    email_copy: boolean;
    /** The file's name in the old program's folder, without the e-mail address an e-mailed copy's name carries. */
    source_file: string;
    /** When it was imported, as the database writes it ("2026-11-17 08:30:00", UTC). */
    imported_at: string;
}
/** The returns imported from the old program for one branch's TIN (12 to 14 digits, dashes allowed), oldest import first; none for a TIN the import never named. */
export function list_legacy_filed_returns(tin: string): LegacyFiledReturn[];
