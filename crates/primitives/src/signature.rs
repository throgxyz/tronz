//! Recoverable secp256k1 signature in TRON's `r || s || v` wire form.

use core::fmt;

use k256::ecdsa::{RecoveryId, Signature, VerifyingKey};

use crate::{Address, B256, error::SignatureError};

/// Length of the serialized signature: `r(32) || s(32) || v(1)`.
pub const SIGNATURE_LEN: usize = 65;

/// A secp256k1 ECDSA signature plus the recovery id needed to recover the
/// signing public key.
///
/// Serializes to the 65-byte TRON wire format `r || s || v`, where `v` is the
/// recovery id (`0` or `1`).
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct RecoverableSignature {
    r: [u8; 32],
    s: [u8; 32],
    v: u8,
}

impl RecoverableSignature {
    /// Build from a non-recoverable [`Signature`] and its [`RecoveryId`].
    pub fn from_signature(sig: &Signature, recovery_id: RecoveryId) -> Self {
        let bytes = sig.to_bytes();
        let mut r = [0u8; 32];
        let mut s = [0u8; 32];
        r.copy_from_slice(&bytes[..32]);
        s.copy_from_slice(&bytes[32..]);
        Self { r, s, v: recovery_id.to_byte() }
    }

    /// Parse the 65-byte `r || s || v` representation.
    ///
    /// Accepts a `v` of `0`/`1` or the Ethereum-style `27`/`28`, normalising to
    /// `0`/`1`.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, SignatureError> {
        if bytes.len() != SIGNATURE_LEN {
            return Err(SignatureError::BadLength(bytes.len()));
        }
        let v = match bytes[64] {
            v @ (0 | 1) => v,
            27 => 0,
            28 => 1,
            other => return Err(SignatureError::BadRecoveryId(other)),
        };
        let mut r = [0u8; 32];
        let mut s = [0u8; 32];
        r.copy_from_slice(&bytes[..32]);
        s.copy_from_slice(&bytes[32..64]);
        Ok(Self { r, s, v })
    }

    /// The 32-byte `r` scalar.
    pub fn r(&self) -> &[u8; 32] {
        &self.r
    }

    /// The 32-byte `s` scalar.
    pub fn s(&self) -> &[u8; 32] {
        &self.s
    }

    /// The recovery id (`0` or `1`).
    pub fn v(&self) -> u8 {
        self.v
    }

    /// Serialize to the 65-byte `r || s || v` wire form (`v` = `0`/`1`).
    ///
    /// TRON's native transaction encoding. For TronWeb/TronLink message
    /// signatures (`v` = `27`/`28`) use [`to_legacy_bytes`](Self::to_legacy_bytes).
    pub fn to_bytes(&self) -> [u8; SIGNATURE_LEN] {
        let mut out = [0u8; SIGNATURE_LEN];
        out[..32].copy_from_slice(&self.r);
        out[32..64].copy_from_slice(&self.s);
        out[64] = self.v;
        out
    }

    /// Serialize to `r || s || v` using the legacy `27`/`28` recovery id.
    ///
    /// Matches TronWeb `signMessageV2` / TronLink output (ends in `1b`/`1c`).
    /// The internal `v` stays `0`/`1`; only this encoding adds the `+27` offset.
    pub fn to_legacy_bytes(&self) -> [u8; SIGNATURE_LEN] {
        let mut out = self.to_bytes();
        out[64] += 27;
        out
    }

    /// Recover the TRON address that produced this signature over `prehash`.
    pub fn recover_address_from_prehash(&self, prehash: B256) -> Result<Address, SignatureError> {
        let (sig, recid) = self.split()?;
        let vk = VerifyingKey::recover_from_prehash(prehash.as_slice(), &sig, recid)?;
        Ok(Address::from_public_key(&vk))
    }

    /// Recover the non-recoverable [`Signature`] and [`RecoveryId`] components.
    pub fn split(&self) -> Result<(Signature, RecoveryId), SignatureError> {
        let mut rs = [0u8; 64];
        rs[..32].copy_from_slice(&self.r);
        rs[32..].copy_from_slice(&self.s);
        let sig = Signature::from_slice(&rs)?;
        let recid = RecoveryId::from_byte(self.v).ok_or(SignatureError::BadRecoveryId(self.v))?;
        Ok((sig, recid))
    }
}

impl fmt::Debug for RecoverableSignature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "RecoverableSignature(0x{})", hex::encode(self.to_bytes()))
    }
}

#[cfg(test)]
mod tests {
    use k256::ecdsa::{SigningKey, signature::hazmat::PrehashSigner};

    use super::*;

    #[test]
    fn bytes_roundtrip() {
        let mut bytes = [7u8; SIGNATURE_LEN];
        bytes[64] = 1;
        let sig = RecoverableSignature::from_bytes(&bytes).unwrap();
        assert_eq!(sig.to_bytes(), bytes);
        assert_eq!(sig.v(), 1);
    }

    #[test]
    fn normalises_eth_v() {
        let mut bytes = [3u8; SIGNATURE_LEN];
        bytes[64] = 28;
        let sig = RecoverableSignature::from_bytes(&bytes).unwrap();
        assert_eq!(sig.v(), 1);
    }

    #[test]
    fn bad_length_and_recid() {
        assert!(matches!(
            RecoverableSignature::from_bytes(&[0u8; 10]),
            Err(SignatureError::BadLength(10))
        ));
        let mut bytes = [0u8; SIGNATURE_LEN];
        bytes[64] = 5;
        assert!(matches!(
            RecoverableSignature::from_bytes(&bytes),
            Err(SignatureError::BadRecoveryId(5))
        ));
    }

    #[test]
    fn from_signature_and_split() {
        let signing = SigningKey::from_bytes(&[1u8; 32].into()).unwrap();
        let (sig, recid): (Signature, RecoveryId) = signing.sign_prehash(&[9u8; 32]).unwrap();
        let rec = RecoverableSignature::from_signature(&sig, recid);
        let (sig2, recid2) = rec.split().unwrap();
        assert_eq!(sig, sig2);
        assert_eq!(recid.to_byte(), recid2.to_byte());
    }

    #[test]
    fn recover_address_round_trips() {
        use k256::ecdsa::signature::hazmat::PrehashSigner;

        let signing = SigningKey::from_bytes(&[1u8; 32].into()).unwrap();
        let expected = crate::Address::from_public_key(signing.verifying_key());
        let prehash = crate::B256::repeat_byte(0x42);
        let (sig, recid): (Signature, RecoveryId) =
            signing.sign_prehash(prehash.as_slice()).unwrap();
        let rec = RecoverableSignature::from_signature(&sig, recid);
        assert_eq!(rec.recover_address_from_prehash(prehash).unwrap(), expected);
    }

    #[test]
    fn to_bytes_stays_0_1_and_legacy_is_27_28() {
        // Guard: transaction signing relies on to_bytes() staying 0/1.
        for v in [0u8, 1] {
            let mut bytes = [3u8; SIGNATURE_LEN];
            bytes[64] = v;
            let sig = RecoverableSignature::from_bytes(&bytes).unwrap();
            assert!(matches!(sig.to_bytes()[64], 0 | 1));
            assert!(matches!(sig.to_legacy_bytes()[64], 27 | 28));
            assert_eq!(sig.to_bytes()[..64], sig.to_legacy_bytes()[..64]);
            assert_eq!(RecoverableSignature::from_bytes(&sig.to_legacy_bytes()).unwrap(), sig);
        }
    }
}
