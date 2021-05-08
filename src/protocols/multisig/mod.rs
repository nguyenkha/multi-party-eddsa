#![allow(non_snake_case)]
/*
    multi-party-ed25519

    Copyright 2018 by Kzen Networks

    This file is part of multi-party-ed25519 library
    (https://github.com/KZen-networks/multisig-schnorr)

    multi-party-ed25519 is free software: you can redistribute
    it and/or modify it under the terms of the GNU General Public
    License as published by the Free Software Foundation, either
    version 3 of the License, or (at your option) any later version.

    @license GPL-3.0+ <https://github.com/KZen-networks/multi-party-ed25519/blob/master/LICENSE>
*/
//! Schnorr {n,n}-Signatures based on Accountable-Subgroup Multisignatures
//!
//See (https://pdfs.semanticscholar.org/6bf4/f9450e7a8e31c106a8670b961de4735589cf.pdf)
use curv::arithmetic::traits::Converter;
use curv::cryptographic_primitives::hashing::hash_sha256::HSha256;
use curv::cryptographic_primitives::hashing::hash_sha512::HSha512;
use curv::cryptographic_primitives::hashing::traits::*;
use curv::elliptic::curves::traits::*;
use curv::elliptic::curves::ed25519::{FE, GE};
use curv::BigInt;
use protocols::multisig;

// TODO: move to a common location to be used by all protocols.
#[derive(Debug, Clone)]
pub struct ExpendedPrivateKey {
    pub prefix: FE,
    private_key: FE,
}
// I is a private key and public key keypair, X is a commitment of the form X = xG used only in key generation (see p11 in the paper)
#[derive(Debug, Clone)]
pub struct Keys {
    pub I: ExpendedKeyPair,
    pub X: KeyPair,
}

#[derive(Debug, Clone)]
pub struct ExpendedKeyPair {
    pub public_key: GE,
    expended_private_key: ExpendedPrivateKey,
}

#[derive(Debug, Clone)]
pub struct KeyPair {
    pub public_key: GE,
    private_key: FE,
}
impl KeyPair {
    pub fn create() -> KeyPair {
        let ec_point: GE = ECPoint::generator();
        let private_key: FE = ECScalar::new_random();
        let public_key = ec_point * &private_key;
        KeyPair {
            public_key,
            private_key,
        }
    }
    pub fn create_from_private_key(private_key: FE) -> KeyPair {
        let g: GE = ECPoint::generator();
        let public_key = g * &private_key;

        KeyPair {
            public_key,
            private_key,
        }
    }
}
impl ExpendedKeyPair {
    pub fn create() -> ExpendedKeyPair {
        let sk: FE = ECScalar::new_random();
        Self::create_from_private_key(sk)
    }

    pub fn create_from_private_key(sk: FE) -> ExpendedKeyPair {
        let ec_point: GE = ECPoint::generator();
        let h = HSha512::create_hash(&vec![&sk.to_big_int()]);
        let h_vec = h.to_bytes();
        let mut h_vec_padded = vec![0; 64 - h_vec.len()];  // ensure hash result is padded to 64 bytes
        h_vec_padded.extend_from_slice(&h_vec);
        let mut private_key: [u8; 32] = [0u8; 32];
        let mut prefix: [u8; 32] = [0u8; 32];
        prefix.copy_from_slice(&h_vec_padded[32..64]);
        private_key.copy_from_slice(&h_vec_padded[00..32]);
        private_key[0] &= 248;
        private_key[31] &= 63;
        private_key[31] |= 64;
        let private_key = &private_key[..private_key.len()];
        let prefix = &prefix[..prefix.len()];
        let private_key: FE = ECScalar::from(&BigInt::from_bytes(private_key));
        let prefix: FE = ECScalar::from(&BigInt::from_bytes(prefix));
        let public_key = ec_point * &private_key;
        ExpendedKeyPair {
            public_key,
            expended_private_key: ExpendedPrivateKey {
                prefix,
                private_key,
            },
        }
    }

    pub fn update_key_pair(&mut self, to_add: FE) {
        self.expended_private_key.private_key = to_add + &self.expended_private_key.private_key;
        let g: GE = ECPoint::generator();
        self.public_key = g * &self.expended_private_key.private_key;
    }
}

impl Keys {
    pub fn create() -> Keys {
        let I = ExpendedKeyPair::create();
        let X = KeyPair::create();
        Keys { I, X }
    }

    pub fn create_from_private_keys(priv_I: FE, priv_X: FE) -> Keys {
        let I = ExpendedKeyPair::create_from_private_key(priv_I);
        let X = KeyPair::create_from_private_key(priv_X);
        Keys { I, X }
    }

