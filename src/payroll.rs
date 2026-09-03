//! `libbiz::payroll` -- payroll computation.
//!
//! WHAT:    Pure compute: given hours and deductions per
//!          employee, compute gross, deductions, and net for
//!          each. The reconciliation (FR-4) is a property of
//!          the math: gross = net + deductions.
//! WHY:     Payroll is the second deferred FR (FR-4). Phase 6
//!          ships the smallest end-to-end shape.
//! LAYER:   Entity. Pure: same inputs, same result.
//! DEPENDS: stdlib only.
//! USED BY: `bin/payroll.rs` (the driver).

use std::path::Path;

/// One employee's payroll line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PayrollLine {
    pub employee: String,
    pub cost_center: String,
    pub hours: i64,
    pub rate_minor_per_hour: i64,
    pub gross: i64,
    pub deductions: i64,
    pub net: i64,
}

/// Read a TSV with header `employee\thours\trate\tcost_center`
/// and return the rows as raw (employee, hours, rate, cc) tuples.
pub fn read_hours(path: &Path) -> Result<Vec<(String, i64, i64, String)>, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("read hours {}: {e}", path.display()))?;
    let mut lines = text.lines().filter(|l| !l.trim().is_empty());
    let _header = lines.next().ok_or_else(|| "empty hours file".to_string())?;
    let mut out: Vec<(String, i64, i64, String)> = Vec::new();
    for line in lines {
        let cols: Vec<&str> = line.split('\t').collect();
        if cols.len() != 4 {
            return Err(format!("hours row: expected 4 columns, got {}", cols.len()));
        }
        let hours: i64 = cols[1].parse()
            .map_err(|e| format!("hours: not an integer: {e}"))?;
        let rate: i64 = cols[2].parse()
            .map_err(|e| format!("rate: not an integer: {e}"))?;
        out.push((cols[0].to_string(), hours, rate, cols[3].to_string()));
    }
    Ok(out)
}

/// Read a TSV with header `employee\tdeduction` and return the
/// per-employee deduction total.
pub fn read_deductions(path: &Path) -> Result<std::collections::HashMap<String, i64>, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("read deductions {}: {e}", path.display()))?;
    let mut lines = text.lines().filter(|l| !l.trim().is_empty());
    let _header = lines.next().ok_or_else(|| "empty deductions file".to_string())?;
    let mut out: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
    for line in lines {
        let cols: Vec<&str> = line.split('\t').collect();
        if cols.len() != 2 {
            return Err(format!("deductions row: expected 2 columns, got {}", cols.len()));
        }
        let ded: i64 = cols[1].parse()
            .map_err(|e| format!("deduction: not an integer: {e}"))?;
        *out.entry(cols[0].to_string()).or_insert(0) += ded;
    }
    Ok(out)
}

/// Compute each payroll line: gross = hours * rate;
/// net = gross - deductions. Reconciliation invariant:
/// net + deductions = gross (FR-4).
pub fn compute(
    hours: &[(String, i64, i64, String)],
    deductions: &std::collections::HashMap<String, i64>,
) -> Result<Vec<PayrollLine>, String> {
    let mut out: Vec<PayrollLine> = Vec::new();
    for (employee, hours, rate, cc) in hours {
        let gross = hours.checked_mul(*rate)
            .ok_or_else(|| format!("payroll: overflow on {hours}*{rate}"))?;
        let deductions = deductions.get(employee).copied().unwrap_or(0);
        let net = gross - deductions;
        out.push(PayrollLine {
            employee: employee.clone(),
            cost_center: cc.clone(),
            hours: *hours,
            rate_minor_per_hour: *rate,
            gross,
            deductions,
            net,
        });
    }
    Ok(out)
}
