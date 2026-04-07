use anyhow::{anyhow, Result};

/// Parse a human-readable size string like "100K", "1MB", "2.5G" into bytes.
pub fn parse_size(input: &str) -> Result<u64> {
    let s = input.trim();
    if s.is_empty() {
        return Err(anyhow!("empty size"));
    }
    let lower = s.to_ascii_lowercase();
    let stripped = lower
        .strip_suffix("ib")
        .or_else(|| lower.strip_suffix('b'))
        .unwrap_or(&lower);
    let (num_part, mult) = if let Some(n) = stripped.strip_suffix('k') {
        (n, 1024u64)
    } else if let Some(n) = stripped.strip_suffix('m') {
        (n, 1024u64 * 1024)
    } else if let Some(n) = stripped.strip_suffix('g') {
        (n, 1024u64 * 1024 * 1024)
    } else if let Some(n) = stripped.strip_suffix('t') {
        (n, 1024u64 * 1024 * 1024 * 1024)
    } else {
        (stripped, 1u64)
    };
    let num: f64 = num_part
        .trim()
        .parse()
        .map_err(|_| anyhow!("invalid size: '{}'", input))?;
    if !num.is_finite() {
        return Err(anyhow!("invalid size: '{}'", input));
    }
    if num < 0.0 {
        return Err(anyhow!("negative size: '{}'", input));
    }
    let bytes = num * mult as f64;
    if bytes >= (u64::MAX as f64) {
        return Err(anyhow!("size too large: '{}'", input));
    }
    Ok(bytes as u64)
}

/// Format a byte count as a human-readable string.
pub fn format_size(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{} {}", bytes, UNITS[0])
    } else {
        format!("{:.2} {}", value, UNITS[unit])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_plain_bytes() {
        assert_eq!(parse_size("100").unwrap(), 100);
        assert_eq!(parse_size("0").unwrap(), 0);
    }

    #[test]
    fn parses_units() {
        assert_eq!(parse_size("1K").unwrap(), 1024);
        assert_eq!(parse_size("1KB").unwrap(), 1024);
        assert_eq!(parse_size("100K").unwrap(), 102400);
        assert_eq!(parse_size("1M").unwrap(), 1024 * 1024);
        assert_eq!(parse_size("2MB").unwrap(), 2 * 1024 * 1024);
        assert_eq!(parse_size("1G").unwrap(), 1024 * 1024 * 1024);
        assert_eq!(parse_size("1GiB").unwrap(), 1024 * 1024 * 1024);
    }

    #[test]
    fn parses_decimals() {
        assert_eq!(parse_size("1.5K").unwrap(), 1536);
    }

    #[test]
    fn rejects_garbage() {
        assert!(parse_size("abc").is_err());
        assert!(parse_size("").is_err());
        assert!(parse_size("  ").is_err());
        assert!(parse_size("-1K").is_err());
        assert!(parse_size("1.5.2K").is_err());
        assert!(parse_size("inf").is_err());
        assert!(parse_size("NaN").is_err());
    }

    #[test]
    fn rejects_overflow() {
        // 10^20 bytes is > u64::MAX (~1.8 * 10^19)
        assert!(parse_size("100000000000T").is_err());
    }

    #[test]
    fn parses_zero() {
        assert_eq!(parse_size("0").unwrap(), 0);
        assert_eq!(parse_size("0K").unwrap(), 0);
        assert_eq!(parse_size("0MB").unwrap(), 0);
    }

    #[test]
    fn parses_whitespace_between_num_and_unit() {
        assert_eq!(parse_size("1 K").unwrap(), 1024);
        assert_eq!(parse_size("  2MB  ").unwrap(), 2 * 1024 * 1024);
    }

    #[test]
    fn parses_case_insensitive_units() {
        assert_eq!(parse_size("1k").unwrap(), 1024);
        assert_eq!(parse_size("1Kb").unwrap(), 1024);
        assert_eq!(parse_size("1kB").unwrap(), 1024);
    }

    #[test]
    fn formats_sizes() {
        assert_eq!(format_size(0), "0 B");
        assert_eq!(format_size(512), "512 B");
        assert_eq!(format_size(1024), "1.00 KB");
        assert_eq!(format_size(1536), "1.50 KB");
        assert_eq!(format_size(1024 * 1024), "1.00 MB");
        assert_eq!(format_size(1024u64 * 1024 * 1024 * 3), "3.00 GB");
    }

    #[test]
    fn formats_edge_sizes() {
        assert_eq!(format_size(1), "1 B");
        assert_eq!(format_size(1023), "1023 B");
        assert_eq!(format_size(1024u64 * 1024 * 1024 * 1024), "1.00 TB");
    }
}
