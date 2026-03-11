//! Comprehensive tests for `bor-chainspec` hardfork logic, parameter transitions,
//! genesis configurations, and string parsing.

use std::str::FromStr;

use bor_chainspec::{
    bor_amoy_genesis, bor_mainnet_genesis,
    params::{
        base_fee_change_denominator, block_gas_limit, is_span_start, is_sprint_start,
        max_code_size, span_size, sprint_size,
    },
    BorHardfork,
};
use reth_chainspec::EthChainSpec;
use reth_ethereum_forks::Hardfork;

// ---------------------------------------------------------------------------
// 3.1 Mid-sprint hardfork activation: Agra (50_523_000 % 16 == 8)
// ---------------------------------------------------------------------------

#[test]
fn agra_activates_mid_sprint() {
    // Agra block is NOT aligned to a 16-block sprint boundary.
    assert_eq!(50_523_000_u64 % 16, 8, "Agra should land mid-sprint");
}

#[test]
fn agra_active_at_its_block() {
    let spec = bor_mainnet_genesis();
    assert!(spec.is_bor_fork_active_at_block(BorHardfork::Agra, 50_523_000));
}

#[test]
fn agra_not_active_one_block_before() {
    let spec = bor_mainnet_genesis();
    assert!(!spec.is_bor_fork_active_at_block(BorHardfork::Agra, 50_522_999));
}

#[test]
fn sprint_size_unchanged_around_agra() {
    // Sprint size changed at Delhi, not Agra -- should stay 16 around Agra.
    assert_eq!(sprint_size(50_522_999), 16);
    assert_eq!(sprint_size(50_523_000), 16);
    assert_eq!(sprint_size(50_523_001), 16);
}

// ---------------------------------------------------------------------------
// 3.2 Mid-sprint hardfork activation: Lisovo (83_756_500 % 16 == 4)
// ---------------------------------------------------------------------------

#[test]
fn lisovo_activates_mid_sprint() {
    assert_eq!(83_756_500_u64 % 16, 4, "Lisovo should land mid-sprint");
}

#[test]
fn params_at_lisovo_block() {
    let block = 83_756_500;
    // Post-Delhi sprint size
    assert_eq!(sprint_size(block), 16);
    // Post-Rio span size
    assert_eq!(span_size(block), 1600);
    // Post-Bhilai gas limit
    assert_eq!(block_gas_limit(block), 45_000_000);
    // Post-Bhilai base fee denominator
    assert_eq!(base_fee_change_denominator(block), 64);
}

#[test]
fn lisovo_fork_activation() {
    let spec = bor_mainnet_genesis();
    assert!(spec.is_bor_fork_active_at_block(BorHardfork::Lisovo, 83_756_500));
    assert!(!spec.is_bor_fork_active_at_block(BorHardfork::Lisovo, 83_756_499));
}

// ---------------------------------------------------------------------------
// 3.3 Delhi sprint size transition
// ---------------------------------------------------------------------------

#[test]
fn sprint_size_pre_delhi() {
    assert_eq!(sprint_size(38_189_055), 64);
}

#[test]
fn sprint_size_at_delhi() {
    assert_eq!(sprint_size(38_189_056), 16);
}

#[test]
fn sprint_size_post_delhi() {
    assert_eq!(sprint_size(38_189_057), 16);
}

#[test]
fn is_sprint_start_pre_delhi() {
    // Pre-Delhi sprint size is 64
    assert!(!is_sprint_start(0)); // genesis is never a sprint start
    assert!(is_sprint_start(64));
    assert!(is_sprint_start(128));
    assert!(!is_sprint_start(1));
    assert!(!is_sprint_start(63));
    assert!(!is_sprint_start(65));
}

#[test]
fn is_sprint_start_at_delhi_boundary() {
    // Delhi block 38_189_056 is divisible by both 64 and 16
    assert_eq!(38_189_056_u64 % 64, 0);
    assert_eq!(38_189_056_u64 % 16, 0);
    assert!(is_sprint_start(38_189_056));
}

