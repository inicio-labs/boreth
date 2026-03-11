//! Cross-crate tests using golden mainnet block fixtures.
//!
//! These tests verify boreth crate outputs against REAL Polygon mainnet values.
//! They are the ultimate "does our code produce the right answer?" check.

use bor_testdata::blocks::*;

// ============================================================================
// bor-chainspec: Parameter functions vs known ground truth
// ============================================================================

mod chainspec_tests {
    use super::*;
    use bor_chainspec::params::{
        sprint_size, span_size, block_gas_limit,
        base_fee_change_denominator, is_sprint_start, is_span_start,
    };

    #[test]
    fn test_sprint_size_at_all_known_blocks() {
        for known in known_parameter_blocks() {
            let actual = sprint_size(known.block);
            assert_eq!(
                actual, known.sprint_size,
                "sprint_size({}) should be {} (era: {}), got {}",
                known.block, known.sprint_size, known.era, actual
            );
        }
    }

    #[test]
    fn test_span_size_at_all_known_blocks() {
        for known in known_parameter_blocks() {
            let actual = span_size(known.block);
            assert_eq!(
                actual, known.span_size,
                "span_size({}) should be {} (era: {}), got {}",
                known.block, known.span_size, known.era, actual
            );
        }
    }

    #[test]
    fn test_gas_limit_at_all_known_blocks() {
        for known in known_parameter_blocks() {
            let actual = block_gas_limit(known.block);
            assert_eq!(
                actual, known.gas_limit_era,
                "block_gas_limit({}) should be {} (era: {}), got {}",
                known.block, known.gas_limit_era, known.era, actual
            );
        }
    }

    #[test]
    fn test_base_fee_denominator_at_all_known_blocks() {
        for known in known_parameter_blocks() {
            let actual = base_fee_change_denominator(known.block);
            assert_eq!(
                actual, known.base_fee_denom,
                "base_fee_change_denominator({}) should be {} (era: {}), got {}",
                known.block, known.base_fee_denom, known.era, actual
            );
        }
    }

    #[test]
    fn test_sprint_boundaries_at_known_blocks() {
        for known in known_boundary_blocks() {
            let actual = is_sprint_start(known.block);
            assert_eq!(
                actual, known.is_sprint_start,
                "is_sprint_start({}) should be {} (sprint_size={})",
                known.block, known.is_sprint_start, known.sprint_size
            );
        }
    }

    #[test]
    fn test_span_boundaries_at_known_blocks() {
        for known in known_boundary_blocks() {
            let actual = is_span_start(known.block, known.span_size);
            assert_eq!(
                actual, known.is_span_start,
                "is_span_start({}, {}) should be {}",
                known.block, known.span_size, known.is_span_start
            );
        }
    }

    /// CRITICAL TEST: Mid-sprint hardfork activation.
    /// Agra and Lisovo activate mid-sprint. Verify our code doesn't
    /// wrongly treat them as sprint boundaries.
    #[test]
    fn test_mid_sprint_hardforks_are_not_sprint_boundaries() {
        for fork in mid_sprint_hardforks() {
            let is_boundary = is_sprint_start(fork.block);
            assert!(
                !is_boundary,
                "{} hardfork at block {} activates mid-sprint (block % 16 = {}). \
                 is_sprint_start() MUST return false!",
                fork.name, fork.block, fork.block_mod_16
            );

            // Verify the containing sprint boundaries
            assert!(
                is_sprint_start(fork.sprint_containing_start),
                "Sprint containing {} should start at {}",
                fork.name, fork.sprint_containing_start
            );
            assert!(
                is_sprint_start(fork.next_sprint_start),
                "Next sprint after {} should start at {}",
                fork.name, fork.next_sprint_start
            );

            // Verify block_mod is correct
            assert_eq!(
                fork.block % 16, fork.block_mod_16,
                "{} block {} mod 16 should be {}",
                fork.name, fork.block, fork.block_mod_16
            );
        }
    }

    /// Verify every block in the sprint around Agra uses the right parameters.
    #[test]
    fn test_agra_mid_sprint_parameter_continuity() {
        let fork = &mid_sprint_hardforks()[0]; // Agra
        assert_eq!(fork.name, "Agra");

        for block in fork.sprint_containing_start..=fork.sprint_containing_end {
            // Sprint size should be 16 for ALL blocks in this range
            // (Delhi already activated long ago)
            assert_eq!(
                sprint_size(block), 16,
                "sprint_size({block}) during Agra sprint should be 16"
            );

            // Span size should be 6400 for all (Rio hasn't activated yet)
            assert_eq!(
                span_size(block), 6400,
                "span_size({block}) during Agra sprint should be 6400"
            );
        }
    }

    /// Verify every block in the sprint around Lisovo uses the right parameters.
    #[test]
    fn test_lisovo_mid_sprint_parameter_continuity() {
        let fork = &mid_sprint_hardforks()[1]; // Lisovo
        assert_eq!(fork.name, "Lisovo");

        for block in fork.sprint_containing_start..=fork.sprint_containing_end {
            assert_eq!(
                sprint_size(block), 16,
                "sprint_size({block}) during Lisovo sprint should be 16"
            );
            assert_eq!(
                span_size(block), 1600,
                "span_size({block}) during Lisovo sprint should be 1600"
            );
        }
    }
}

