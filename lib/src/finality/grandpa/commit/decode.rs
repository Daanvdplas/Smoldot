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

use alloc::vec::Vec;

/// Attempt to decode the given SCALE-encoded Grandpa commit.
pub fn decode_grandpa_commit(
    scale_encoded: &[u8],
    block_number_bytes: usize,
) -> Result<CommitMessageRef, Error> {
    match nom::combinator::all_consuming(commit_message(block_number_bytes))(scale_encoded) {
        Ok((_, commit)) => Ok(commit),
        Err(err) => Err(Error(err)),
    }
}

/// Attempt to decode the given SCALE-encoded commit.
///
/// Contrary to [`decode_grandpa_commit`], doesn't return an error if the slice is too long but
/// returns the remainder.
pub fn decode_partial_grandpa_commit(
    scale_encoded: &[u8],
    block_number_bytes: usize,
) -> Result<(CommitMessageRef, &[u8]), Error> {
    match commit_message(block_number_bytes)(scale_encoded) {
        Ok((remainder, commit)) => Ok((commit, remainder)),
        Err(err) => Err(Error(err)),
    }
}

/// Error potentially returned by [`decode_grandpa_commit`].
#[derive(Debug, derive_more::Display)]
pub struct Error<'a>(nom::Err<nom::error::Error<&'a [u8]>>);

// TODO: document and explain
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitMessageRef<'a> {
    pub round_number: u64,
    pub set_id: u64,
    pub message: CompactCommitRef<'a>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompactCommitRef<'a> {
    pub target_hash: &'a [u8; 32],
    pub target_number: u64,
    // TODO: don't use Vec
    pub precommits: Vec<UnsignedPrecommitRef<'a>>,

    /// List of Ed25519 signatures and public keys.
    // TODO: refactor
    // TODO: don't use Vec
    pub auth_data: Vec<(&'a [u8; 64], &'a [u8; 32])>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnsignedPrecommitRef<'a> {
    pub target_hash: &'a [u8; 32],
    pub target_number: u64,
}

fn commit_message<'a>(
    block_number_bytes: usize,
) -> impl FnMut(&'a [u8]) -> nom::IResult<&[u8], CommitMessageRef> {
    nom::error::context(
        "commit_message",
        nom::combinator::map(
            nom::sequence::tuple((
                nom::number::streaming::le_u64,
                nom::number::streaming::le_u64,
                compact_commit(block_number_bytes),
            )),
            |(round_number, set_id, message)| CommitMessageRef {
                round_number,
                set_id,
                message,
            },
        ),
    )
}

fn compact_commit<'a>(
    block_number_bytes: usize,
) -> impl FnMut(&'a [u8]) -> nom::IResult<&[u8], CompactCommitRef> {
    nom::error::context(
        "compact_commit",
        nom::combinator::map(
            nom::sequence::tuple((
                nom::bytes::streaming::take(32u32),
                crate::util::nom_varsize_number_decode_u64(block_number_bytes),
                nom::combinator::flat_map(crate::util::nom_scale_compact_usize, move |num_elems| {
                    nom::multi::many_m_n(
                        num_elems,
                        num_elems,
                        unsigned_precommit(block_number_bytes),
                    )
                }),
                nom::combinator::flat_map(crate::util::nom_scale_compact_usize, |num_elems| {
                    nom::multi::many_m_n(
                        num_elems,
                        num_elems,
                        nom::combinator::map(
                            nom::sequence::tuple((
                                nom::bytes::streaming::take(64u32),
                                nom::bytes::streaming::take(32u32),
                            )),
                            |(sig, pubkey)| {
                                (
                                    <&[u8; 64]>::try_from(sig).unwrap(),
                                    <&[u8; 32]>::try_from(pubkey).unwrap(),
                                )
                            },
                        ),
                    )
                }),
            )),
            |(target_hash, target_number, precommits, auth_data)| CompactCommitRef {
                target_hash: <&[u8; 32]>::try_from(target_hash).unwrap(),
                target_number,
                precommits,
                auth_data,
            },
        ),
    )
}