#[test]
fn is_sprint_start_post_delhi() {
    // Post-Delhi, sprint size is 16
    assert!(is_sprint_start(38_189_056 + 16));
    assert!(!is_sprint_start(38_189_056 + 1));
    assert!(!is_sprint_start(38_189_056 + 15));
}

// ---------------------------------------------------------------------------
// 3.4 Rio span size transition
// ---------------------------------------------------------------------------

#[test]
fn span_size_pre_rio() {
    assert_eq!(span_size(77_414_655), 6400);
}

#[test]
fn span_size_at_rio() {
    assert_eq!(span_size(77_414_656), 1600);
}

#[test]
fn span_size_post_rio() {
    assert_eq!(span_size(77_414_657), 1600);
}

#[test]
fn first_post_rio_span_boundary() {
    // The first span boundary at or after Rio with the new span size.
    let rio = 77_414_656_u64;
    let new_span = 1600_u64;
    // Find the first multiple of 1600 >= rio
    let first_boundary = rio.div_ceil(new_span) * new_span;
    assert_eq!(first_boundary % new_span, 0);
    assert!(first_boundary >= rio);
    assert!(is_span_start(first_boundary, new_span));
}

#[test]
fn span_id_calculation_pre_rio() {
    // With 6400-block spans, span_id = block / 6400
    let block = 77_414_655_u64;
    let span = span_size(block);
    assert_eq!(span, 6400);
    let span_id = block / span;
    assert_eq!(span_id, 12_096);
}

#[test]
fn is_span_start_various() {
    assert!(!is_span_start(0, 6400)); // genesis is never a span start
    assert!(is_span_start(6400, 6400));
    assert!(!is_span_start(6401, 6400));
    assert!(!is_span_start(0, 1600)); // genesis is never a span start
    assert!(is_span_start(1600, 1600));
    assert!(!is_span_start(1601, 1600));
}

// ---------------------------------------------------------------------------
// 3.6 Bhilai gas limit transition
// ---------------------------------------------------------------------------

#[test]
fn gas_limit_pre_bhilai() {
    assert_eq!(block_gas_limit(75_999_999), 30_000_000);
}

#[test]
fn gas_limit_at_bhilai() {
    assert_eq!(block_gas_limit(76_000_000), 45_000_000);
}

#[test]
fn gas_limit_post_bhilai() {
    assert_eq!(block_gas_limit(76_000_001), 45_000_000);
}

#[test]
fn gas_limit_at_genesis() {
    assert_eq!(block_gas_limit(0), 30_000_000);
}

// ---------------------------------------------------------------------------
// 3.7 Base fee change denominator transitions
// ---------------------------------------------------------------------------

#[test]
fn base_fee_denom_pre_delhi() {
    assert_eq!(base_fee_change_denominator(0), 8);
    assert_eq!(base_fee_change_denominator(1), 8);
    assert_eq!(base_fee_change_denominator(38_189_055), 8);
}

#[test]
fn base_fee_denom_at_delhi() {
    assert_eq!(base_fee_change_denominator(38_189_056), 16);
}

#[test]
fn base_fee_denom_between_delhi_and_bhilai() {
    assert_eq!(base_fee_change_denominator(38_189_057), 16);
    assert_eq!(base_fee_change_denominator(50_000_000), 16);
    assert_eq!(base_fee_change_denominator(75_999_999), 16);
}

#[test]
fn base_fee_denom_at_bhilai() {
    assert_eq!(base_fee_change_denominator(76_000_000), 64);
}

#[test]
fn base_fee_denom_post_bhilai() {
    assert_eq!(base_fee_change_denominator(76_000_001), 64);
    assert_eq!(base_fee_change_denominator(100_000_000), 64);
}

// ---------------------------------------------------------------------------
// 3.8 All 10 hardforks integration
// ---------------------------------------------------------------------------

#[test]
fn all_hardforks_returns_ten() {
    assert_eq!(BorHardfork::all().len(), 10);
}