// ============================================================================
// bor-consensus: Difficulty and validation against known values
// ============================================================================

mod consensus_tests {
    use bor_chainspec::BorHardfork;

    /// Verify all hardfork activation blocks are in strictly ascending order.
    #[test]
    fn test_hardfork_ordering_matches_known_constants() {
        use bor_testdata::forks;
        let expected_order = [
            ("Delhi", forks::DELHI),
            ("Indore", forks::INDORE),
            ("Agra", forks::AGRA),
            ("Napoli", forks::NAPOLI),
            ("Ahmedabad", forks::AHMEDABAD),
            ("Bhilai", forks::BHILAI),
            ("Rio", forks::RIO),
            ("Madhugiri", forks::MADHUGIRI),
            ("Dandeli", forks::DANDELI),
            ("Lisovo", forks::LISOVO),
        ];

        // Verify boreth's hardfork blocks match our known constants
        let boreth_forks = BorHardfork::all();
        for (i, fork) in boreth_forks.iter().enumerate() {
            assert_eq!(
                fork.mainnet_block(), expected_order[i].1,
                "{} mainnet block should be {} but boreth says {}",
                expected_order[i].0, expected_order[i].1, fork.mainnet_block()
            );
        }

        // Verify strictly ascending
        for window in expected_order.windows(2) {
            assert!(
                window[0].1 < window[1].1,
                "{} ({}) must activate before {} ({})",
                window[0].0, window[0].1, window[1].0, window[1].1
            );
        }
    }

    /// Verify difficulty calculation properties:
    /// - Single validator: difficulty always 1
    /// - Difficulty >= 1 always
    /// - INTURN difficulty = validator_count
    #[test]
    fn test_difficulty_properties_with_known_blocks() {
        use bor_consensus::difficulty::{calculate_difficulty, diff_inturn, diff_noturn};
        use alloy_primitives::{Address, U256};

        // Single validator: always INTURN, difficulty = 1
        let single = vec![Address::new([0xaa; 20])];
        for block_num in [0u64, 1, 16, 6400, 38_189_056, 80_084_800] {
            let diff = calculate_difficulty(&single[0], &single, block_num);
            assert_eq!(diff, U256::from(1), "single validator at block {block_num} should have diff=1");
        }

        // With N validators, INTURN difficulty = N
        for n in [2, 5, 10, 50, 100] {
            assert_eq!(diff_inturn(n), U256::from(n));
        }

        // NOTURN difficulty is always >= 1
        for n in [1, 2, 5, 10, 100] {
            for d in 0..=n + 5 {
                let diff = diff_noturn(n, d);
                assert!(diff >= U256::from(1), "diff_noturn({n}, {d}) must be >= 1");
            }
        }
    }
}

// ============================================================================
// bor-storage: Receipt storage path selection
// ============================================================================

mod storage_tests {
    use super::*;
    use bor_storage::receipt::{is_post_madhugiri, store_block_receipts};
    use alloy_primitives::B256;

    /// Verify Madhugiri transition using known block numbers.
    #[test]
    fn test_madhugiri_transition_at_known_blocks() {
        for known in known_parameter_blocks() {
            let actual = is_post_madhugiri(known.block);
            assert_eq!(
                actual, known.is_post_madhugiri,
                "is_post_madhugiri({}) should be {} (era: {})",
                known.block, known.is_post_madhugiri, known.era
            );
        }
    }

    /// Verify that pre-Madhugiri uses separate storage, post uses unified.
    #[test]
    fn test_receipt_storage_path_at_boundary() {
        let hash = B256::from([0xab; 32]);

        // One block before Madhugiri
        let pre = store_block_receipts(80_084_799, &hash);
        assert!(pre.separate, "block 80_084_799 should use separate storage");

        // Madhugiri activation block
        let post = store_block_receipts(80_084_800, &hash);
        assert!(!post.separate, "block 80_084_800 should use unified storage");

        // Keys must be different (different encoding schemes)
        assert_ne!(pre.key, post.key, "pre and post Madhugiri keys must differ");
    }
}

// ============================================================================
// bor-evm: System transaction planning
// ============================================================================

mod evm_tests {
    use super::*;
    use bor_evm::plan_system_txs;
    use alloy_primitives::{Bytes, U256};

    /// Verify system tx planning at known boundary blocks.
    #[test]
    fn test_system_tx_planning_at_known_boundaries() {
        for known in known_boundary_blocks() {
            let events = if known.is_sprint_start {
                vec![(U256::from(1), Bytes::from_static(b"test_event"))]
            } else {
                vec![]
            };

            let plan = plan_system_txs(
                known.block,
                known.sprint_size,
                known.span_size,
                known.is_span_start,
                &events,
            );

            if known.is_span_start && known.block > 0 {
                assert!(
                    plan.execute_commit_span,
                    "block {} is span start — should execute commitSpan",
                    known.block
                );
            }

            if known.is_sprint_start && known.block > 0 {
                assert_eq!(
                    plan.state_sync_events.len(), events.len(),
                    "block {} is sprint start — should have state sync events",
                    known.block
                );
            }

            if !known.is_sprint_start || known.block == 0 {
                assert!(
                    plan.state_sync_events.is_empty(),
                    "block {} is NOT a sprint start — should have no state sync events",
                    known.block
                );
            }
        }
    }

