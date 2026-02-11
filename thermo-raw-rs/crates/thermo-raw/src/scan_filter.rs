//! Scan filter string parsing.
//!
//! Thermo scan filters encode acquisition parameters in a compact string
//! format, e.g.: "FTMS + p NSI Full ms [200.00-2000.00]"

use crate::types::{MsLevel, Polarity};

/// Parsed scan filter.
#[derive(Debug, Clone)]
pub struct ScanFilter {
    pub ms_level: MsLevel,
    pub polarity: Polarity,
    pub analyzer: String,
    pub scan_mode: String,
    pub mass_range: Option<(f64, f64)>,
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

    let ms_level = if filter.contains("ms2") || filter.contains("ms 2") {
        MsLevel::Ms2
    } else if filter.contains("ms3") || filter.contains("ms 3") {
        MsLevel::Ms3
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

    // Parse mass range [low-high]
    let mass_range = parse_mass_range(filter);

    ScanFilter {
        ms_level,
        polarity,
        analyzer,
        scan_mode,
        mass_range,
        raw_string: filter.to_string(),
    }
}

fn parse_mass_range(filter: &str) -> Option<(f64, f64)> {
    // Look for pattern [low-high] or [low.xx-high.xx]
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
    }

    #[test]
    fn test_parse_negative_polarity() {
        let filter = parse_filter("FTMS - p NSI Full ms [100.00-1500.00]");
        assert_eq!(filter.polarity, Polarity::Negative);
    }

    #[test]
    fn test_parse_ms2() {
        let filter = parse_filter("FTMS + c NSI d Full ms2 524.2648@hcd28.00 [100.0000-1060.0000]");
        assert!(matches!(filter.ms_level, MsLevel::Ms2));
        assert_eq!(filter.mass_range, Some((100.0, 1060.0)));
    }
}