#[test]
fn all_hardforks_ascending_mainnet_blocks() {
    let forks = BorHardfork::all();
    for window in forks.windows(2) {
        assert!(
            window[0].mainnet_block() < window[1].mainnet_block(),
            "{} (block {}) should activate before {} (block {})",
            window[0],
            window[0].mainnet_block(),
            window[1],
            window[1].mainnet_block(),
        );
    }
}

#[test]
fn no_two_hardforks_share_same_block() {
    let forks = BorHardfork::all();
    for i in 0..forks.len() {
        for j in (i + 1)..forks.len() {
            assert_ne!(
                forks[i].mainnet_block(),
                forks[j].mainnet_block(),
                "{} and {} must not share the same activation block",
                forks[i],
                forks[j],
            );
        }
    }
}

#[test]
fn all_params_correct_at_each_boundary() {
    let expected: &[(BorHardfork, u64, u64, u64, u64, u64)] = &[
        // (fork, block, sprint, span, gas_limit, base_fee_denom)
        (BorHardfork::Delhi, 38_189_056, 16, 6400, 30_000_000, 16),
        (BorHardfork::Indore, 44_934_656, 16, 6400, 30_000_000, 16),
        (BorHardfork::Agra, 50_523_000, 16, 6400, 30_000_000, 16),
        (BorHardfork::Napoli, 68_195_328, 16, 6400, 30_000_000, 16),
        (BorHardfork::Ahmedabad, 73_100_000, 16, 6400, 30_000_000, 16),
        (BorHardfork::Bhilai, 76_000_000, 16, 6400, 45_000_000, 64),
        (BorHardfork::Rio, 77_414_656, 16, 1600, 45_000_000, 64),
        (BorHardfork::Madhugiri, 80_084_800, 16, 1600, 45_000_000, 64),
        (BorHardfork::Dandeli, 81_900_000, 16, 1600, 45_000_000, 64),
        (BorHardfork::Lisovo, 83_756_500, 16, 1600, 45_000_000, 64),
    ];

    for &(fork, block, exp_sprint, exp_span, exp_gas, exp_denom) in expected {
        assert_eq!(
            fork.mainnet_block(),
            block,
            "wrong mainnet_block for {fork}"
        );
        assert_eq!(sprint_size(block), exp_sprint, "wrong sprint_size at {fork}");
        assert_eq!(span_size(block), exp_span, "wrong span_size at {fork}");
        assert_eq!(
            block_gas_limit(block),
            exp_gas,
            "wrong block_gas_limit at {fork}"
        );
        assert_eq!(
            base_fee_change_denominator(block),
            exp_denom,
            "wrong base_fee_change_denominator at {fork}"
        );
    }
}

// ---------------------------------------------------------------------------
// 3.9 Amoy testnet: all forks active at block 0
// ---------------------------------------------------------------------------

#[test]
fn amoy_all_forks_at_block_zero() {
    for fork in BorHardfork::all() {
        assert_eq!(fork.amoy_block(), 0, "{fork} amoy_block should be 0");
    }
}

#[test]
fn amoy_spec_all_forks_active_at_genesis() {
    let spec = bor_amoy_genesis();
    for fork in BorHardfork::all() {
        assert!(
            spec.is_bor_fork_active_at_block(*fork, 0),
            "{fork} should be active at Amoy genesis"
        );
    }
}

#[test]
fn amoy_latest_rules_from_genesis() {
    // On Amoy, block 0 should have the latest parameter values.
    // Since all forks are active at 0, the "latest" Delhi-era sprint size
    // applies. However, param functions use mainnet block numbers, so we
    // only verify via the chainspec fork activation here.
    let spec = bor_amoy_genesis();
    assert!(spec.is_bor_fork_active_at_block(BorHardfork::Lisovo, 0));
    assert!(spec.is_bor_fork_active_at_block(BorHardfork::Delhi, 0));
    assert!(spec.is_bor_fork_active_at_block(BorHardfork::Bhilai, 0));
    assert!(spec.is_bor_fork_active_at_block(BorHardfork::Rio, 0));
}

// ---------------------------------------------------------------------------
// 12.1 Mainnet chainspec completeness
// ---------------------------------------------------------------------------

