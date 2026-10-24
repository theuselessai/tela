use super::{
    bytes, fail, fixed,
    handles::{self, Material},
    integer, output, CryptoRuntime, MAX_BYTES,
};
use chacha20poly1305::{
    aead::{Aead, Payload},
    ChaCha20Poly1305, KeyInit,
};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use hmac::{Hmac, Mac};
use p256::{
    elliptic_curve::{
        ff::PrimeField,
        group::Group,
        sec1::{FromEncodedPoint, ToEncodedPoint},
    },
    AffinePoint, EncodedPoint, ProjectivePoint, Scalar,
};
use rand_core::{OsRng, RngCore};
use rquickjs::{Ctx, Result, Value};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use subtle::ConstantTimeEq;
use x25519_dalek::{PublicKey, StaticSecret};
use zeroize::Zeroizing;

pub(super) fn dispatch<'js>(
    ctx: &Ctx<'js>,
    state: &Arc<CryptoRuntime>,
    op: &str,
    args: &[Value<'js>],
) -> Result<Value<'js>> {
    let arity = match op {
        "scalarRandomNonzero" | "generate" => 0,
        "randomBytes" | "sha256" | "scalarImport32" | "scalarReduceBE" | "scalarExport32"
        | "pointImport65" | "pointExport65" | "baseMul" | "isIdentity" | "importPrivate32"
        | "publicKey" | "importKey32" => 1,
        "ed25519Verify" => 3,
        "hkdfSha256" | "seal" | "open" => 4,
        _ => 2,
    };
    if args.len() != arity {
        return Err(fail(ctx, "crypto invalid argument count"));
    }
    let a = args;
    match op {
        "randomBytes" => {
            let len = integer(ctx, &a[0], 0, MAX_BYTES)?;
            let mut out = Zeroizing::new(vec![0; len]);
            OsRng
                .try_fill_bytes(&mut out)
                .map_err(|_| fail(ctx, "crypto entropy unavailable"))?;
            output(ctx, &out)
        }
        "sha256" => output(ctx, &Sha256::digest(&*bytes(ctx, &a[0], MAX_BYTES)?)),
        "hmacSha256" => {
            let key = bytes(ctx, &a[0], MAX_BYTES)?;
            let data = bytes(ctx, &a[1], MAX_BYTES)?;
            let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(&key)
                .map_err(|_| fail(ctx, "crypto invalid key"))?;
            mac.update(&data);
            output(ctx, &mac.finalize().into_bytes())
        }
        "hkdfSha256" => {
            let ikm = bytes(ctx, &a[0], MAX_BYTES)?;
            let salt = if a[1].is_null() {
                None
            } else {
                Some(bytes(ctx, &a[1], MAX_BYTES)?)
            };
            let info = bytes(ctx, &a[2], MAX_BYTES)?;
            let len = integer(ctx, &a[3], 0, 8160)?;
            let mut out = Zeroizing::new(vec![0; len]);
            hkdf::Hkdf::<Sha256>::new(salt.as_ref().map(|s| s.as_slice()), &ikm)
                .expand(&info, &mut out)
                .map_err(|_| fail(ctx, "crypto invalid HKDF length"))?;
            output(ctx, &out)
        }
        "timingSafeEqual" => {
            let left = bytes(ctx, &a[0], MAX_BYTES)?;
            let right = bytes(ctx, &a[1], MAX_BYTES)?;
            Ok(Value::new_bool(
                ctx.clone(),
                bool::from(left.as_slice().ct_eq(right.as_slice())),
            ))
        }
        "ed25519Verify" => {
            let pk = fixed::<32>(ctx, &a[0])?;
            let msg = bytes(ctx, &a[1], MAX_BYTES)?;
            let sig = fixed::<64>(ctx, &a[2])?;
            // Match ed25519-dalek Verifier::verify, deliberately not verify_strict.
            let valid = VerifyingKey::from_bytes(&pk)
                .and_then(|k| k.verify(&msg, &Signature::from_bytes(&sig)))
                .is_ok();
            Ok(Value::new_bool(ctx.clone(), valid))
        }
        "scalarRandomNonzero" => {
            let mut raw = Zeroizing::new([0u8; 32]);
            loop {
                OsRng
                    .try_fill_bytes(&mut *raw)
                    .map_err(|_| fail(ctx, "crypto entropy unavailable"))?;
                if let Some(s) = Option::<Scalar>::from(Scalar::from_repr((*raw).into())) {
                    if s != Scalar::ZERO {
                        return handles::create(ctx, state, Material::Scalar(s));
                    }
                }
            }
        }
        "scalarImport32" => {
            let raw = fixed::<32>(ctx, &a[0])?;
            let s = Option::<Scalar>::from(Scalar::from_repr((*raw).into()))
                .ok_or_else(|| fail(ctx, "crypto noncanonical scalar"))?;
            handles::create(ctx, state, Material::Scalar(s))
        }
        "scalarReduceBE" => {
            let raw = bytes(ctx, &a[0], 64)?;
            let mut s = Zeroizing::new(Scalar::ZERO);
            for &b in raw.iter() {
                *s = *s * Scalar::from(256u64) + Scalar::from(b as u64);
            }
            handles::create(ctx, state, Material::Scalar(*s))
        }
        "scalarExport32" => {
            let s = handles::scalar(ctx, &a[0])?;
            let raw = Zeroizing::new(s.to_bytes());
            output(ctx, &raw)
        }
        "pointImport65" => {
            let raw = fixed::<65>(ctx, &a[0])?;
            if raw[0] != 4 {
                return Err(fail(ctx, "crypto requires uncompressed point"));
            }
            let encoded =
                EncodedPoint::from_bytes(&*raw).map_err(|_| fail(ctx, "crypto invalid point"))?;
            let affine = Option::<AffinePoint>::from(AffinePoint::from_encoded_point(&encoded))
                .ok_or_else(|| fail(ctx, "crypto invalid point"))?;
            let point = ProjectivePoint::from(affine);
            if bool::from(point.is_identity()) {
                return Err(fail(ctx, "crypto identity encoding rejected"));
            }
            handles::create(ctx, state, Material::Point(point))
        }
        "pointExport65" => {
            let point = handles::point(ctx, &a[0])?;
            if bool::from(point.is_identity()) {
                return Err(fail(ctx, "crypto identity encoding rejected"));
            }
            output(ctx, point.to_affine().to_encoded_point(false).as_bytes())
        }
        "baseMul" => {
            let scalar = handles::scalar(ctx, &a[0])?;
            handles::create(
                ctx,
                state,
                Material::Point(ProjectivePoint::GENERATOR * *scalar),
            )
        }
        "mul" => {
            let point = handles::point(ctx, &a[0])?;
            let scalar = handles::scalar(ctx, &a[1])?;
            handles::create(ctx, state, Material::Point(point * *scalar))
        }
        "add" | "sub" => {
            let left = handles::point(ctx, &a[0])?;
            let right = handles::point(ctx, &a[1])?;
            handles::create(
                ctx,
                state,
                Material::Point(if op == "add" {
                    left + right
                } else {
                    left - right
                }),
            )
        }
        "isIdentity" => Ok(Value::new_bool(
            ctx.clone(),
            bool::from(handles::point(ctx, &a[0])?.is_identity()),
        )),
        "generate" => {
            let mut raw = Zeroizing::new([0u8; 32]);
            OsRng
                .try_fill_bytes(&mut *raw)
                .map_err(|_| fail(ctx, "crypto entropy unavailable"))?;
            handles::create(ctx, state, Material::Private(raw))
        }
        "importPrivate32" => {
            handles::create(ctx, state, Material::Private(fixed::<32>(ctx, &a[0])?))
        }
        "publicKey" => {
            let raw = handles::secret(ctx, &a[0], true)?;
            let private = StaticSecret::from(*raw);
            output(ctx, PublicKey::from(&private).as_bytes())
        }
        "derive" => {
            let raw = handles::secret(ctx, &a[0], true)?;
            let peer = fixed::<32>(ctx, &a[1])?;
            let shared = StaticSecret::from(*raw).diffie_hellman(&PublicKey::from(*peer));
            if bool::from(shared.as_bytes().ct_eq(&[0u8; 32])) {
                return Err(fail(ctx, "crypto noncontributory peer"));
            }
            output(ctx, shared.as_bytes())
        }
        "importKey32" => handles::create(ctx, state, Material::Key(fixed::<32>(ctx, &a[0])?)),
        "seal" | "open" => {
            let key = handles::secret(ctx, &a[0], false)?;
            let nonce = fixed::<12>(ctx, &a[1])?;
            let aad = bytes(ctx, &a[2], MAX_BYTES)?;
            let data = bytes(
                ctx,
                &a[3],
                if op == "seal" {
                    MAX_BYTES - 16
                } else {
                    MAX_BYTES
                },
            )?;
            if op == "open" && data.len() < 16 {
                return Err(fail(ctx, "crypto invalid ciphertext"));
            }
            let cipher = ChaCha20Poly1305::new((&*key).into());
            let payload = Payload {
                msg: &data,
                aad: &aad,
            };
            let out = if op == "seal" {
                cipher.encrypt((&*nonce).into(), payload)
            } else {
                cipher.decrypt((&*nonce).into(), payload)
            }
            .map_err(|_| fail(ctx, "crypto authentication failed"))?;
            output(ctx, &Zeroizing::new(out))
        }
        _ => Err(fail(ctx, "crypto unknown operation")),
    }
}
