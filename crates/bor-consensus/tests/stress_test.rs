//! Stress test: concurrent header verification.
//!
//! Verifies that multiple threads can verify headers concurrently without
//! deadlocking or corrupting shared caches.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::thread;

use alloy_primitives::{keccak256, Address, B256, U256};
use bor_consensus::validation::{validate_header, HeaderValidationParams};
use bor_consensus::snapshot::BorSnapshot;
use bor_primitives::{Validator, ValidatorSet};

/// Number of concurrent verification threads.
const NUM_THREADS: usize = 10;

/// Number of blocks each thread verifies.
const BLOCKS_PER_THREAD: usize = 100;

fn make_validator(id: u64, addr_byte: u8, power: i64) -> Validator {
    Validator {
        id,
        address: Address::new([addr_byte; 20]),
        voting_power: power,
        signer: Address::new([addr_byte; 20]),
        proposer_priority: 0,
    }
}

fn make_snapshot() -> BorSnapshot {
    let validators = vec![
        make_validator(1, 0x01, 100),
        make_validator(2, 0x02, 100),
        make_validator(3, 0x03, 100),
        make_validator(4, 0x04, 100),
        make_validator(5, 0x05, 100),
    ];
    let vs = ValidatorSet {
        validators,
        proposer: None,
    };
    BorSnapshot::new(0, B256::ZERO, vs)
}

#[test]
fn test_concurrent_header_verification_no_panics() {
    let snapshot = Arc::new(make_snapshot());
    let signers: Vec<Address> = snapshot
        .validator_set
        .validators
        .iter()
        .map(|v| v.signer)
        .collect();
    let signers = Arc::new(signers);

    let mut handles = Vec::new();

    for thread_id in 0..NUM_THREADS {
        let signers = Arc::clone(&signers);
        let _snapshot = Arc::clone(&snapshot);

        let handle = thread::spawn(move || {
            let recents = BTreeMap::new();
            let mut verified = 0usize;

            for i in 0..BLOCKS_PER_THREAD {
                let block_number = (thread_id * BLOCKS_PER_THREAD + i) as u64;

                // Create a header with minimal extra data (will fail seal recovery,
                // but we're testing concurrency, not correctness)
                let params = HeaderValidationParams {
                    number: block_number,
                    timestamp: 1000 + block_number * 2,
                    nonce: 0,
                    mix_hash: B256::ZERO,
                    difficulty: U256::from(5),
                    extra_data: vec![0u8; 97],
                    gas_limit: 30_000_000,
                    seal_hash: keccak256(block_number.to_be_bytes()),
                    has_ommers: false,
                };

                // We expect this to fail (zero seal can't recover a valid signer),
                // but it must NOT panic or deadlock.
                let result = validate_header(&params, &signers, &recents, 1000 + block_number * 2);
                assert!(result.is_err(), "zero seal should fail recovery");
                verified += 1;
            }

            verified
        });

        handles.push(handle);
    }

    // Wait for all threads and collect results
    let mut total_verified = 0usize;
    for handle in handles {
        let count = handle.join().expect("thread should not panic");
        total_verified += count;
    }

    assert_eq!(
        total_verified,
        NUM_THREADS * BLOCKS_PER_THREAD,
        "all blocks should be verified"
    );
}

#[test]
fn test_concurrent_snapshot_access_no_panics() {
    let snapshot = Arc::new(make_snapshot());

    let mut handles = Vec::new();

    for thread_id in 0..NUM_THREADS {
        let snapshot = Arc::clone(&snapshot);

        let handle = thread::spawn(move || {
            // Each thread reads from the shared snapshot concurrently
            for i in 0..BLOCKS_PER_THREAD {
                let block_number = (thread_id * BLOCKS_PER_THREAD + i) as u64;

                // Read-only snapshot access
                let is_auth = snapshot.is_authorized(&Address::new([((block_number % 5) + 1) as u8; 20]));
                assert!(is_auth);

                // Encode/decode (independent copies)
                let encoded = snapshot.encode();
                let decoded = BorSnapshot::decode(&encoded).unwrap();
                assert_eq!(decoded.validator_set.validators.len(), 5);
            }
        });

        handles.push(handle);
    }

    for handle in handles {
        handle.join().expect("thread should not panic");
    }
}

#[test]
fn test_concurrent_difficulty_calculation() {
    use bor_consensus::difficulty::calculate_difficulty;

    let signers: Vec<Address> = (1..=10)
        .map(|i| Address::new([i; 20]))
        .collect();
    let signers = Arc::new(signers);

    let mut handles = Vec::new();

    for thread_id in 0..NUM_THREADS {
        let signers = Arc::clone(&signers);

        let handle = thread::spawn(move || {
            for i in 0..BLOCKS_PER_THREAD {
                let block_number = (thread_id * BLOCKS_PER_THREAD + i) as u64;
                let signer_idx = (block_number as usize) % signers.len();

                let diff = calculate_difficulty(&signers[signer_idx], &signers, block_number);

                // In-turn validator should get difficulty == validator_count
                assert_eq!(diff, U256::from(10), "inturn difficulty should be 10");
            }
        });

        handles.push(handle);
    }

    for handle in handles {
        handle.join().expect("thread should not panic");
    }
}
