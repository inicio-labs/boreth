use alloy_primitives::Address;
use bor_primitives::*;

/// Helper to create a validator with a distinct signer byte.
fn make_validator(id: u64, signer_byte: u8) -> Validator {
    Validator {
        id,
        address: Address::new([signer_byte; 20]),
        voting_power: 100,
        signer: Address::new([signer_byte; 20]),
        proposer_priority: 0,
    }
}

// ---------------------------------------------------------------------------
// 1. encode/decode roundtrip for 1 validator
// ---------------------------------------------------------------------------
#[test]
fn encode_decode_roundtrip_single_validator() {
    let validators = vec![make_validator(1, 0xaa)];
    let bytes = encode_validator_bytes(&validators);
    let addresses = decode_validator_bytes(&bytes);

    assert_eq!(addresses.len(), 1);
    assert_eq!(addresses[0], validators[0].signer);
}

// ---------------------------------------------------------------------------
// 2. encode/decode roundtrip for 100 validators
// ---------------------------------------------------------------------------
#[test]
fn encode_decode_roundtrip_100_validators() {
    let validators: Vec<Validator> = (0..100)
        .map(|i| make_validator(i as u64, (i % 256) as u8))
        .collect();

    let bytes = encode_validator_bytes(&validators);
    let addresses = decode_validator_bytes(&bytes);

    assert_eq!(addresses.len(), 100);
    for (i, addr) in addresses.iter().enumerate() {
        assert_eq!(*addr, validators[i].signer, "mismatch at index {i}");
    }
}

// ---------------------------------------------------------------------------
// 3. encode empty validators produces empty bytes
// ---------------------------------------------------------------------------
#[test]
fn encode_empty_validators_returns_empty_bytes() {
    let bytes = encode_validator_bytes(&[]);
    assert!(bytes.is_empty());
}

// ---------------------------------------------------------------------------
// 4. decode empty bytes produces empty addresses
// ---------------------------------------------------------------------------
#[test]
fn decode_empty_bytes_returns_empty_addresses() {
    let addresses = decode_validator_bytes(&[]);
    assert!(addresses.is_empty());
}

// ---------------------------------------------------------------------------
// 5. encode produces exactly N * 20 bytes
// ---------------------------------------------------------------------------
#[test]
fn encode_produces_n_times_20_bytes() {
    for n in [1, 2, 5, 10, 50] {
        let validators: Vec<Validator> = (0..n).map(|i| make_validator(i, (i % 256) as u8)).collect();
        let bytes = encode_validator_bytes(&validators);
        assert_eq!(
            bytes.len(),
            n as usize * 20,
            "expected {} bytes for {n} validators",
            n as usize * 20
        );
    }
}

// ---------------------------------------------------------------------------
// 6. decode with non-multiple-of-20 bytes (remainder ignored by chunks_exact)
// ---------------------------------------------------------------------------
#[test]
fn decode_non_multiple_of_20_ignores_remainder() {
    // 25 bytes = 1 full address (20) + 5 leftover bytes
    let mut bytes = vec![0xaa; 20];
    bytes.extend_from_slice(&[0xbb; 5]);

    let addresses = decode_validator_bytes(&bytes);
    assert_eq!(addresses.len(), 1);
    assert_eq!(addresses[0], Address::new([0xaa; 20]));

    // 19 bytes = 0 full addresses
    let short_bytes = vec![0xcc; 19];
    let addresses = decode_validator_bytes(&short_bytes);
    assert!(addresses.is_empty());

    // 41 bytes = 2 full addresses + 1 leftover
    let mut bytes41 = vec![0xdd; 20];
    bytes41.extend_from_slice(&[0xee; 20]);
    bytes41.push(0xff);
    let addresses = decode_validator_bytes(&bytes41);
    assert_eq!(addresses.len(), 2);
}

// ---------------------------------------------------------------------------
// 7. span_id_at for specific block numbers with span_size=6400
// ---------------------------------------------------------------------------
#[test]
fn span_id_at_with_default_span_size() {
    let span_size = 6400u64;

    assert_eq!(span_id_at(0, span_size), 0);
    assert_eq!(span_id_at(1, span_size), 0);
    assert_eq!(span_id_at(6399, span_size), 0);
    assert_eq!(span_id_at(6400, span_size), 1);
    assert_eq!(span_id_at(12799, span_size), 1);
    assert_eq!(span_id_at(12800, span_size), 2);
}

