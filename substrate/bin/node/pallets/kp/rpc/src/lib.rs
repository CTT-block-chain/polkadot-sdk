use std::{convert::TryInto, sync::Arc};

use codec::{Codec, Decode};
use jsonrpsee::{
	core::{Error as JsonRpseeError, RpcResult},
	proc_macros::rpc,
	types::error::{CallError, ErrorCode, ErrorObject},
};
use primitives::{AuthAccountId, Balance, BlockNumber, PowerSize};
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_core::Bytes;
use sp_rpc::number::NumberOrHex;
use sp_runtime::traits::{Block as BlockT, MaybeDisplay};

pub use pallet_kp_rpc_runtime_api::KpApi as KpRuntimeRpcApi;

#[rpc(client, server)]
pub trait KpApi<BlockHash, AccountId, Balance, BlockNumber> {
	#[method(name = "kp_totalPower")]
	fn total_power(&self, at: Option<BlockHash>) -> RpcResult<PowerSize>;
}

pub struct Kp<C, M> {
	// If you have more generics, no need to SumStorage<C, M, N, P, ...>
	// just use a tuple like SumStorage<C, (M, N, P, ...)>
	client: Arc<C>,
	_marker: std::marker::PhantomData<M>,
}

impl<C, M> Kp<C, M> {
	/// Create new `Kp` instance with the given reference to the client.
	pub fn new(client: Arc<C>) -> Self {
		Self { client, _marker: Default::default() }
	}
}

pub enum Error {
	/// The transaction was not decodable.
	DecodeError,
	/// The call to runtime failed.
	RuntimeError,
}

impl From<Error> for i32 {
	fn from(e: Error) -> i32 {
		match e {
			Error::RuntimeError => 1,
			Error::DecodeError => 2,
		}
	}
}

impl<C, Block> KpApiServer<<Block as BlockT>::Hash, AuthAccountId, Balance, BlockNumber>
	for Kp<C, Block>
where
	Block: BlockT,
	C: Send + Sync + 'static,
	C: ProvideRuntimeApi<Block>,
	C: HeaderBackend<Block>,
	C::Api: KpRuntimeRpcApi<Block, AuthAccountId, Balance, BlockNumber>,
{
	fn total_power(&self, at: Option<<Block as BlockT>::Hash>) -> RpcResult<PowerSize> {
		let api = self.client.runtime_api();
		let at_hash = at.unwrap_or_else(|| self.client.info().best_hash);

		fn map_err(error: impl ToString, desc: &'static str) -> CallError {
			CallError::Custom(ErrorObject::owned(
				Error::RuntimeError.into(),
				desc,
				Some(error.to_string()),
			))
		}

		let runtime_api_result = api.total_power(at_hash);
		Ok(runtime_api_result.map_err(|e| map_err(e, "Unable to query total power."))?)
	}
}
