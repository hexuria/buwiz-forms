# Data Source Provider Architecture

## Objective
Implement a robust "Connector" architecture within the desktop application. Instead of forcing all external software to adopt our specific JSON format, our app will have built-in **Data Providers** (Adapters) that know how to speak the native language of specific accounting platforms (like Taxman, QuickBooks, or Xero) and transform their data into our internal `UniversalTaxPayload`.

---

## 1. Core Architecture: The `DataProvider` Trait
We will create a plugin-like architecture in `bir-core::integration::providers`. All remote data sources will implement a standard Rust trait:

```rust
#[async_trait]
pub trait DataProvider {
    /// Unique ID (e.g., "taxman", "quickbooks", "generic")
    fn id(&self) -> &str;
    
    /// Human-readable name (e.g., "Taxman / Veritas Ledger")
    fn name(&self) -> &str;
    
    /// Fetch raw data from the remote API and transform it into our universal schema
    async fn fetch_tax_data(
        &self, 
        config: &ProviderConfig, // Contains URL, API Key, or OAuth token
        tin: &str, 
        period_start: &str, 
        period_end: &str
    ) -> Result<UniversalTaxPayload, ProviderError>;
}
```

## 2. Built-in Providers

We will start by building two concrete providers:

### A. `GenericProvider`
- **Use Case:** Custom ERPs or internal systems that want to integrate easily.
- **How it works:** It expects the remote URL to return our exact `UniversalTaxPayload` JSON (the SDK schema we defined). This is the exact code we wrote earlier in `remote.rs`.

### B. `TaxmanProvider` (Veritas Ledger)
- **Use Case:** First-party integration with our own AI accounting system.
- **How it works:** It connects to the Taxman API (e.g., `/api/reports/tax?tin=...`), fetches Taxman-specific JSON, and securely maps Taxman's `Categories` or `Ledger Lines` into the required `IncomeSource` structures for eBIRForms.

*(Future providers like `QuickBooksProvider` or `XeroProvider` will be added here, handling their own OAuth flows and mapping P&L reports to tax lines).*

---

## 3. Database & Storage
We will update our encrypted SQLite database to store configured Data Sources safely:
- `id`: Primary key
- `profile_tin`: Which taxpayer profile this belongs to
- `provider_id`: e.g., "taxman"
- `name`: e.g., "My Q1 Taxman Source"
- `credentials`: JSON blob containing API Keys or Access Tokens (safely encrypted by SQLCipher).

---

## 4. UI/UX Workflow (Desktop App)

**1. Configuration (Settings -> Data Sources)**
- User clicks "Add Data Source".
- Selects the Provider Type (Taxman vs. Generic).
- Enters the remote URL and API Key.
- Clicks "Test Connection" to verify.

**2. Form Generation (Drafting a 2551Q)**
- When the user starts a new form, they see a button: **"Auto-Fill from Taxman"**.
- Behind the scenes:
  1. GPUI triggers the `fetch_tax_data` async task.
  2. The `TaxmanProvider` queries the Taxman API for the selected quarter.
  3. The API response is mapped to a `UniversalTaxPayload`.
  4. Our `process_sync()` engine converts it into a `Form2551QDraft`.
  5. The UI routes the user directly to the Visual PDF Layout Editor, fully populated with the remote data!

---

## 5. Execution Steps
1. Define the `DataProvider` trait and `ProviderConfig` models in `bir-core`.
2. Implement the `GenericProvider` and a mock `TaxmanProvider`.
3. Add a new `data_sources` table to the SQLite schema and implement CRUD methods.
4. Build the UI in `bir-desktop` to configure these sources and trigger the fetch.

