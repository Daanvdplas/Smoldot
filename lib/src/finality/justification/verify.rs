// Smoldot
// Copyright (C) 2019-2022  Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later WITH Classpath-exception-2.0

// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>.

use crate::finality::justification::decode;

use alloc::vec::Vec;
use core::{cmp, iter, mem};
use rand_chacha::{
    rand_core::{RngCore as _, SeedableRng as _},
    ChaCha20Rng,
};

/// Configuration for a justification verification process.
#[derive(Debug)]
pub struct Config<'a, I> {
    /// Justification to verify.
    pub justification: decode::GrandpaJustificationRef<'a>,

    pub block_number_bytes: usize,

    // TODO: document
    pub authorities_set_id: u64,

    /// List of authorities that are allowed to emit pre-commits for the block referred to by
    /// the justification. Must implement `Iterator<Item = &[u8]>`, where each item is
    /// the public key of an authority.
    pub authorities_list: I,

    /// Seed for a PRNG used for various purposes during the verification.
    ///
    /// > **Note**: The verification is nonetheless deterministic.
    pub randomness_seed: [u8; 32],
}

/// Verifies that a justification is valid.
pub fn verify<'a>(config: Config<impl Iterator<Item = &'a [u8]>>) -> Result<(), Error> {
    let num_precommits = config.justification.precommits.iter().count();

    let mut randomness = ChaCha20Rng::from_seed(config.randomness_seed);

    // Collect the authorities in a set in order to be able to determine with a low complexity
    // whether a public key is an authority.
    // For each authority, contains a boolean indicating whether the authority has been seen
    // before in the list of pre-commits.
    let mut authorities_list = {
        let mut list = hashbrown::HashMap::<&[u8], _, _>::with_capacity_and_hasher(
            0,
            crate::util::SipHasherBuild::new({
                let mut seed = [0; 16];
                randomness.fill_bytes(&mut seed);
                seed
            }),
        );
        for authority in config.authorities_list {
            list.insert(authority, false);
        }
        list
    };

    // Check that justification contains a number of signatures equal to at least 2/3rd of the
    // number of authorities.
    // Duplicate signatures are checked below.
    // The logic of the check is `actual >= (expected * 2 / 3) + 1`.
    if num_precommits < (authorities_list.len() * 2 / 3) + 1 {
        return Err(Error::NotEnoughSignatures);
    }

    // Verifying all the signatures together brings better performances than verifying them one
    // by one.
    // Note that batched ed25519 verification has some issues. The code below uses a special
    // flavour of ed25519 where ambiguities are removed.
    // See https://docs.rs/ed25519-zebra/2.2.0/ed25519_zebra/batch/index.html and
    // https://github.com/zcash/zips/blob/master/zip-0215.rst
    let mut batch = ed25519_zebra::batch::Verifier::new();

    for precommit in config.justification.precommits.iter() {
        match authorities_list.entry(precommit.authority_public_key) {
            hashbrown::hash_map::Entry::Occupied(mut entry) => {
                if entry.insert(true) {
                    return Err(Error::DuplicateSignature(*precommit.authority_public_key));
                }
            }
            hashbrown::hash_map::Entry::Vacant(_) => {
                return Err(Error::NotAuthority(*precommit.authority_public_key))
            }
        }

        // TODO: must check signed block ancestry using `votes_ancestries`

        let mut msg = Vec::with_capacity(1 + 32 + 4 + 8 + 8);
        msg.push(1u8); // This `1` indicates which kind of message is being signed.
        msg.extend_from_slice(&precommit.target_hash[..]);
        // The message contains the little endian block number. While simple in concept,
        // in reality it is more complicated because we don't know the number of bytes of
        // this block number at compile time. We thus copy as many bytes as appropriate and
        // pad with 0s if necessary.
        msg.extend_from_slice(
            &precommit.target_number.to_le_bytes()[..cmp::min(
                mem::size_of_val(&precommit.target_number),
                config.block_number_bytes,
            )],
        );
        msg.extend(
            iter::repeat(0).take(
                config
                    .block_number_bytes
                    .saturating_sub(mem::size_of_val(&precommit.target_number)),
            ),
        );
        msg.extend_from_slice(&u64::to_le_bytes(config.justification.round)[..]);
        msg.extend_from_slice(&u64::to_le_bytes(config.authorities_set_id)[..]);
        debug_assert_eq!(msg.len(), msg.capacity());

        batch.queue(ed25519_zebra::batch::Item::from((
            ed25519_zebra::VerificationKeyBytes::from(*precommit.authority_public_key),
            ed25519_zebra::Signature::from(*precommit.signature),
            &msg,
        )));
    }

    // Actual signatures verification performed here.
    batch
        .verify(&mut randomness)
        .map_err(|_| Error::BadSignature)?;

    // TODO: must check that votes_ancestries doesn't contain any unused entry
    // TODO: there's also a "ghost" thing?

    Ok(())
}

/// Error that can happen while verifying a justification.
#[derive(Debug, derive_more::Display)]
pub enum Error {
    /// One of the public keys is invalid.
    BadPublicKey,
    /// One of the signatures can't be verified.
    BadSignature,
    /// One authority has produced two signatures.
    #[display(fmt = "One authority has produced two signatures")]
    DuplicateSignature([u8; 32]),
    /// One of the public keys isn't in the list of authorities.
    #[display(fmt = "One of the public keys isn't in the list of authorities")]
    NotAuthority([u8; 32]),
    /// Justification doesn't contain enough authorities signatures to be valid.
    NotEnoughSignatures,
}
