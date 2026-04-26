use super::engine::PenaltyProfile;

pub fn lookup_compromise(unpaid_tax: f64, gross_sales: f64, profile: PenaltyProfile) -> f64 {
    if unpaid_tax <= 0.0 {
        // RMO 7-2015 Annex A: No amount due tier based on gross sales
        if gross_sales <= 100_000.0 { return 1_000.0; }
        if gross_sales <= 500_000.0 { return 3_000.0; }
        if gross_sales <= 1_000_000.0 { return 5_000.0; }
        if gross_sales <= 5_000_000.0 { return 10_000.0; }
        if gross_sales <= 10_000_000.0 { return 15_000.0; }
        if gross_sales <= 50_000_000.0 { return 20_000.0; }
        return 25_000.0;
    }
    
    // Using standard RMO 7-2015 Annex A for failure to file/pay
    match profile {
        PenaltyProfile::StandardFiling => {
            if unpaid_tax <= 5_000.0 { 1_000.0 }
            else if unpaid_tax <= 10_000.0 { 3_000.0 }
            else if unpaid_tax <= 20_000.0 { 5_000.0 }
            else if unpaid_tax <= 50_000.0 { 10_000.0 }
            else if unpaid_tax <= 100_000.0 { 15_000.0 }
            else if unpaid_tax <= 500_000.0 { 20_000.0 }
            else if unpaid_tax <= 1_000_000.0 { 30_000.0 }
            else if unpaid_tax <= 5_000_000.0 { 40_000.0 }
            else { 50_000.0 }
        }
        PenaltyProfile::Withholding => {
            // For now, use the same schedule or adjust if needed for failure to withhold.
            if unpaid_tax <= 5_000.0 { 1_000.0 }
            else if unpaid_tax <= 10_000.0 { 3_000.0 }
            else if unpaid_tax <= 20_000.0 { 5_000.0 }
            else if unpaid_tax <= 50_000.0 { 10_000.0 }
            else if unpaid_tax <= 100_000.0 { 15_000.0 }
            else if unpaid_tax <= 500_000.0 { 20_000.0 }
            else if unpaid_tax <= 1_000_000.0 { 30_000.0 }
            else if unpaid_tax <= 5_000_000.0 { 40_000.0 }
            else { 50_000.0 }
        }
    }
}