#[test]
fn mainnet_chainspec_contains_all_10_hardforks() {
    let spec = bor_mainnet_genesis();
    let bor_forks = spec.bor_hardforks();
    assert_eq!(bor_forks.len(), 10);
    for fork in BorHardfork::all() {
        assert!(bor_forks.contains_key(fork), "missing hardfork: {fork}");
    }
}

#[test]
fn mainnet_chainspec_block_numbers_correct() {
    let expected: &[(BorHardfork, u64)] = &[
        (BorHardfork::Delhi, 38_189_056),
        (BorHardfork::Indore, 44_934_656),
        (BorHardfork::Agra, 50_523_000),
        (BorHardfork::Napoli, 68_195_328),
        (BorHardfork::Ahmedabad, 73_100_000),
        (BorHardfork::Bhilai, 76_000_000),
        (BorHardfork::Rio, 77_414_656),
        (BorHardfork::Madhugiri, 80_084_800),
        (BorHardfork::Dandeli, 81_900_000),
        (BorHardfork::Lisovo, 83_756_500),
    ];
    for &(fork, block) in expected {
        assert_eq!(fork.mainnet_block(), block, "wrong block for {fork}");
    }
}

#[test]
fn mainnet_chainspec_strictly_ascending_no_duplicates() {
    let forks = BorHardfork::all();
    let blocks: Vec<u64> = forks.iter().map(|f| f.mainnet_block()).collect();
    for window in blocks.windows(2) {
        assert!(
            window[0] < window[1],
            "blocks must be strictly ascending: {} vs {}",
            window[0],
            window[1]
        );
    }
}

// ---------------------------------------------------------------------------
// 12.3 Hardfork string parsing roundtrip
// ---------------------------------------------------------------------------

#[test]
fn hardfork_display_roundtrip() {
    for fork in BorHardfork::all() {
        let s = fork.to_string();
        let parsed = BorHardfork::from_str(&s).expect("should parse display output");
        assert_eq!(*fork, parsed);
    }
}

#[test]
fn hardfork_name_matches_display() {
    for fork in BorHardfork::all() {
        assert_eq!(fork.name(), fork.to_string());
    }
}

#[test]
fn hardfork_parse_case_insensitive() {
    assert_eq!(BorHardfork::from_str("delhi").unwrap(), BorHardfork::Delhi);
    assert_eq!(BorHardfork::from_str("DELHI").unwrap(), BorHardfork::Delhi);
    assert_eq!(BorHardfork::from_str("Delhi").unwrap(), BorHardfork::Delhi);
    assert_eq!(BorHardfork::from_str("dElHi").unwrap(), BorHardfork::Delhi);
    assert_eq!(
        BorHardfork::from_str("lisovo").unwrap(),
        BorHardfork::Lisovo
    );
    assert_eq!(
        BorHardfork::from_str("LISOVO").unwrap(),
        BorHardfork::Lisovo
    );
}

#[test]
fn hardfork_parse_unknown_returns_err() {
    assert!(BorHardfork::from_str("unknown").is_err());
}

#[test]
fn hardfork_parse_empty_returns_err() {
    assert!(BorHardfork::from_str("").is_err());
}

#[test]
fn hardfork_parse_all_variants_lowercase() {
    let names = [
        "delhi",
        "indore",
        "agra",
        "napoli",
        "ahmedabad",
        "bhilai",
        "rio",
        "madhugiri",
        "dandeli",
        "lisovo",
    ];
    let expected = BorHardfork::all();
    for (name, fork) in names.iter().zip(expected.iter()) {
        let parsed = BorHardfork::from_str(name)
            .unwrap_or_else(|e| panic!("failed to parse '{name}': {e}"));
        assert_eq!(parsed, *fork);
    }
}

// ---------------------------------------------------------------------------
// is_sprint_start at various blocks (pre/post Delhi)
// ---------------------------------------------------------------------------

#[test]
fn is_sprint_start_at_block_zero() {
    assert!(!is_sprint_start(0)); // genesis is never a sprint start
}