    pub fn create_from(secret_share: FE) -> Keys {
        let I = ExpendedKeyPair::create_from_private_key(secret_share);
        let X = KeyPair::create();
        Keys { I, X }
    }

    pub fn create_signing_key(keys: &Keys, eph_key: &EphKey) -> Keys {
        Keys {
            I: keys.I.clone(),
            X: eph_key.eph_key_pair.clone(),
        }
    }

    pub fn broadcast(keys: Keys) -> Vec<GE> {
        return vec![keys.I.public_key, keys.X.public_key];
    }

    pub fn collect_and_compute_challenge(ix_vec: &[Vec<GE>]) -> FE {
        let concat_vec = ix_vec.iter().fold(Vec::new(), |mut acc, x| {
            acc.extend_from_slice(x);
            acc
        });
        let ref_vec = concat_vec.iter().collect::<Vec<&GE>>();
        multisig::hash_4(&ref_vec[..])
    }
}

pub fn partial_sign(keys: &Keys, e: FE) -> FE {
    e * &keys.I.expended_private_key.private_key + &keys.X.private_key
}

pub fn verify<'a>(I: &GE, sig: &Signature, e: &FE) -> Result<(), &'a str> {
    let X = &sig.X;
    let y = &sig.y;
    let base_point: GE = ECPoint::generator();
    let yG = base_point * y;
    let eI = I * e;
    let X_plus_eI = X + &eI;
    if yG == X_plus_eI {
        Ok(())
    } else {
        Err("error verification")
    }
}

fn hash_4(key_list: &[&GE]) -> FE {
    let four_fe: FE = ECScalar::from(&BigInt::from_bytes(&[4]));
    let base_point: GE = ECPoint::generator();
    let four_ge = base_point * four_fe;
    let mut four_ge_vec = vec![&four_ge];
    four_ge_vec.extend(key_list);
    HSha256::create_hash_from_ge(&four_ge_vec)
}

pub struct EphKey {
    pub eph_key_pair: KeyPair,
}

impl EphKey {
    //signing step 1
    pub fn gen_commit(key_gen_key_pair: &ExpendedKeyPair, message: &BigInt) -> EphKey {
        // here we deviate from the spec, by introducing  non-deterministic element (random number)
        // to the nonce
        let r = HSha512::create_hash(&vec![
            &key_gen_key_pair.expended_private_key.prefix.to_big_int(),
            &message,
            &FE::new_random().to_big_int(),
        ]);
        let r_fe: FE = ECScalar::from(&r);
        let g: GE = ECPoint::generator();
        let eph_key_pair = KeyPair {
            public_key: g * &r_fe,
            private_key: r_fe,
        };
        EphKey { eph_key_pair }
    }
    //signing steps 2,3
    // we treat S as a list of public keys and compute a sum.
    pub fn compute_joint_comm_e(
        mut pub_key_vec: Vec<GE>,
        mut eph_pub_key_vec: Vec<GE>,
        message: &BigInt,
    ) -> (GE, GE, FE) {
        let first_pub_key = pub_key_vec.remove(0);
        let sum_pub = pub_key_vec
            .iter()
            .fold(first_pub_key, |acc, x| acc.add_point(&x.get_element()));
        let first_eph_pub_key = eph_pub_key_vec.remove(0);
        let sum_pub_eph = eph_pub_key_vec
            .iter()
            .fold(first_eph_pub_key, |acc, x| acc.add_point(&x.get_element()));
        //TODO: maybe there is a better way?
        let m_fe: FE = ECScalar::from(&message);
        let base_point: GE = GE::generator();
        let m_ge = base_point.scalar_mul(&m_fe.get_element());
        let e = multisig::hash_4(&[&sum_pub_eph, &m_ge, &sum_pub]);
        (sum_pub, sum_pub_eph, e)
    }

    pub fn partial_sign(&self, local_keys: &ExpendedKeyPair, es: FE) -> FE {
        es * &local_keys.expended_private_key.private_key + &self.eph_key_pair.private_key
    }

    pub fn add_signature_parts(sig_vec: Vec<FE>) -> FE {
        let mut sig_vec_c = sig_vec;
        let first_sig = sig_vec_c.remove(0);

        sig_vec_c
            .iter()
            .fold(first_sig, |acc, x| acc.add(&x.get_element()))
    }
}

pub struct Signature {
    X: GE,
    y: FE,
}

impl Signature {
    pub fn set_signature(X: &GE, y: &FE) -> Signature {
        Signature {
            X: X.clone(),
            y: y.clone(),
        }
    }
}

mod test;
