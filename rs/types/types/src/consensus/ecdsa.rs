//! Defines types used for threshold ECDSA key generation.

// TODO: Remove once we have implemented the functionality
#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub use crate::consensus::ecdsa_refs::{
    EcdsaBlockReader, IDkgTranscriptOperationRef, IDkgTranscriptParamsRef, MaskedTranscript,
    PreSignatureQuadrupleRef, QuadrupleInCreation, RandomTranscriptParams, RequestId, RequestIdTag,
    ReshareOfMaskedParams, ReshareOfUnmaskedParams, ThresholdEcdsaSigInputsRef,
    TranscriptCastError, TranscriptLookupError, TranscriptRef, UnmaskedTimesMaskedParams,
    UnmaskedTranscript,
};
use crate::consensus::{BasicSignature, MultiSignature, MultiSignatureShare};
use crate::crypto::{
    canister_threshold_sig::idkg::{IDkgDealing, IDkgTranscript, IDkgTranscriptId},
    canister_threshold_sig::{ThresholdEcdsaCombinedSignature, ThresholdEcdsaSigShare},
    CryptoHashOf, Signed, SignedBytesWithoutDomainSeparator,
};
use crate::{Height, NodeId};

pub type EcdsaSignature = ThresholdEcdsaCombinedSignature;

/// The payload information necessary for ECDSA threshold signatures, that is
/// published on every consensus round. It represents the current state of
/// the protocol since the summary block.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EcdsaDataPayload {
    /// Signatures that we agreed upon in this round.
    pub signature_agreements: BTreeMap<RequestId, EcdsaSignature>,

    /// The `RequestIds` for which we are currently generating signatures.
    pub ongoing_signatures: BTreeMap<RequestId, ThresholdEcdsaSigInputsRef>,

    /// ECDSA transcript quadruples that we can use to create ECDSA signatures.
    pub available_quadruples: BTreeMap<QuadrupleId, PreSignatureQuadrupleRef>,

    /// Ecdsa Quadruple in creation.
    pub quadruples_in_creation: BTreeMap<QuadrupleId, QuadrupleInCreation>,

    /// Next TranscriptId that is incremented after creating a new transcript.
    pub next_unused_transcript_id: IDkgTranscriptId,

    /// Progress of creating the next ECDSA key transcript.
    pub next_key_transcript_creation: Option<KeyTranscriptCreation>,

    /// Transcripts created at this height.
    pub idkg_transcripts: BTreeMap<IDkgTranscriptId, IDkgTranscript>,
}

/// The creation of an ecdsa key transcript goes through one of the two paths below:
/// 1. RandomTranscript -> ReshareOfMasked -> Created
/// 2. ReshareOfUnmasked -> Created
///
/// The initial bootstrap will start with an empty 'EcdsaSummaryPayload', and then
/// we'll go through the first path to create the key transcript.
///
/// After the initial key transcript is created, we will be able to create the first
/// 'EcdsaSummaryPayload' by carrying over the key transcript, which will be carried
/// over to the next DKG interval if there is no node membership change.
///
/// If in the future there is a membership change, we will create a new key transcript
/// by going through the second path above. Then the switch-over will happen at
/// the next 'EcdsaSummaryPayload'.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum KeyTranscriptCreation {
    // Configuration to create initial random transcript.
    RandomTranscriptParams(RandomTranscriptParams),
    // Configuration to create initial key transcript by resharing the random transcript.
    ReshareOfMaskedParams(ReshareOfMaskedParams),
    // Configuration to create next key transcript by resharing the current key transcript.
    ReshareOfUnmaskedParams(ReshareOfUnmaskedParams),
    // Created
    Created(UnmaskedTranscript),
}

impl EcdsaDataPayload {
    /// Return an iterator of all transcript configs that have no matching
    /// results yet.
    pub fn iter_transcript_configs_in_creation(
        &self,
    ) -> Box<dyn Iterator<Item = &IDkgTranscriptParamsRef> + '_> {
        let iter =
            self.next_key_transcript_creation
                .iter()
                .filter_map(|transcript| match transcript {
                    KeyTranscriptCreation::RandomTranscriptParams(x) => Some(x.as_ref()),
                    KeyTranscriptCreation::ReshareOfMaskedParams(x) => Some(x.as_ref()),
                    KeyTranscriptCreation::ReshareOfUnmaskedParams(x) => Some(x.as_ref()),
                    KeyTranscriptCreation::Created(_) => None,
                });
        Box::new(
            self.quadruples_in_creation
                .iter()
                .map(|(_, quadruple)| quadruple.iter_transcript_configs_in_creation())
                .flatten()
                .chain(iter),
        )
    }
}