fn unsigned_precommit<'a>(
    block_number_bytes: usize,
) -> impl FnMut(&'a [u8]) -> nom::IResult<&[u8], UnsignedPrecommitRef> {
    nom::error::context(
        "unsigned_precommit",
        nom::combinator::map(
            nom::sequence::tuple((
                nom::bytes::streaming::take(32u32),
                crate::util::nom_varsize_number_decode_u64(block_number_bytes),
            )),
            |(target_hash, target_number)| UnsignedPrecommitRef {
                target_hash: <&[u8; 32]>::try_from(target_hash).unwrap(),
                target_number,
            },
        ),
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn basic_decode_commit() {
        let actual = super::decode_grandpa_commit(
            &[
                85, 14, 0, 0, 0, 0, 0, 0, 162, 13, 0, 0, 0, 0, 0, 0, 182, 68, 115, 35, 15, 201,
                152, 195, 12, 181, 59, 244, 231, 124, 34, 248, 98, 253, 4, 180, 158, 70, 161, 84,
                76, 118, 151, 68, 101, 104, 187, 82, 49, 231, 77, 0, 28, 182, 68, 115, 35, 15, 201,
                152, 195, 12, 181, 59, 244, 231, 124, 34, 248, 98, 253, 4, 180, 158, 70, 161, 84,
                76, 118, 151, 68, 101, 104, 187, 82, 49, 231, 77, 0, 182, 68, 115, 35, 15, 201,
                152, 195, 12, 181, 59, 244, 231, 124, 34, 248, 98, 253, 4, 180, 158, 70, 161, 84,
                76, 118, 151, 68, 101, 104, 187, 82, 49, 231, 77, 0, 182, 68, 115, 35, 15, 201,
                152, 195, 12, 181, 59, 244, 231, 124, 34, 248, 98, 253, 4, 180, 158, 70, 161, 84,
                76, 118, 151, 68, 101, 104, 187, 82, 49, 231, 77, 0, 182, 68, 115, 35, 15, 201,
                152, 195, 12, 181, 59, 244, 231, 124, 34, 248, 98, 253, 4, 180, 158, 70, 161, 84,
                76, 118, 151, 68, 101, 104, 187, 82, 49, 231, 77, 0, 182, 68, 115, 35, 15, 201,
                152, 195, 12, 181, 59, 244, 231, 124, 34, 248, 98, 253, 4, 180, 158, 70, 161, 84,
                76, 118, 151, 68, 101, 104, 187, 82, 49, 231, 77, 0, 182, 68, 115, 35, 15, 201,
                152, 195, 12, 181, 59, 244, 231, 124, 34, 248, 98, 253, 4, 180, 158, 70, 161, 84,
                76, 118, 151, 68, 101, 104, 187, 82, 49, 231, 77, 0, 182, 68, 115, 35, 15, 201,
                152, 195, 12, 181, 59, 244, 231, 124, 34, 248, 98, 253, 4, 180, 158, 70, 161, 84,
                76, 118, 151, 68, 101, 104, 187, 82, 49, 231, 77, 0, 28, 189, 185, 216, 33, 163,
                12, 201, 104, 162, 255, 11, 241, 156, 90, 244, 205, 251, 44, 45, 139, 129, 117,
                178, 85, 129, 78, 58, 255, 76, 232, 199, 85, 236, 30, 227, 87, 50, 34, 22, 27, 241,
                6, 33, 137, 55, 5, 190, 36, 122, 61, 112, 51, 99, 34, 119, 46, 185, 156, 188, 133,
                140, 103, 33, 10, 45, 154, 173, 12, 30, 12, 25, 95, 195, 198, 235, 98, 29, 248, 44,
                121, 73, 203, 132, 51, 196, 138, 65, 42, 3, 49, 169, 182, 129, 146, 242, 193, 228,
                217, 26, 9, 233, 239, 30, 213, 103, 10, 33, 27, 44, 13, 178, 236, 216, 167, 190, 9,
                123, 151, 143, 1, 199, 58, 77, 121, 122, 215, 22, 19, 238, 190, 216, 8, 62, 6, 216,
                37, 197, 124, 141, 51, 196, 205, 205, 193, 24, 86, 246, 60, 16, 139, 66, 51, 93,
                168, 159, 147, 77, 90, 91, 8, 64, 14, 252, 119, 77, 211, 141, 23, 18, 115, 222, 3,
                2, 22, 42, 105, 85, 176, 71, 232, 230, 141, 12, 9, 124, 205, 194, 191, 90, 47, 202,
                233, 218, 161, 80, 55, 8, 134, 223, 202, 4, 137, 45, 10, 71, 90, 162, 252, 99, 19,
                252, 17, 175, 5, 75, 208, 81, 0, 96, 218, 5, 89, 250, 183, 161, 188, 227, 62, 107,
                34, 63, 155, 28, 176, 141, 174, 113, 162, 229, 148, 55, 39, 65, 36, 97, 159, 198,
                238, 222, 34, 76, 187, 40, 19, 109, 1, 67, 146, 40, 75, 194, 208, 80, 208, 221,
                175, 151, 239, 239, 127, 65, 39, 237, 145, 130, 36, 154, 135, 68, 105, 52, 102, 49,
                62, 137, 34, 187, 159, 55, 157, 88, 195, 49, 116, 72, 11, 37, 132, 176, 74, 69, 60,
                157, 67, 36, 156, 165, 71, 164, 86, 220, 240, 241, 13, 40, 125, 79, 147, 27, 56,
                254, 198, 231, 108, 187, 214, 187, 98, 229, 123, 116, 160, 126, 192, 98, 132, 247,
                206, 70, 228, 175, 152, 217, 252, 4, 109, 98, 24, 90, 117, 184, 11, 107, 32, 186,
                217, 155, 44, 253, 198, 120, 175, 170, 229, 66, 122, 141, 158, 75, 68, 108, 104,
                182, 223, 91, 126, 210, 38, 84, 143, 10, 142, 225, 77, 169, 12, 215, 222, 158, 85,
                4, 111, 196, 47, 56, 147, 93, 1, 202, 247, 137, 115, 30, 127, 94, 191, 31, 223,
                162, 16, 73, 219, 118, 52, 40, 255, 191, 183, 70, 132, 115, 91, 214, 191, 156, 189,
                203, 208, 152, 165, 115, 64, 123, 209, 153, 80, 44, 134, 143, 188, 140, 168, 162,
                134, 178, 192, 122, 10, 137, 41, 133, 127, 72, 223, 16, 65, 170, 114, 53, 173, 180,
                59, 208, 190, 54, 96, 123, 199, 137, 214, 115, 240, 73, 87, 253, 137, 81, 36, 66,
                175, 76, 40, 52, 216, 110, 234, 219, 158, 208, 142, 85, 168, 43, 164, 19, 154, 21,
                125, 174, 153, 165, 45, 54, 100, 36, 196, 46, 95, 64, 192, 178, 156, 16, 112, 5,
                237, 207, 113, 132, 125, 148, 34, 132, 105, 148, 216, 148, 182, 33, 74, 215, 161,
                252, 44, 24, 67, 77, 87, 6, 94, 109, 38, 64, 10, 195, 28, 194, 169, 175, 7, 98,
                210, 151, 4, 221, 136, 161, 204, 171, 251, 101, 63, 21, 245, 84, 189, 77, 59, 75,
                136, 44, 17, 217, 119, 206, 191, 191, 137, 127, 81, 55, 208, 225, 33, 209, 59, 83,
                121, 234, 160, 191, 38, 82, 1, 102, 178, 140, 58, 20, 131, 206, 37, 148, 106, 135,
                149, 74, 57, 27, 84, 215, 0, 47, 68, 1, 8, 139, 183, 125, 169, 4, 165, 168, 86,
                218, 178, 95, 157, 185, 64, 45, 211, 221, 151, 205, 240, 69, 133, 200, 15, 213,
                170, 162, 127, 93, 224, 36, 86, 116, 44, 42, 22, 255, 144, 193, 35, 175, 145, 62,
                184, 67, 143, 199, 253, 37, 115, 23, 154, 213, 141, 122, 105,
            ],
            4,
        )
        .unwrap();

        let expected = super::CommitMessageRef {
            round_number: 3669,
            set_id: 3490,
            message: super::CompactCommitRef {
                target_hash: &[
                    182, 68, 115, 35, 15, 201, 152, 195, 12, 181, 59, 244, 231, 124, 34, 248, 98,
                    253, 4, 180, 158, 70, 161, 84, 76, 118, 151, 68, 101, 104, 187, 82,
                ],
                target_number: 5_105_457,
                precommits: vec![
                    super::UnsignedPrecommitRef {
                        target_hash: &[
                            182, 68, 115, 35, 15, 201, 152, 195, 12, 181, 59, 244, 231, 124, 34,
                            248, 98, 253, 4, 180, 158, 70, 161, 84, 76, 118, 151, 68, 101, 104,
                            187, 82,
                        ],
                        target_number: 5_105_457,
                    },
                    super::UnsignedPrecommitRef {
                        target_hash: &[
                            182, 68, 115, 35, 15, 201, 152, 195, 12, 181, 59, 244, 231, 124, 34,
                            248, 98, 253, 4, 180, 158, 70, 161, 84, 76, 118, 151, 68, 101, 104,
                            187, 82,
                        ],
                        target_number: 5_105_457,
                    },
                    super::UnsignedPrecommitRef {
                        target_hash: &[
                            182, 68, 115, 35, 15, 201, 152, 195, 12, 181, 59, 244, 231, 124, 34,
                            248, 98, 253, 4, 180, 158, 70, 161, 84, 76, 118, 151, 68, 101, 104,
                            187, 82,
                        ],
                        target_number: 5_105_457,
                    },
                    super::UnsignedPrecommitRef {
                        target_hash: &[
                            182, 68, 115, 35, 15, 201, 152, 195, 12, 181, 59, 244, 231, 124, 34,
                            248, 98, 253, 4, 180, 158, 70, 161, 84, 76, 118, 151, 68, 101, 104,
                            187, 82,
                        ],
                        target_number: 5_105_457,
                    },
                    super::UnsignedPrecommitRef {
                        target_hash: &[
                            182, 68, 115, 35, 15, 201, 152, 195, 12, 181, 59, 244, 231, 124, 34,
                            248, 98, 253, 4, 180, 158, 70, 161, 84, 76, 118, 151, 68, 101, 104,
                            187, 82,
                        ],
                        target_number: 5_105_457,
                    },
                    super::UnsignedPrecommitRef {
                        target_hash: &[
                            182, 68, 115, 35, 15, 201, 152, 195, 12, 181, 59, 244, 231, 124, 34,
                            248, 98, 253, 4, 180, 158, 70, 161, 84, 76, 118, 151, 68, 101, 104,
                            187, 82,
                        ],
                        target_number: 5_105_457,
                    },
                    super::UnsignedPrecommitRef {
                        target_hash: &[
                            182, 68, 115, 35, 15, 201, 152, 195, 12, 181, 59, 244, 231, 124, 34,
                            248, 98, 253, 4, 180, 158, 70, 161, 84, 76, 118, 151, 68, 101, 104,
                            187, 82,
                        ],
                        target_number: 5_105_457,
                    },
                ],
                auth_data: vec![
                    (
                        &[
                            189, 185, 216, 33, 163, 12, 201, 104, 162, 255, 11, 241, 156, 90, 244,
                            205, 251, 44, 45, 139, 129, 117, 178, 85, 129, 78, 58, 255, 76, 232,
                            199, 85, 236, 30, 227, 87, 50, 34, 22, 27, 241, 6, 33, 137, 55, 5, 190,
                            36, 122, 61, 112, 51, 99, 34, 119, 46, 185, 156, 188, 133, 140, 103,
                            33, 10,
                        ],
                        &[
                            45, 154, 173, 12, 30, 12, 25, 95, 195, 198, 235, 98, 29, 248, 44, 121,
                            73, 203, 132, 51, 196, 138, 65, 42, 3, 49, 169, 182, 129, 146, 242,
                            193,
                        ],
                    ),
                    (
                        &[
                            228, 217, 26, 9, 233, 239, 30, 213, 103, 10, 33, 27, 44, 13, 178, 236,
                            216, 167, 190, 9, 123, 151, 143, 1, 199, 58, 77, 121, 122, 215, 22, 19,
                            238, 190, 216, 8, 62, 6, 216, 37, 197, 124, 141, 51, 196, 205, 205,
                            193, 24, 86, 246, 60, 16, 139, 66, 51, 93, 168, 159, 147, 77, 90, 91,
                            8,
                        ],
                        &[
                            64, 14, 252, 119, 77, 211, 141, 23, 18, 115, 222, 3, 2, 22, 42, 105,
                            85, 176, 71, 232, 230, 141, 12, 9, 124, 205, 194, 191, 90, 47, 202,
                            233,
                        ],
                    ),
                    (
                        &[
                            218, 161, 80, 55, 8, 134, 223, 202, 4, 137, 45, 10, 71, 90, 162, 252,
                            99, 19, 252, 17, 175, 5, 75, 208, 81, 0, 96, 218, 5, 89, 250, 183, 161,
                            188, 227, 62, 107, 34, 63, 155, 28, 176, 141, 174, 113, 162, 229, 148,
                            55, 39, 65, 36, 97, 159, 198, 238, 222, 34, 76, 187, 40, 19, 109, 1,
                        ],
                        &[
                            67, 146, 40, 75, 194, 208, 80, 208, 221, 175, 151, 239, 239, 127, 65,
                            39, 237, 145, 130, 36, 154, 135, 68, 105, 52, 102, 49, 62, 137, 34,
                            187, 159,
                        ],
                    ),
                    (
                        &[
                            55, 157, 88, 195, 49, 116, 72, 11, 37, 132, 176, 74, 69, 60, 157, 67,
                            36, 156, 165, 71, 164, 86, 220, 240, 241, 13, 40, 125, 79, 147, 27, 56,
                            254, 198, 231, 108, 187, 214, 187, 98, 229, 123, 116, 160, 126, 192,
                            98, 132, 247, 206, 70, 228, 175, 152, 217, 252, 4, 109, 98, 24, 90,
                            117, 184, 11,
                        ],
                        &[
                            107, 32, 186, 217, 155, 44, 253, 198, 120, 175, 170, 229, 66, 122, 141,
                            158, 75, 68, 108, 104, 182, 223, 91, 126, 210, 38, 84, 143, 10, 142,
                            225, 77,
                        ],
                    ),
                    (
                        &[
                            169, 12, 215, 222, 158, 85, 4, 111, 196, 47, 56, 147, 93, 1, 202, 247,
                            137, 115, 30, 127, 94, 191, 31, 223, 162, 16, 73, 219, 118, 52, 40,
                            255, 191, 183, 70, 132, 115, 91, 214, 191, 156, 189, 203, 208, 152,
                            165, 115, 64, 123, 209, 153, 80, 44, 134, 143, 188, 140, 168, 162, 134,
                            178, 192, 122, 10,
                        ],
                        &[
                            137, 41, 133, 127, 72, 223, 16, 65, 170, 114, 53, 173, 180, 59, 208,
                            190, 54, 96, 123, 199, 137, 214, 115, 240, 73, 87, 253, 137, 81, 36,
                            66, 175,
                        ],
                    ),
                    (
                        &[
                            76, 40, 52, 216, 110, 234, 219, 158, 208, 142, 85, 168, 43, 164, 19,
                            154, 21, 125, 174, 153, 165, 45, 54, 100, 36, 196, 46, 95, 64, 192,
                            178, 156, 16, 112, 5, 237, 207, 113, 132, 125, 148, 34, 132, 105, 148,
                            216, 148, 182, 33, 74, 215, 161, 252, 44, 24, 67, 77, 87, 6, 94, 109,
                            38, 64, 10,
                        ],
                        &[
                            195, 28, 194, 169, 175, 7, 98, 210, 151, 4, 221, 136, 161, 204, 171,
                            251, 101, 63, 21, 245, 84, 189, 77, 59, 75, 136, 44, 17, 217, 119, 206,
                            191,
                        ],
                    ),
                    (
                        &[
                            191, 137, 127, 81, 55, 208, 225, 33, 209, 59, 83, 121, 234, 160, 191,
                            38, 82, 1, 102, 178, 140, 58, 20, 131, 206, 37, 148, 106, 135, 149, 74,
                            57, 27, 84, 215, 0, 47, 68, 1, 8, 139, 183, 125, 169, 4, 165, 168, 86,
                            218, 178, 95, 157, 185, 64, 45, 211, 221, 151, 205, 240, 69, 133, 200,
                            15,
                        ],
                        &[
                            213, 170, 162, 127, 93, 224, 36, 86, 116, 44, 42, 22, 255, 144, 193,
                            35, 175, 145, 62, 184, 67, 143, 199, 253, 37, 115, 23, 154, 213, 141,
                            122, 105,
                        ],
                    ),
                ],
            },
        };

        assert_eq!(actual, expected);
    }
}
