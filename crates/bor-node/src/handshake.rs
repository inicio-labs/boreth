//! Custom Bor eth RLPx handshake.
//!
//! Go-Bor's eth/69 Status message includes a `TD` (Total Difficulty) field
//! that standard eth/69 (EIP-7642) removed. This custom handshake handles
//! both Go-Bor's non-standard format and standard Reth/Geth format.

use alloy_chains::Chain;
use alloy_primitives::{B256, U256};
use alloy_rlp::{Decodable, Encodable, RlpDecodable, RlpEncodable};
use bytes::{Bytes, BytesMut};
use futures::SinkExt;
use reth_eth_wire::{
    errors::{EthHandshakeError, EthStreamError, P2PStreamError},
    handshake::{EthRlpxHandshake, UnauthEth},
    CanDisconnect, DisconnectReason, EthMessage, EthMessageID, EthNetworkPrimitives, EthVersion,
    ProtocolMessage, UnifiedStatus,
};
use reth_ethereum_forks::{ForkFilter, ForkId};
use reth_primitives_traits::GotExpected;
use std::{fmt::Debug, future::Future, pin::Pin, time::Duration};
use tokio::time::timeout;
use tokio_stream::StreamExt;
use tracing::{debug, trace};

/// Go-Bor's eth/69 Status message (8 fields, includes TD).
///
/// ```text
/// [version, chain, td, genesis, forkid, earliest, latest, blockhash]
/// ```
///
/// Standard eth/69 omits `td` and has only 7 fields.
#[derive(Debug, Clone, PartialEq, Eq, RlpEncodable, RlpDecodable)]
pub struct BorStatusEth69 {
    /// Protocol version (69).
    pub version: EthVersion,
    /// Chain ID.
    pub chain: Chain,
    /// Total difficulty (Bor-specific, not in standard eth/69).
    pub total_difficulty: U256,
    /// Genesis hash.
    pub genesis: B256,
    /// Fork ID.
    pub forkid: ForkId,
    /// Earliest block this node can serve.
    pub earliest: u64,
    /// Latest block number.
    pub latest: u64,
    /// Hash of the latest block.
    pub blockhash: B256,
}

/// Custom Bor RLPx handshake that handles Go-Bor's non-standard eth/69 Status.
#[derive(Debug, Default, Clone)]
#[non_exhaustive]
pub struct BorRlpxHandshake;

impl EthRlpxHandshake for BorRlpxHandshake {
    fn handshake<'a>(
        &'a self,
        unauth: &'a mut dyn UnauthEth,
        status: UnifiedStatus,
        fork_filter: ForkFilter,
        timeout_limit: Duration,
    ) -> Pin<Box<dyn Future<Output = Result<UnifiedStatus, EthStreamError>> + 'a + Send>> {
        Box::pin(async move {
            timeout(timeout_limit, bor_eth_handshake(unauth, status, fork_filter))
                .await
                .map_err(|_| EthStreamError::StreamTimeout)?
        })
    }
}

