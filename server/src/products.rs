// SPDX-License-Identifier: AGPL-3.0-or-later
//! Packaged products by barcode (PLAN.md §8 "Packaged products").

/// 8–14 digits whose last digit is the GTIN check digit.
pub fn gtin_ok(code: &str) -> bool {
    if !(8..=14).contains(&code.len()) || !code.bytes().all(|b| b.is_ascii_digit()) {
        return false;
    }
    // Weights 1, 3, 1, 3, … from the right, check digit included; the sum is a multiple of 10.
    let sum: u32 = code
        .bytes()
        .rev()
        .enumerate()
        .map(|(i, b)| u32::from(b - b'0') * if i % 2 == 0 { 1 } else { 3 })
        .sum();
    sum % 10 == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gtin_check_digit() {
        assert!(gtin_ok("4006381333931")); // EAN-13
        assert!(gtin_ok("036000291452")); // UPC-A
        assert!(gtin_ok("96385074")); // EAN-8
        assert!(gtin_ok("00012345600012")); // GTIN-14
        assert!(!gtin_ok("4006381333932")); // bad check digit
        assert!(!gtin_ok("1234565")); // too short
        assert!(!gtin_ok("123456789012345")); // too long
        assert!(!gtin_ok("40063813339a1"));
        assert!(!gtin_ok("+4006381333931"));
        assert!(!gtin_ok(""));
    }
}