    /// Block 0 should NEVER have system transactions.
    #[test]
    fn test_block_zero_no_system_txs() {
        let plan = plan_system_txs(
            0, 16, 6400, true,
            &[(U256::from(1), Bytes::from_static(b"should_not_execute"))],
        );
        assert!(!plan.execute_commit_span, "block 0 should not commitSpan");
        assert!(plan.state_sync_events.is_empty(), "block 0 should not have state sync");
    }

    /// Mid-sprint hardfork blocks should NOT trigger sprint-boundary system txs.
    #[test]
    fn test_mid_sprint_hardforks_no_system_txs() {
        for fork in mid_sprint_hardforks() {
            let plan = plan_system_txs(
                fork.block, 16, 1600, false,
                &[(U256::from(1), Bytes::from_static(b"should_not_execute"))],
            );
            assert!(
                plan.state_sync_events.is_empty(),
                "{} at block {} is mid-sprint (mod 16 = {}) — \
                 should NOT trigger state sync!",
                fork.name, fork.block, fork.block_mod_16
            );
        }
    }
}

// ============================================================================
// bor-primitives: Span ID calculation
// ============================================================================

mod primitives_tests {
    use bor_primitives::span_id_at;

    /// Verify span ID calculation at known blocks.
    #[test]
    fn test_span_id_at_known_blocks() {
        // Pre-Rio (span_size = 6400)
        assert_eq!(span_id_at(0, 6400), 0);
        assert_eq!(span_id_at(6399, 6400), 0);
        assert_eq!(span_id_at(6400, 6400), 1);
        assert_eq!(span_id_at(51200, 6400), 8);
        assert_eq!(span_id_at(64000, 6400), 10);

        // Post-Rio (span_size = 1600)
        assert_eq!(span_id_at(77_414_656, 1600), 48384);
        assert_eq!(span_id_at(77_416_256, 1600), 48385);

        // Verify span IDs don't collide across Rio transition
        let pre_rio_span = span_id_at(77_414_655, 6400);
        let post_rio_span = span_id_at(77_414_656, 1600);
        // These WILL be different numbers since different divisors
        assert_ne!(pre_rio_span, post_rio_span,
            "span IDs across Rio should differ due to span_size change");
    }
}

// ============================================================================
// Cross-cutting: Verify boreth parameter functions are consistent with each other
// ============================================================================

mod consistency_tests {
    use super::*;
    use bor_chainspec::params::*;

    /// For every block in known_parameter_blocks, verify ALL parameter functions
    /// return mutually consistent values.
    #[test]
    fn test_all_parameters_consistent_at_every_known_block() {
        for known in known_parameter_blocks() {
            let ss = sprint_size(known.block);
            let sp = span_size(known.block);
            let gl = block_gas_limit(known.block);
            let bfd = base_fee_change_denominator(known.block);

            // Sprint size must divide span size
            assert_eq!(
                sp % ss, 0,
                "block {}: span_size {} must be divisible by sprint_size {} (era: {})",
                known.block, sp, ss, known.era
            );

            // Gas limit must be one of the known values
            assert!(
                gl == 30_000_000 || gl == 45_000_000,
                "block {}: gas_limit {} must be 30M or 45M (era: {})",
                known.block, gl, known.era
            );

            // Base fee denominator must be one of 8, 16, or 64
            assert!(
                bfd == 8 || bfd == 16 || bfd == 64,
                "block {}: base_fee_denom {} must be 8, 16, or 64 (era: {})",
                known.block, bfd, known.era
            );
        }
    }

    /// Verify parameter transitions are monotonic (never go backwards).
    #[test]
    fn test_parameter_monotonicity() {
        let blocks = known_parameter_blocks();
        for window in blocks.windows(2) {
            let prev = &window[0];
            let next = &window[1];

            if prev.block < next.block {
                // Sprint size can only decrease (64 → 16)
                assert!(
                    prev.sprint_size >= next.sprint_size,
                    "sprint_size should not increase: {} at block {} vs {} at block {}",
                    prev.sprint_size, prev.block, next.sprint_size, next.block
                );

                // Span size can only decrease (6400 → 1600)
                assert!(
                    prev.span_size >= next.span_size,
                    "span_size should not increase: {} at block {} vs {} at block {}",
                    prev.span_size, prev.block, next.span_size, next.block
                );

                // Gas limit can only increase (30M → 45M)
                assert!(
                    prev.gas_limit_era <= next.gas_limit_era,
                    "gas_limit should not decrease: {} at block {} vs {} at block {}",
                    prev.gas_limit_era, prev.block, next.gas_limit_era, next.block
                );
            }
        }
    }
}
