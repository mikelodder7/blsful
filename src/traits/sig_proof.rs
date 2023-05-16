use crate::*;
use bls12_381_plus::elliptic_curve::{Field, Group};
use bls12_381_plus::group::GroupEncoding;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Methods for creating a signature proof of knowledge as in
/// <https://miracl.com/assets/pdf-downloads/mpin4.pdf>
pub trait BlsSignatureProof:
    Pairing
    + HashToPoint<Output = Self::Signature>
    + HashToScalar<Output = <Self::Signature as Group>::Scalar>
{
    /// The domain separation tag for the hash function
    const DST: &'static [u8];

    /// Create the value `U` and `x`
    fn generate_commitment<B: AsRef<[u8]>, D: AsRef<[u8]>>(
        msg: B,
        dst: D,
    ) -> BlsResult<(Self::Signature, <Self::Signature as Group>::Scalar)> {
        let x = <Self::Signature as Group>::Scalar::random(get_crypto_rng());
        if x.is_zero().into() {
            return Err(BlsError::InvalidInputs("x is zero".to_string()));
        }
        let a = Self::hash_to_point(msg, dst);
        Ok((a * x, x))
    }

    /// Create the timestamp based challenge for `y`
    fn generate_timestamp_based_y(u: Self::Signature) -> (<Self::Signature as Group>::Scalar, u64) {
        let t = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        (Self::compute_y(u, t), t)
    }

    /// Shared methods for generating `y` challenge
    fn compute_y(u: Self::Signature, t: u64) -> <Self::Signature as Group>::Scalar {
        let u_bytes = u.to_bytes();
        let u_ref = u_bytes.as_ref();
        let u_len = u_ref.len();
        let mut bytes = vec![0u8; 8 + u_len];
        bytes[..u_len].copy_from_slice(u_ref);
        bytes[u_len..].copy_from_slice(&t.to_le_bytes());
        Self::hash_to_scalar(&bytes, Self::DST)
    }

    /// Create the value `V`
    fn generate_proof(
        commitment: Self::Signature,
        x: <Self::Signature as Group>::Scalar,
        y: <Self::Signature as Group>::Scalar,
        sig: Self::Signature,
    ) -> BlsResult<(Self::Signature, Self::Signature)> {
        if commitment.is_identity().into() {
            return Err(BlsError::InvalidInputs(
                "commitment is the identity point".to_string(),
            ));
        }
        if sig.is_identity().into() {
            return Err(BlsError::InvalidInputs(
                "signature is the identity point".to_string(),
            ));
        }
        if x.is_zero().into() {
            return Err(BlsError::InvalidInputs("x is the zero".to_string()));
        }
        if y.is_zero().into() {
            return Err(BlsError::InvalidInputs("y is the zero".to_string()));
        }
        Ok((commitment, -(sig * (x + y))))
    }

    /// Create the value `V` using a timestamp
    fn generate_timestamp_proof<B: AsRef<[u8]>, D: AsRef<[u8]>>(
        msg: B,
        dst: D,
        sig: Self::Signature,
    ) -> BlsResult<(Self::Signature, Self::Signature, u64)> {
        if sig.is_identity().into() {
            return Err(BlsError::InvalidInputs(
                "signature is the identity point".to_string(),
            ));
        }
        let x = <Self::Signature as Group>::Scalar::random(get_crypto_rng());
        let a = Self::hash_to_point(msg, dst);
        let u = a * x;
        let (y, t) = Self::generate_timestamp_based_y(u);
        let v = sig * (x + y);
        Ok((u, -v, t))
    }

    /// Verify the signature proof of knowledge
    fn verify<B: AsRef<[u8]>, D: AsRef<[u8]>>(
        commitment: Self::Signature,
        proof: Self::Signature,
        pk: Self::PublicKey,
        y: <Self::Signature as Group>::Scalar,
        msg: B,
        dst: D,
    ) -> BlsResult<()> {
        if commitment.is_identity().into() {
            return Err(BlsError::InvalidInputs(
                "commitment is the identity point".to_string(),
            ));
        }
        if proof.is_identity().into() {
            return Err(BlsError::InvalidInputs(
                "proof is the identity point".to_string(),
            ));
        }
        if pk.is_identity().into() {
            return Err(BlsError::InvalidInputs(
                "pk is the identity point".to_string(),
            ));
        }
        if y.is_zero().into() {
            return Err(BlsError::InvalidInputs("y is the zero".to_string()));
        }

        let a = Self::hash_to_point(msg, dst);
        if Self::pairing(&[
            (proof, <Self::PublicKey as Group>::generator()),
            (commitment + a * y, pk),
        ])
        .is_identity()
        .into()
        {
            Ok(())
        } else {
            Err(BlsError::InvalidProof)
        }
    }

    /// Verify a timestamp proof of knowledge
    fn verify_timestamp_proof<B: AsRef<[u8]>, D: AsRef<[u8]>>(
        commitment: Self::Signature,
        proof: Self::Signature,
        pk: Self::PublicKey,
        t: u64,
        timeout_ms: u64,
        msg: B,
        dst: D,
    ) -> BlsResult<()> {
        let now = SystemTime::now();
        let since = UNIX_EPOCH + Duration::from_millis(t);
        let elapsed = now.duration_since(since).unwrap().as_millis() as u64;
        if elapsed > timeout_ms {
            return Err(BlsError::InvalidProof);
        }

        let y = Self::compute_y(commitment, t);
        Self::verify(commitment, proof, pk, y, msg, dst)
    }
}
