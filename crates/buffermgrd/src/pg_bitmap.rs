//! PG bitmap and range combination generation

use std::collections::HashSet;

/// Generate PG range strings from bitmap
///
/// Generates all possible PG range combinations from a bitmap where each bit
/// represents a PG number (0-31).
///
/// Examples:
/// - bitmap 0b00001000 (bit 3) → ["3"]
/// - bitmap 0b00011000 (bits 3,4) → ["3", "4", "3-4"]
/// - bitmap 0b00101000 (bits 3,5) → ["3", "5", "3-5"]
///
/// This matches the C++ `generateIdListFromMap()` behavior.
pub fn generate_pg_combinations(bitmap: u32) -> HashSet<String> {
    let mut combinations = HashSet::new();
    let mut pgs = Vec::new();

    // Extract individual PG numbers from bitmap
    for i in 0..32 {
        if (bitmap & (1 << i)) != 0 {
            pgs.push(i);
        }
    }

    if pgs.is_empty() {
        return combinations;
    }

    // Generate all range combinations
    for start_idx in 0..pgs.len() {
        for end_idx in start_idx..pgs.len() {
            let start = pgs[start_idx];
            let end = pgs[end_idx];

            if start == end {
                // Single PG
                combinations.insert(start.to_string());
            } else {
                // Range
                combinations.insert(format!("{}-{}", start, end));
            }
        }
    }

    combinations
}

/// Convert comma-separated PG list to bitmap
///
/// Parses a PFC enable string like "3,4" into a bitmap where bits 3 and 4 are set.
///
/// Example: "3,4" → 0b00011000 (bits 3 and 4 set)
pub fn pfc_to_bitmap(pfc_enable: &str) -> u32 {
    let mut bitmap = 0u32;

    for pg_str in pfc_enable.split(',') {
        if let Ok(pg) = pg_str.trim().parse::<u8>() {
            if pg < 32 {
                bitmap |= 1 << pg;
            }
        }
    }

    bitmap
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pfc_to_bitmap_simple() {
        assert_eq!(pfc_to_bitmap("3,4"), 0b00011000);
        assert_eq!(pfc_to_bitmap("0"), 0b00000001);
        assert_eq!(pfc_to_bitmap("7"), 0b10000000);
    }

    #[test]
    fn test_pfc_to_bitmap_with_spaces() {
        assert_eq!(pfc_to_bitmap("3, 4"), 0b00011000);
        assert_eq!(pfc_to_bitmap(" 3 , 4 "), 0b00011000);
    }

    #[test]
    fn test_pfc_to_bitmap_invalid() {
        // Invalid entries are ignored
        assert_eq!(pfc_to_bitmap("3,invalid,4"), 0b00011000);
        assert_eq!(pfc_to_bitmap(""), 0);
        assert_eq!(pfc_to_bitmap("abc"), 0);
    }

    #[test]
    fn test_pfc_to_bitmap_out_of_range() {
        // PG >= 32 are ignored
        assert_eq!(pfc_to_bitmap("3,32,33"), 0b00001000);
    }

    #[test]
    fn test_generate_pg_combinations_single() {
        let combos = generate_pg_combinations(0b00001000); // bit 3
        assert!(combos.contains("3"));
        assert_eq!(combos.len(), 1);
    }

    #[test]
    fn test_generate_pg_combinations_two_adjacent() {
        let combos = generate_pg_combinations(0b00011000); // bits 3,4
        assert!(combos.contains("3"));
        assert!(combos.contains("4"));
        assert!(combos.contains("3-4"));
        assert_eq!(combos.len(), 3);
    }

    #[test]
    fn test_generate_pg_combinations_two_non_adjacent() {
        let combos = generate_pg_combinations(0b00101000); // bits 3,5
        assert!(combos.contains("3"));
        assert!(combos.contains("5"));
        assert!(combos.contains("3-5"));
        assert_eq!(combos.len(), 3);
    }

    #[test]
    fn test_generate_pg_combinations_three() {
        let combos = generate_pg_combinations(0b00111000); // bits 3,4,5
                                                           // Should have: 3, 4, 5, 3-4, 3-5, 4-5, 3-5 (but 3-5 is duplicate)
                                                           // Actually: 3, 4, 5, 3-4, 4-5, 3-5
        assert!(combos.contains("3"));
        assert!(combos.contains("4"));
        assert!(combos.contains("5"));
        assert!(combos.contains("3-4"));
        assert!(combos.contains("4-5"));
        assert!(combos.contains("3-5"));
        assert_eq!(combos.len(), 6);
    }

    #[test]
    fn test_generate_pg_combinations_empty() {
        let combos = generate_pg_combinations(0);
        assert!(combos.is_empty());
    }

    #[test]
    fn test_pfc_to_bitmap_and_generate() {
        // Full round-trip test
        let bitmap = pfc_to_bitmap("3,4");
        let combos = generate_pg_combinations(bitmap);

        assert!(combos.contains("3"));
        assert!(combos.contains("4"));
        assert!(combos.contains("3-4"));
    }
}