/// The payload information necessary for ECDSA threshold signatures, that is
/// published on summary blocks.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EcdsaSummaryPayload {
    /// The `RequestIds` for which we are currently generating signatures.
    pub ongoing_signatures: BTreeMap<RequestId, ThresholdEcdsaSigInputsRef>,

    /// The ECDSA key transcript used for the corresponding interval.
    pub current_key_transcript: UnmaskedTranscript,

    /// ECDSA transcript quadruples that we can use to create ECDSA signatures.
    pub available_quadruples: BTreeMap<QuadrupleId, PreSignatureQuadrupleRef>,

    /// Next TranscriptId that is incremented after creating a new transcript.
    pub next_unused_transcript_id: IDkgTranscriptId,

    /// Full copy of the transcripts referred to by the parent payload block.
    pub idkg_transcripts: BTreeMap<IDkgTranscriptId, IDkgTranscript>,
}

#[derive(
    Copy, Clone, Default, Debug, PartialOrd, Ord, PartialEq, Eq, Serialize, Deserialize, Hash,
)]
pub struct QuadrupleId(pub usize);

impl QuadrupleId {
    pub fn increment(self) -> QuadrupleId {
        QuadrupleId(self.0 + 1)
    }
}

/// The ECDSA artifact.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum EcdsaMessage {
    EcdsaSignedDealing(EcdsaSignedDealing),
    EcdsaDealingSupport(EcdsaDealingSupport),
    EcdsaSigShare(EcdsaSigShare),
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Hash)]
pub enum EcdsaMessageHash {
    EcdsaSignedDealing(CryptoHashOf<EcdsaSignedDealing>),
    EcdsaDealingSupport(CryptoHashOf<EcdsaDealingSupport>),
    EcdsaSigShare(CryptoHashOf<EcdsaSigShare>),
}

/// The dealing content generated by a dealer
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub struct EcdsaDealing {
    /// Height of the finalized block that requested the transcript
    pub requested_height: Height,

    /// The crypto dealing
    /// TODO: dealers should send the BasicSigned<> dealing
    pub idkg_dealing: IDkgDealing,
}

impl SignedBytesWithoutDomainSeparator for EcdsaDealing {
    fn as_signed_bytes_without_domain_separator(&self) -> Vec<u8> {
        serde_cbor::to_vec(&self).unwrap()
    }
}

/// The signed dealing sent by dealers
pub type EcdsaSignedDealing = Signed<EcdsaDealing, BasicSignature<EcdsaDealing>>;

impl EcdsaSignedDealing {
    pub fn get(&self) -> &EcdsaDealing {
        &self.content
    }
}

impl SignedBytesWithoutDomainSeparator for EcdsaSignedDealing {
    fn as_signed_bytes_without_domain_separator(&self) -> Vec<u8> {
        serde_cbor::to_vec(&self).unwrap()
    }
}

/// TODO: EcdsaDealing can be big, consider sending only the signature
/// as part of the shares
/// The individual signature share in support of a dealing
pub type EcdsaDealingSupport = Signed<EcdsaDealing, MultiSignatureShare<EcdsaDealing>>;

/// The multi-signature verified dealing
pub type EcdsaVerifiedDealing = Signed<EcdsaDealing, MultiSignature<EcdsaDealing>>;

/// The ECDSA signature share
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub struct EcdsaSigShare {
    /// Height of the finalized block that requested the signature
    pub requested_height: Height,

    /// The node that signed the share
    pub signer_id: NodeId,

    /// The request this signature share belongs to
    pub request_id: RequestId,

    /// The signature share
    pub share: ThresholdEcdsaSigShare,
}

/// The final output of the transcript creation sequence
pub type EcdsaTranscript = IDkgTranscript;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EcdsaMessageAttribute {
    EcdsaSignedDealing(Height),
    EcdsaDealingSupport(Height),
    EcdsaSigShare(Height),
}

impl From<&EcdsaMessage> for EcdsaMessageAttribute {
    fn from(msg: &EcdsaMessage) -> EcdsaMessageAttribute {
        match msg {
            EcdsaMessage::EcdsaSignedDealing(dealing) => {
                EcdsaMessageAttribute::EcdsaSignedDealing(dealing.content.requested_height)
            }
            EcdsaMessage::EcdsaDealingSupport(support) => {
                EcdsaMessageAttribute::EcdsaDealingSupport(support.content.requested_height)
            }
            EcdsaMessage::EcdsaSigShare(share) => {
                EcdsaMessageAttribute::EcdsaSigShare(share.requested_height)
            }
        }
    }
}

#[allow(missing_docs)]
/// Mock module of the crypto types that are needed by consensus for threshold
/// ECDSA generation. These types should be replaced by the real Types once they
/// are available.
pub mod ecdsa_crypto_mock {
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Hash)]
    pub struct EcdsaComplaint;

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Hash)]
    pub struct EcdsaOpening;
}

// The ECDSA summary.
pub type Summary = Option<EcdsaSummaryPayload>;

pub type Payload = Option<EcdsaDataPayload>;