/// Perform the Bor-specific eth protocol handshake.
///
/// When negotiated version is eth/69, sends and receives Go-Bor's format
/// (with TD). For eth/68 and below, uses standard Reth format.
async fn bor_eth_handshake<S>(
    unauth: &mut S,
    unified_status: UnifiedStatus,
    fork_filter: ForkFilter,
) -> Result<UnifiedStatus, EthStreamError>
where
    S: tokio_stream::Stream<Item = Result<BytesMut, P2PStreamError>>
        + futures::Sink<Bytes, Error = P2PStreamError>
        + CanDisconnect<Bytes>
        + Unpin
        + Send
        + ?Sized,
{
    let version = unified_status.version;

    // Encode and send our status
    let status_bytes: Bytes = if version >= EthVersion::Eth69 {
        // Send Go-Bor format with TD
        let bor_status = BorStatusEth69 {
            version,
            chain: unified_status.chain,
            total_difficulty: unified_status.total_difficulty.unwrap_or(U256::ZERO),
            genesis: unified_status.genesis,
            forkid: unified_status.forkid,
            earliest: unified_status.earliest_block.unwrap_or(0),
            latest: unified_status.latest_block.unwrap_or(0),
            blockhash: unified_status.blockhash,
        };
        // Encode as: message_id (0x00) + RLP(BorStatusEth69)
        let mut buf = Vec::new();
        EthMessageID::Status.encode(&mut buf);
        bor_status.encode(&mut buf);
        buf.into()
    } else {
        // Standard legacy format
        let status_msg = unified_status.into_message();
        alloy_rlp::encode(ProtocolMessage::<EthNetworkPrimitives>::from(
            EthMessage::Status(status_msg),
        ))
        .into()
    };

    unauth.send(status_bytes).await.map_err(EthStreamError::from)?;

    // Receive peer's status
    let their_msg = match unauth.next().await {
        Some(Ok(msg)) => msg,
        Some(Err(e)) => return Err(EthStreamError::from(e)),
        None => {
            unauth
                .disconnect(DisconnectReason::DisconnectRequested)
                .await
                .map_err(EthStreamError::from)?;
            return Err(EthStreamError::EthHandshakeError(
                EthHandshakeError::NoResponse,
            ));
        }
    };

    if their_msg.len() > 10 * 1024 * 1024 {
        unauth
            .disconnect(DisconnectReason::ProtocolBreach)
            .await
            .map_err(EthStreamError::from)?;
        return Err(EthStreamError::MessageTooBig(their_msg.len()));
    }

    // Decode the peer's status
    let their_status = if version >= EthVersion::Eth69 {
        // Try Go-Bor format first (8 fields with TD), fall back to standard (7 fields)
        decode_bor_eth69_status(&their_msg)?
    } else {
        // Standard legacy decode
        match ProtocolMessage::<EthNetworkPrimitives>::decode_status(version, &mut their_msg.as_ref())
        {
            Ok(msg) => UnifiedStatus::from_message(msg),
            Err(err) => {
                debug!("decode error in eth handshake: msg={their_msg:x}");
                unauth
                    .disconnect(DisconnectReason::ProtocolBreach)
                    .await
                    .map_err(EthStreamError::from)?;
                return Err(EthStreamError::InvalidMessage(err));
            }
        }
    };

    debug!(
        their_version = %their_status.version,
        their_chain = %their_status.chain,
        their_genesis = %their_status.genesis,
        their_forkid = ?their_status.forkid,
        our_genesis = %unified_status.genesis,
        our_chain = %unified_status.chain,
        "Validating incoming ETH status from Bor peer"
    );

    // Validate genesis
    if unified_status.genesis != their_status.genesis {
        debug!("Genesis mismatch: rejecting peer");
        unauth
            .disconnect(DisconnectReason::ProtocolBreach)
            .await
            .map_err(EthStreamError::from)?;
        return Err(EthHandshakeError::MismatchedGenesis(
            GotExpected {
                expected: unified_status.genesis,
                got: their_status.genesis,
            }
            .into(),
        )
        .into());
    }

    // Validate version
    if version != their_status.version {
        unauth
            .disconnect(DisconnectReason::ProtocolBreach)
            .await
            .map_err(EthStreamError::from)?;
        return Err(EthHandshakeError::MismatchedProtocolVersion(GotExpected {
            got: their_status.version,
            expected: version,
        })
        .into());
    }

    // Validate chain
    if unified_status.chain != their_status.chain {
        unauth
            .disconnect(DisconnectReason::ProtocolBreach)
            .await
            .map_err(EthStreamError::from)?;
        return Err(EthHandshakeError::MismatchedChain(GotExpected {
            got: their_status.chain,
            expected: unified_status.chain,
        })
        .into());
    }

    // Validate TD (if present)
    if let Some(td) = their_status.total_difficulty {
        if td.bit_len() > 160 {
            unauth
                .disconnect(DisconnectReason::ProtocolBreach)
                .await
                .map_err(EthStreamError::from)?;
            return Err(EthHandshakeError::TotalDifficultyBitLenTooLarge {
                got: td.bit_len(),
                maximum: 160,
            }
            .into());
        }
    }

    // Validate fork ID
    if let Err(err) = fork_filter
        .validate(their_status.forkid)
        .map_err(EthHandshakeError::InvalidFork)
    {
        debug!(?err, their_forkid = ?their_status.forkid, "Fork ID validation failed");
        unauth
            .disconnect(DisconnectReason::ProtocolBreach)
            .await
            .map_err(EthStreamError::from)?;
        return Err(err.into());
    }

    // Validate block range for eth/69
    if let (Some(earliest), Some(latest)) = (their_status.earliest_block, their_status.latest_block)
    {
        if earliest > latest {
            return Err(EthHandshakeError::EarliestBlockGreaterThanLatestBlock {
                got: earliest,
                latest,
            }
            .into());
        }

        if their_status.blockhash.is_zero() {
            return Err(EthHandshakeError::BlockhashZero.into());
        }
    }

    debug!("Bor eth handshake completed successfully");
    Ok(their_status)
}

/// Decode a Go-Bor eth/69 Status message (with TD) from raw bytes.
///
/// Falls back to standard eth/69 format if Bor format fails.
fn decode_bor_eth69_status(msg: &[u8]) -> Result<UnifiedStatus, EthStreamError> {
    let mut buf = msg;

    // Decode message ID
    let message_type = EthMessageID::decode(&mut buf).map_err(|e| {
        EthStreamError::InvalidMessage(reth_eth_wire::message::MessageError::RlpError(e))
    })?;

    if message_type != EthMessageID::Status {
        return Err(EthStreamError::InvalidMessage(
            reth_eth_wire::message::MessageError::ExpectedStatusMessage(message_type),
        ));
    }

    // Try Go-Bor format first (8 fields with TD)
    if let Ok(bor_status) = BorStatusEth69::decode(&mut buf.to_owned().as_slice()) {
        debug!(
            chain = %bor_status.chain,
            genesis = %bor_status.genesis,
            td = %bor_status.total_difficulty,
            latest = bor_status.latest,
            "Decoded Go-Bor eth/69 Status (with TD)"
        );
        return Ok(UnifiedStatus {
            version: bor_status.version,
            chain: bor_status.chain,
            genesis: bor_status.genesis,
            forkid: bor_status.forkid,
            blockhash: bor_status.blockhash,
            total_difficulty: Some(bor_status.total_difficulty),
            earliest_block: Some(bor_status.earliest),
            latest_block: Some(bor_status.latest),
        });
    }

    // Fall back to standard eth/69 format (7 fields, no TD)
    match ProtocolMessage::<EthNetworkPrimitives>::decode_status(
        EthVersion::Eth69,
        &mut msg.to_owned().as_slice(),
    ) {
        Ok(status) => Ok(UnifiedStatus::from_message(status)),
        Err(err) => {
            debug!("decode error in Bor eth/69 handshake: msg={msg:x?}");
            Err(EthStreamError::InvalidMessage(err))
        }
    }
}
