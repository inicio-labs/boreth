#!/usr/bin/env python3
"""
Fetch real Polygon mainnet block data and write golden test fixtures.

Usage:
    python3 fetch_golden_blocks.py [--rpc URL]

Writes JSON fixture files to ../data/ directory.
Run this from your LOCAL machine (not sandbox) — needs RPC access.
"""

import json
import os
import sys
import time
import urllib.request

DEFAULT_RPC = "https://polygon-rpc.com"

# ============================================================================
# STRATEGIC BLOCK SELECTION
# ============================================================================
# Each block is picked for a SPECIFIC reason. Categories:
#   - HARDFORK: exact activation block
#   - BOUNDARY: sprint/span boundary
#   - MID_SPRINT: fork activates mid-sprint (the dangerous case)
#   - TRANSITION: block before/after a parameter change
#   - RANDOM: normal block for baseline testing
# ============================================================================

BLOCKS = [
    # ---------- BASELINE / NORMAL ----------
    {"name": "normal_early",           "block": 1_000_000,  "why": "Early mainnet block, pre-all-forks, sprint=64, span=6400"},
    {"name": "normal_mid",             "block": 30_000_000, "why": "Mid mainnet, pre-Delhi, sprint=64"},

    # ---------- DELHI (sprint 64→16) ----------
    {"name": "pre_delhi_sprint_end",   "block": 38_188_992, "why": "Last sprint-64 boundary before Delhi (38188992 % 64 == 0)"},
    {"name": "pre_delhi_last",         "block": 38_189_055, "why": "Last block before Delhi activation"},
    {"name": "delhi_activation",       "block": 38_189_056, "why": "HARDFORK: Delhi — sprint changes 64→16"},
    {"name": "delhi_plus_1",           "block": 38_189_057, "why": "First non-boundary post-Delhi block"},
    {"name": "post_delhi_first_sprint","block": 38_189_072, "why": "First sprint-16 boundary after Delhi (38189072 % 16 == 0)"},

    # ---------- INDORE ----------
    {"name": "indore_activation",      "block": 44_934_656, "why": "HARDFORK: Indore activation"},

    # ---------- AGRA (MID-SPRINT!) ----------
    {"name": "pre_agra_sprint_start",  "block": 50_522_992, "why": "Sprint start before Agra (50522992 % 16 == 0)"},
    {"name": "pre_agra_last",          "block": 50_522_999, "why": "Last block before Agra"},
    {"name": "agra_activation",        "block": 50_523_000, "why": "HARDFORK+MID_SPRINT: Agra — 50523000 % 16 = 8!"},
    {"name": "agra_next_sprint",       "block": 50_523_008, "why": "First sprint boundary after Agra"},

    # ---------- SPAN BOUNDARY (pre-Rio) ----------
    {"name": "span_8_start",           "block": 51_200,     "why": "BOUNDARY: span=8 start (51200/6400 = 8)"},
    {"name": "span_boundary_mid",      "block": 64_000,     "why": "BOUNDARY: span=10 start (64000/6400 = 10)"},

    # ---------- NAPOLI ----------
    {"name": "napoli_activation",      "block": 68_195_328, "why": "HARDFORK: Napoli activation"},

    # ---------- AHMEDABAD ----------
    {"name": "ahmedabad_activation",   "block": 73_100_000, "why": "HARDFORK: Ahmedabad activation"},

    # ---------- BHILAI (gas 30M→45M) ----------
    {"name": "pre_bhilai",             "block": 75_999_999, "why": "TRANSITION: last block with gasLimit 30M"},
    {"name": "bhilai_activation",      "block": 76_000_000, "why": "HARDFORK: Bhilai — gas limit 30M→45M"},
    {"name": "bhilai_plus_1",          "block": 76_000_001, "why": "First block with 45M gas limit"},

    # ---------- RIO (span 6400→1600, VEBloP) ----------
    {"name": "pre_rio",                "block": 77_414_655, "why": "TRANSITION: last block with span=6400"},
    {"name": "rio_activation",         "block": 77_414_656, "why": "HARDFORK: Rio — span 6400→1600, VEBloP"},
    {"name": "post_rio_first_span",    "block": 77_416_256, "why": "BOUNDARY: first span boundary post-Rio (77414656+1600)"},

    # ---------- MADHUGIRI (receipt storage unification) ----------
    {"name": "pre_madhugiri_last",     "block": 80_084_799, "why": "TRANSITION: last block with separate bor receipts"},
    {"name": "madhugiri_activation",   "block": 80_084_800, "why": "HARDFORK: Madhugiri — receipts unified, block time 1s"},
    {"name": "post_madhugiri_sprint",  "block": 80_084_816, "why": "BOUNDARY: first sprint boundary post-Madhugiri (80084816 % 16 == 0)"},

    # ---------- DANDELI ----------
    {"name": "dandeli_activation",     "block": 81_900_000, "why": "HARDFORK: Dandeli activation"},

    # ---------- LISOVO (MID-SPRINT!) ----------
    {"name": "pre_lisovo_sprint",      "block": 83_756_496, "why": "Sprint start before Lisovo (83756496 % 16 == 0)"},
    {"name": "pre_lisovo_last",        "block": 83_756_499, "why": "Last block before Lisovo"},
    {"name": "lisovo_activation",      "block": 83_756_500, "why": "HARDFORK+MID_SPRINT: Lisovo — 83756500 % 16 = 4!"},
    {"name": "lisovo_next_sprint",     "block": 83_756_512, "why": "First sprint boundary after Lisovo"},

    # ---------- RANDOM NORMAL BLOCKS (variety) ----------
    {"name": "random_post_delhi",      "block": 45_000_000, "why": "RANDOM: post-Delhi, pre-Agra baseline"},
    {"name": "random_post_napoli",     "block": 70_000_000, "why": "RANDOM: post-Napoli baseline"},
    {"name": "random_post_madhugiri",  "block": 82_000_000, "why": "RANDOM: post-Madhugiri, pre-Lisovo baseline"},
    {"name": "random_recent",          "block": 83_500_000, "why": "RANDOM: recent block, near-tip baseline"},
]