// ---------------------------------------------------------------------------
// 8. span_id_at with post-Rio span_size=1600
// ---------------------------------------------------------------------------
#[test]
fn span_id_at_with_post_rio_span_size() {
    let span_size = 1600u64;

    assert_eq!(span_id_at(0, span_size), 0);
    assert_eq!(span_id_at(1599, span_size), 0);
    assert_eq!(span_id_at(1600, span_size), 1);
    assert_eq!(span_id_at(3200, span_size), 2);
    assert_eq!(span_id_at(16000, span_size), 10);
}

// ---------------------------------------------------------------------------
// 9. Span JSON roundtrip with all fields populated
// ---------------------------------------------------------------------------
#[test]
fn span_json_roundtrip_all_fields() {
    let proposer = make_validator(1, 0xaa);
    let span = Span {
        id: 42,
        start_block: 268800,
        end_block: 275199,
        validator_set: ValidatorSet {
            validators: vec![
                make_validator(1, 0xaa),
                make_validator(2, 0xbb),
                make_validator(3, 0xcc),
            ],
            proposer: Some(proposer),
        },
        selected_producers: vec![make_validator(1, 0xaa), make_validator(2, 0xbb)],
        bor_chain_id: "137".to_string(),
    };

    let json = serde_json::to_string(&span).unwrap();
    let deserialized: Span = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.id, 42);
    assert_eq!(deserialized.start_block, 268800);
    assert_eq!(deserialized.end_block, 275199);
    assert_eq!(deserialized.bor_chain_id, "137");
    assert_eq!(deserialized.validator_set.validators.len(), 3);
    assert!(deserialized.validator_set.proposer.is_some());
    assert_eq!(deserialized.selected_producers.len(), 2);
}

// ---------------------------------------------------------------------------
// 10. ValidatorSet with None proposer roundtrip
// ---------------------------------------------------------------------------
#[test]
fn validator_set_none_proposer_roundtrip() {
    let vs = ValidatorSet {
        validators: vec![make_validator(1, 0x11), make_validator(2, 0x22)],
        proposer: None,
    };

    let json = serde_json::to_string(&vs).unwrap();
    let deserialized: ValidatorSet = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.validators.len(), 2);
    assert!(deserialized.proposer.is_none());
}

// ---------------------------------------------------------------------------
// 11. Validator with negative voting_power and proposer_priority
// ---------------------------------------------------------------------------
#[test]
fn validator_negative_voting_power_and_priority() {
    let v = Validator {
        id: 99,
        address: Address::new([0xde; 20]),
        voting_power: -500,
        signer: Address::new([0xde; 20]),
        proposer_priority: -1000,
    };

    let json = serde_json::to_string(&v).unwrap();
    let deserialized: Validator = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.id, 99);
    assert_eq!(deserialized.voting_power, -500);
    assert_eq!(deserialized.proposer_priority, -1000);
    assert_eq!(deserialized.signer, Address::new([0xde; 20]));
}

// ---------------------------------------------------------------------------
// 12. Multiple validators with distinct signers encode in correct order
// ---------------------------------------------------------------------------
#[test]
fn encode_preserves_validator_order() {
    let validators = vec![
        make_validator(1, 0x11),
        make_validator(2, 0x22),
        make_validator(3, 0x33),
        make_validator(4, 0x44),
    ];

    let bytes = encode_validator_bytes(&validators);
    let addresses = decode_validator_bytes(&bytes);

    assert_eq!(addresses.len(), 4);
    assert_eq!(addresses[0], Address::new([0x11; 20]));
    assert_eq!(addresses[1], Address::new([0x22; 20]));
    assert_eq!(addresses[2], Address::new([0x33; 20]));
    assert_eq!(addresses[3], Address::new([0x44; 20]));

    // Verify raw byte layout: first 20 bytes are 0x11, next 20 are 0x22, etc.
    assert!(bytes[0..20].iter().all(|&b| b == 0x11));
    assert!(bytes[20..40].iter().all(|&b| b == 0x22));
    assert!(bytes[40..60].iter().all(|&b| b == 0x33));
    assert!(bytes[60..80].iter().all(|&b| b == 0x44));
}
