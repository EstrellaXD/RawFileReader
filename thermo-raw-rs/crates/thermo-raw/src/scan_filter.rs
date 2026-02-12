//! Scan filter string parsing.
//!
//! Thermo scan filters encode acquisition parameters in a compact string
//! format, e.g.: "FTMS + p NSI Full ms [200.00-2000.00]"

use crate::types::{MsLevel, Polarity};

/// Precursor info extracted from a filter string.
#[derive(Debug, Clone)]
pub struct FilterPrecursor {
    pub mz: f64,
    pub activation: String,
    pub collision_energy: f64,
}

/// Parsed scan filter.
#[derive(Debug, Clone)]
pub struct ScanFilter {
    pub ms_level: MsLevel,
    pub polarity: Polarity,
    pub analyzer: String,
    pub scan_mode: String,
    pub mass_range: Option<(f64, f64)>,
    pub precursor: Option<FilterPrecursor>,
    pub raw_string: String,
}

/// Parse a Thermo scan filter string.
pub fn parse_filter(filter: &str) -> ScanFilter {
    let polarity = if filter.contains(" + ") {
        Polarity::Positive
    } else if filter.contains(" - ") {
        Polarity::Negative
    } else {
        Polarity::Unknown
    };

    let lower = filter.to_lowercase();
    let ms_level = if lower.contains("ms3") || lower.contains("ms 3") {
        MsLevel::Ms3
    } else if lower.contains("ms2") || lower.contains("ms 2") {
        MsLevel::Ms2
    } else {
        MsLevel::Ms1
    };

    let analyzer = if filter.contains("FTMS") {
        "FTMS".to_string()
    } else if filter.contains("ITMS") {
        "ITMS".to_string()
    } else {
        "Unknown".to_string()
    };

    let scan_mode = if filter.contains("Full") {
        "Full".to_string()
    } else if filter.contains("SIM") {
        "SIM".to_string()
    } else if filter.contains("SRM") {
        "SRM".to_string()
    } else {
        "Unknown".to_string()
    };

    let mass_range = parse_mass_range(filter);

    let precursor = if matches!(ms_level, MsLevel::Ms2 | MsLevel::Ms3) {
        parse_precursor_from_filter(filter)
    } else {
        None
    };

    ScanFilter {
        ms_level,
        polarity,
        analyzer,
        scan_mode,
        mass_range,
        precursor,
        raw_string: filter.to_string(),
    }
}

/// Extract precursor m/z, activation type, and collision energy from a filter string.
///
/// Parses patterns like "524.2648@hcd28.00" from filter strings such as:
/// "FTMS + c NSI d Full ms2 524.2648@hcd28.00 [100.0000-1060.0000]"
fn parse_precursor_from_filter(filter: &str) -> Option<FilterPrecursor> {
    let at_pos = filter.rfind('@')?;

    // Extract precursor m/z: scan backwards from '@' for the number
    let before_at = &filter[..at_pos];
    let mz_start = before_at
        .rfind(|c: char| !c.is_ascii_digit() && c != '.')
        .map(|i| i + 1)
        .unwrap_or(0);
    let mz_str = before_at[mz_start..].trim();
    if mz_str.is_empty() {
        return None;
    }
    let precursor_mz: f64 = mz_str.parse().ok()?;

    // Extract activation type (alphabetic chars after '@')
    let after_at = &filter[at_pos + 1..];
    let type_end = after_at
        .find(|c: char| c.is_ascii_digit() || c == '.')
        .unwrap_or(after_at.len());
    let activation = after_at[..type_end].to_lowercase();

    // Collision energy follows the activation type
    let ce_str = &after_at[type_end..];
    let ce_end = ce_str
        .find(|c: char| !c.is_ascii_digit() && c != '.')
        .unwrap_or(ce_str.len());
    let collision_energy: f64 = if ce_end > 0 {
        ce_str[..ce_end].parse().unwrap_or(0.0)
    } else {
        0.0
    };

    Some(FilterPrecursor {
        mz: precursor_mz,
        activation,
        collision_energy,
    })
}

fn parse_mass_range(filter: &str) -> Option<(f64, f64)> {
    let start = filter.find('[')?;
    let end = filter.find(']')?;
    let range_str = &filter[start + 1..end];
    let parts: Vec<&str> = range_str.split('-').collect();
    if parts.len() == 2 {
        let low: f64 = parts[0].trim().parse().ok()?;
        let high: f64 = parts[1].trim().parse().ok()?;
        Some((low, high))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_ftms_full_scan() {
        let filter = parse_filter("FTMS + p NSI Full ms [200.00-2000.00]");
        assert_eq!(filter.polarity, Polarity::Positive);
        assert!(matches!(filter.ms_level, MsLevel::Ms1));
        assert_eq!(filter.analyzer, "FTMS");
        assert_eq!(filter.scan_mode, "Full");
        assert_eq!(filter.mass_range, Some((200.0, 2000.0)));
        assert!(filter.precursor.is_none());
    }

    #[test]
    fn test_parse_negative_polarity() {
        let filter = parse_filter("FTMS - p NSI Full ms [100.00-1500.00]");
        assert_eq!(filter.polarity, Polarity::Negative);
    }

    #[test]
    fn test_parse_ms2() {
        let filter =
            parse_filter("FTMS + c NSI d Full ms2 524.2648@hcd28.00 [100.0000-1060.0000]");
        assert!(matches!(filter.ms_level, MsLevel::Ms2));
        assert_eq!(filter.mass_range, Some((100.0, 1060.0)));
        let precursor = filter.precursor.as_ref().unwrap();
        assert!((precursor.mz - 524.2648).abs() < 1e-4);
        assert_eq!(precursor.activation, "hcd");
        assert!((precursor.collision_energy - 28.0).abs() < 0.01);
    }

    #[test]
    fn test_parse_ms2_cid() {
        let filter = parse_filter("ITMS + c NSI d Full ms2 445.120@cid35.00 [120.00-900.00]");
        assert!(matches!(filter.ms_level, MsLevel::Ms2));
        let precursor = filter.precursor.as_ref().unwrap();
        assert!((precursor.mz - 445.12).abs() < 1e-4);
        assert_eq!(precursor.activation, "cid");
        assert!((precursor.collision_energy - 35.0).abs() < 0.01);
    }

    #[test]
    fn test_parse_ms3() {
        let filter = parse_filter(
            "ITMS + c NSI d Full ms3 524.26@hcd28.00 300.15@hcd35.00 [100.00-600.00]",
        );
        assert!(matches!(filter.ms_level, MsLevel::Ms3));
        // rfind('@') gets the last precursor (300.15), which is the direct MS3 precursor
        let precursor = filter.precursor.as_ref().unwrap();
        assert!((precursor.mz - 300.15).abs() < 0.01);
        assert_eq!(precursor.activation, "hcd");
        assert!((precursor.collision_energy - 35.0).abs() < 0.01);
    }
}