#[test]
fn is_sprint_start_pre_delhi_boundaries() {
    // Pre-Delhi: sprint size = 64
    for multiple in [64, 128, 192, 256, 38_189_056 - 64] {
        assert!(
            is_sprint_start(multiple),
            "block {multiple} should be sprint start (pre-Delhi, 64)"
        );
    }
    for non_multiple in [1, 32, 63, 65, 100] {
        assert!(
            !is_sprint_start(non_multiple),
            "block {non_multiple} should NOT be sprint start (pre-Delhi, 64)"
        );
    }
}

#[test]
fn is_sprint_start_post_delhi_boundaries() {
    let delhi = 38_189_056_u64;
    for offset in [0, 16, 32, 48, 64, 1600] {
        let block = delhi + offset;
        assert!(
            is_sprint_start(block),
            "block {block} should be sprint start (post-Delhi, 16)"
        );
    }
    for offset in [1, 8, 15, 17] {
        let block = delhi + offset;
        assert!(
            !is_sprint_start(block),
            "block {block} should NOT be sprint start (post-Delhi, 16)"
        );
    }
}

// ---------------------------------------------------------------------------
// is_span_start with various span sizes
// ---------------------------------------------------------------------------

#[test]
fn is_span_start_with_6400() {
    assert!(!is_span_start(0, 6400)); // genesis is never a span start
    assert!(is_span_start(6400, 6400));
    assert!(is_span_start(12800, 6400));
    assert!(!is_span_start(1, 6400));
    assert!(!is_span_start(6399, 6400));
}

#[test]
fn is_span_start_with_1600() {
    assert!(!is_span_start(0, 1600)); // genesis is never a span start
    assert!(is_span_start(1600, 1600));
    assert!(is_span_start(3200, 1600));
    assert!(!is_span_start(1, 1600));
    assert!(!is_span_start(1599, 1600));
    assert!(!is_span_start(1601, 1600));
}

// ---------------------------------------------------------------------------
// max_code_size always returns 24_576
// ---------------------------------------------------------------------------

#[test]
fn max_code_size_always_24576() {
    assert_eq!(max_code_size(0), 24_576);
    assert_eq!(max_code_size(1), 24_576);
    assert_eq!(max_code_size(38_189_056), 24_576);
    assert_eq!(max_code_size(76_000_000), 24_576);
    assert_eq!(max_code_size(83_756_500), 24_576);
    assert_eq!(max_code_size(100_000_000), 24_576);
    assert_eq!(max_code_size(u64::MAX), 24_576);
}

// ---------------------------------------------------------------------------
// Genesis chain IDs
// ---------------------------------------------------------------------------

#[test]
fn mainnet_genesis_chain_id_137() {
    let spec = bor_mainnet_genesis();
    assert_eq!(spec.chain_id(), 137);
}

#[test]
fn amoy_genesis_chain_id_80002() {
    let spec = bor_amoy_genesis();
    assert_eq!(spec.chain_id(), 80002);
}

// ---------------------------------------------------------------------------
// All forks active at their block, not active one block before
// ---------------------------------------------------------------------------

#[test]
fn all_forks_active_at_activation_block() {
    let spec = bor_mainnet_genesis();
    for fork in BorHardfork::all() {
        let block = fork.mainnet_block();
        assert!(
            spec.is_bor_fork_active_at_block(*fork, block),
            "{fork} should be active at its activation block {block}"
        );
    }
}

#[test]
fn all_forks_inactive_one_block_before_activation() {
    let spec = bor_mainnet_genesis();
    for fork in BorHardfork::all() {
        let block = fork.mainnet_block();
        if block > 0 {
            assert!(
                !spec.is_bor_fork_active_at_block(*fork, block - 1),
                "{fork} should NOT be active at block {} (one before activation)",
                block - 1
            );
        }
    }
}

#[test]
fn all_forks_active_well_after_activation() {
    let spec = bor_mainnet_genesis();
    for fork in BorHardfork::all() {
        let block = fork.mainnet_block() + 1_000_000;
        assert!(
            spec.is_bor_fork_active_at_block(*fork, block),
            "{fork} should be active at block {block}"
        );
    }
}
