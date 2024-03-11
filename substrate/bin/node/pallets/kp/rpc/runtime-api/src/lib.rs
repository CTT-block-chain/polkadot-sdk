#![cfg_attr(not(feature = "std"), no_std)]

use codec::Codec;
use primitives::PowerSize;
use sp_runtime::traits::MaybeDisplay;

sp_api::decl_runtime_apis! {
	#[api_version(4)]
	pub trait KpApi<AccountId, Balance, BlockNumber> where AccountId: Codec, Balance: Codec, BlockNumber: Codec,
	{
		fn total_power() -> PowerSize;
	}
}
