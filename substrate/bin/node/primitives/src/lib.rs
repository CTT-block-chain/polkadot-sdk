// This file is part of Substrate.

// Copyright (C) Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Low-level types used throughout the Substrate code.

#![warn(missing_docs)]
#![cfg_attr(not(feature = "std"), no_std)]

use sp_runtime::{
	generic,
	traits::{BlakeTwo256, IdentifyAccount, Verify},
	DispatchResult, MultiSignature, OpaqueExtrinsic,
};

use sp_std::prelude::*;

/// An index to a block.
pub type BlockNumber = u32;

/// Alias to 512-bit hash when used in the context of a transaction signature on the chain.
pub type Signature = MultiSignature;

/// Some way of identifying an account on the chain. We intentionally make it equivalent
/// to the public key of our transaction signing scheme.
pub type AccountId = <<Signature as Verify>::Signer as IdentifyAccount>::AccountId;

/// The type for looking up accounts. We don't expect more than 4 billion of them.
pub type AccountIndex = u32;

/// Balance of an account.
pub type Balance = u128;

/// Type used for expressing timestamp.
pub type Moment = u64;

/// Index of a transaction in the chain.
pub type Nonce = u32;

/// A hash of some data used by the chain.
pub type Hash = sp_core::H256;

/// A timestamp: milliseconds since the unix epoch.
/// `u64` is enough to represent a duration of half a billion years, when the
/// time scale is milliseconds.
pub type Timestamp = u64;

/// Digest item type.
pub type DigestItem = generic::DigestItem;
/// Header type.
pub type Header = generic::Header<BlockNumber, BlakeTwo256>;
/// Block type.
pub type Block = generic::Block<Header, OpaqueExtrinsic>;
/// Block ID.
pub type BlockId = generic::BlockId<Block>;

/// Used for outside interface
pub type AuthAccountId = <<MultiSignature as Verify>::Signer as IdentifyAccount>::AccountId;

/// CTT Membership trait which implemented by pallet_members
pub trait Membership<AccountId, Hash, Balance> {
	fn is_platform(who: &AccountId, app_id: u32) -> bool;
	fn is_expert(who: &AccountId, app_id: u32, model_id: &Vec<u8>) -> bool;
	fn is_app_admin(who: &AccountId, app_id: u32) -> bool;
	fn is_investor(who: &AccountId) -> bool;
	fn is_finance_member(who: &AccountId) -> bool;
	fn set_model_creator(key: &Hash, creator: &AccountId, is_give_benefit: bool) -> Balance;
	fn transfer_model_owner(key: &Hash, new_owner: &AccountId);
	fn is_model_creator(who: &AccountId, app_id: u32, model_id: &Vec<u8>) -> bool;
	fn config_app_admin(who: &AccountId, app_id: u32);
	fn config_app_key(who: &AccountId, app_id: u32);
	fn config_app_setting(app_id: u32, rate: u32, name: Vec<u8>, stake: Balance);
	fn get_app_setting(app_id: u32) -> (u32, Vec<u8>, Balance);
	fn is_valid_app(app_id: u32) -> bool;
	fn is_valid_app_key(app_id: u32, app_key: &AccountId) -> bool;
	fn valid_finance_members() -> Vec<AccountId>;
	fn slash_finance_member(
		member: &AccountId,
		receiver: &AccountId,
		amount: Balance,
	) -> DispatchResult;
}