def rpc_call(rpc_url, method, params):
    payload = json.dumps({
        "jsonrpc": "2.0", "method": method, "params": params, "id": 1
    }).encode()
    req = urllib.request.Request(rpc_url, data=payload,
                                 headers={"Content-Type": "application/json"})
    with urllib.request.urlopen(req, timeout=30) as resp:
        data = json.loads(resp.read())
        if "error" in data:
            raise RuntimeError(f"RPC error: {data['error']}")
        return data["result"]


def fetch_block(rpc_url, block_number):
    """Fetch block with full transactions."""
    return rpc_call(rpc_url, "eth_getBlockByNumber", [hex(block_number), True])


def fetch_bor_author(rpc_url, block_number):
    """Fetch the block author/signer via bor_getAuthor."""
    try:
        return rpc_call(rpc_url, "bor_getAuthor", [hex(block_number)])
    except Exception:
        return None


def extract_fixture(block_entry, block_data, author):
    """Extract relevant fields into a fixture dict."""
    txs = block_data.get("transactions", [])

    # Classify transactions
    user_txs = []
    system_txs = []
    for tx in txs:
        tx_info = {
            "hash": tx.get("hash"),
            "from": tx.get("from"),
            "to": tx.get("to"),
            "type": tx.get("type"),
            "nonce": tx.get("nonce"),
            "value": tx.get("value"),
            "gas": tx.get("gas"),
            "gas_price": tx.get("gasPrice"),
            "input_first_8": tx.get("input", "0x")[:10],  # selector only
            "input_length": len(tx.get("input", "0x")) - 2,
        }
        # State sync txs have type 0x7e (126) post-Madhugiri, or are from system address
        if tx.get("type") == "0x7e" or tx.get("from", "").lower() == "0xfffffffffffffffffffffffffffffffffffffffe":
            system_txs.append(tx_info)
        else:
            user_txs.append(tx_info)

    return {
        "meta": {
            "name": block_entry["name"],
            "why": block_entry["why"],
            "fetched_at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        },
        "header": {
            "number": block_entry["block"],
            "number_hex": block_data.get("number"),
            "hash": block_data.get("hash"),
            "parent_hash": block_data.get("parentHash"),
            "state_root": block_data.get("stateRoot"),
            "receipts_root": block_data.get("receiptsRoot"),
            "transactions_root": block_data.get("transactionsRoot"),
            "miner": block_data.get("miner"),
            "author": author,  # from bor_getAuthor (the actual signer)
            "difficulty": block_data.get("difficulty"),
            "total_difficulty": block_data.get("totalDifficulty"),
            "gas_limit": block_data.get("gasLimit"),
            "gas_used": block_data.get("gasUsed"),
            "timestamp": block_data.get("timestamp"),
            "extra_data": block_data.get("extraData"),
            "extra_data_length": len(bytes.fromhex(block_data.get("extraData", "0x")[2:])),
            "mix_hash": block_data.get("mixHash"),
            "nonce": block_data.get("nonce"),
            "base_fee_per_gas": block_data.get("baseFeePerGas"),
            "size": block_data.get("size"),
        },
        "transactions": {
            "total_count": len(txs),
            "user_tx_count": len(user_txs),
            "system_tx_count": len(system_txs),
            "first_5_user": user_txs[:5],
            "all_system": system_txs,
        },
        "derived": {
            # These are computed from the header, useful for cross-verification
            "extra_data_vanity": block_data.get("extraData", "0x")[:66] if block_data.get("extraData") else None,  # 32 bytes = 64 hex + "0x"
            "extra_data_seal": "0x" + block_data.get("extraData", "0x")[-130:] if block_data.get("extraData") and len(block_data.get("extraData", "")) > 130 else None,  # 65 bytes = 130 hex
            "has_validator_bytes": len(bytes.fromhex(block_data.get("extraData", "0x")[2:])) > 97,
            "validator_count": (len(bytes.fromhex(block_data.get("extraData", "0x")[2:])) - 97) // 20 if len(bytes.fromhex(block_data.get("extraData", "0x")[2:])) > 97 else 0,
            "is_sprint_boundary_16": block_entry["block"] > 0 and block_entry["block"] % 16 == 0,
            "is_sprint_boundary_64": block_entry["block"] > 0 and block_entry["block"] % 64 == 0,
            "is_span_boundary_6400": block_entry["block"] > 0 and block_entry["block"] % 6400 == 0,
            "is_span_boundary_1600": block_entry["block"] > 0 and block_entry["block"] % 1600 == 0,
            "block_mod_16": block_entry["block"] % 16,
            "block_mod_64": block_entry["block"] % 64,
            "block_mod_6400": block_entry["block"] % 6400,
            "block_mod_1600": block_entry["block"] % 1600,
        },
    }


def main():
    rpc_url = DEFAULT_RPC
    if "--rpc" in sys.argv:
        idx = sys.argv.index("--rpc")
        rpc_url = sys.argv[idx + 1]

    data_dir = os.path.join(os.path.dirname(__file__), "..", "data")
    os.makedirs(data_dir, exist_ok=True)

    all_fixtures = {}
    failed = []

    for entry in BLOCKS:
        name = entry["name"]
        block_num = entry["block"]
        print(f"[{name}] Fetching block {block_num}...", end=" ", flush=True)

        try:
            block_data = fetch_block(rpc_url, block_num)
            if not block_data:
                print("EMPTY RESPONSE")
                failed.append(name)
                continue

            author = fetch_bor_author(rpc_url, block_num)
            fixture = extract_fixture(entry, block_data, author)
            all_fixtures[name] = fixture

            # Write individual fixture file
            filepath = os.path.join(data_dir, f"{name}.json")
            with open(filepath, "w") as f:
                json.dump(fixture, f, indent=2)

            gas_limit = int(block_data.get("gasLimit", "0x0"), 16)
            tx_count = len(block_data.get("transactions", []))
            print(f"OK (hash={block_data['hash'][:18]}..., txs={tx_count}, gasLimit={gas_limit:,})")

            # Rate limit
            time.sleep(0.3)

        except Exception as e:
            print(f"FAILED: {e}")
            failed.append(name)

    # Write combined fixture file
    combined_path = os.path.join(data_dir, "all_blocks.json")
    with open(combined_path, "w") as f:
        json.dump(all_fixtures, f, indent=2)

    # Write Rust source from fixtures
    rust_path = os.path.join(os.path.dirname(__file__), "..", "src", "generated.rs")
    generate_rust(all_fixtures, rust_path)

    print(f"\n{'='*60}")
    print(f"Fetched: {len(all_fixtures)}/{len(BLOCKS)} blocks")
    if failed:
        print(f"Failed:  {', '.join(failed)}")
    print(f"Output:  {data_dir}/")
    print(f"Rust:    {rust_path}")
    print(f"{'='*60}")


def generate_rust(fixtures, output_path):
    """Generate Rust source code with all fixture data as constants."""
    lines = [
        "//! Auto-generated golden block fixtures from Polygon mainnet.",
        "//! DO NOT EDIT — regenerate with: python3 scripts/fetch_golden_blocks.py",
        "//!",
        f"//! Generated: {time.strftime('%Y-%m-%dT%H:%M:%SZ', time.gmtime())}",
        f"//! Blocks: {len(fixtures)}",
        "",
        "use alloy_primitives::{Address, B256, U256};",
        "use crate::GoldenBlock;",
        "",
    ]

    # Generate each block as a const function
    for name, fixture in sorted(fixtures.items(), key=lambda x: x[1]["header"]["number"]):
        header = fixture["header"]
        derived = fixture["derived"]
        meta = fixture["meta"]

        block_num = header["number"]
        block_hash = header["hash"] or "0x" + "00" * 32
        parent_hash = header["parent_hash"] or "0x" + "00" * 32
        state_root = header["state_root"] or "0x" + "00" * 32
        receipts_root = header["receipts_root"] or "0x" + "00" * 32
        miner = header["miner"] or "0x" + "00" * 20
        difficulty_hex = header["difficulty"] or "0x0"
        gas_limit_hex = header["gas_limit"] or "0x0"
        gas_used_hex = header["gas_used"] or "0x0"
        timestamp_hex = header["timestamp"] or "0x0"
        nonce_hex = header["nonce"] or "0x0000000000000000"
        mix_hash = header["mix_hash"] or "0x" + "00" * 32
        base_fee_hex = header["base_fee_per_gas"] or "0x0"
        extra_data = header["extra_data"] or "0x"
        tx_count = fixture["transactions"]["total_count"]
        sys_tx_count = fixture["transactions"]["system_tx_count"]
        validator_count = derived["validator_count"]

        gas_limit = int(gas_limit_hex, 16)
        gas_used = int(gas_used_hex, 16)
        timestamp = int(timestamp_hex, 16)
        difficulty = int(difficulty_hex, 16)

        lines.append(f"/// {meta['why']}")
        lines.append(f"pub fn {name}() -> GoldenBlock {{")
        lines.append(f"    GoldenBlock {{")
        lines.append(f'        name: "{name}",')
        lines.append(f'        why: "{meta["why"]}",')
        lines.append(f"        number: {block_num},")
        lines.append(f'        hash: b256!("{block_hash[2:]}"),')
        lines.append(f'        parent_hash: b256!("{parent_hash[2:]}"),')
        lines.append(f'        state_root: b256!("{state_root[2:]}"),')
        lines.append(f'        receipts_root: b256!("{receipts_root[2:]}"),')
        lines.append(f'        miner: address!("{miner[2:]}"),')
        lines.append(f"        difficulty: {difficulty},")
        lines.append(f"        gas_limit: {gas_limit},")
        lines.append(f"        gas_used: {gas_used},")
        lines.append(f"        timestamp: {timestamp},")
        lines.append(f'        nonce: {int(nonce_hex, 16)},')
        lines.append(f'        mix_hash: b256!("{mix_hash[2:]}"),')
        lines.append(f'        extra_data: hex!("{extra_data[2:]}").to_vec(),')
        lines.append(f"        base_fee_per_gas: {int(base_fee_hex, 16) if base_fee_hex else 0},")
        lines.append(f"        tx_count: {tx_count},")
        lines.append(f"        system_tx_count: {sys_tx_count},")
        lines.append(f"        validator_count: {validator_count},")
        lines.append(f"        is_sprint_boundary: {str(derived['is_sprint_boundary_16']).lower()},")
        lines.append(f"        is_span_boundary_6400: {str(derived['is_span_boundary_6400']).lower()},")
        lines.append(f"        is_span_boundary_1600: {str(derived['is_span_boundary_1600']).lower()},")
        lines.append(f"    }}")
        lines.append(f"}}")
        lines.append("")

    # Generate ALL_BLOCKS array
    lines.append("/// All golden blocks sorted by block number.")
    lines.append(f"pub fn all_blocks() -> Vec<GoldenBlock> {{")
    lines.append(f"    vec![")
    for name in sorted(fixtures.keys(), key=lambda n: fixtures[n]["header"]["number"]):
        lines.append(f"        {name}(),")
    lines.append(f"    ]")
    lines.append(f"}}")

    with open(output_path, "w") as f:
        f.write("\n".join(lines) + "\n")


if __name__ == "__main__":
    main()
