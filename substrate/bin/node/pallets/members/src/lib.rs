#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
//use sp_std::vec::Vec;
use frame_support::{
	dispatch::PostDispatchInfo,
	ensure, fail,
	traits::{Currency, ExistenceRequirement::KeepAlive, Get, ReservableCurrency},
	DefaultNoBound,
};

use node_primitives::{AuthAccountId, Membership};
use scale_info::TypeInfo;
use sp_core::sr25519;
use sp_runtime::{
	traits::StaticLookup,
	traits::{TrailingZeroInput, Verify},
	MultiSignature, RuntimeDebug,
};
use sp_std::cmp::min;
use sp_std::prelude::*;

/// Edit this file to define custom logic or remove it if it is not needed.
/// Learn more about FRAME and the core library of Substrate FRAME pallets:
/// <https://docs.substrate.io/reference/frame-pallets/>
pub use pallet::*;

type BalanceOf<T> =
	<<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

const LOG_TARGET: &str = "ctt::members";

#[derive(Encode, Decode, Clone, RuntimeDebug, TypeInfo)]
#[scale_info(skip_type_params(T))]
pub struct AppData<T: Config> {
	name: Vec<u8>,
	return_rate: u32,
	stake: BalanceOf<T>,
}

impl<T: Config> Default for AppData<T> {
	fn default() -> Self {
		AppData { name: Vec::new(), return_rate: 0, stake: 0u32.into() }
	}
}

#[derive(Encode, Decode, Clone, RuntimeDebug, TypeInfo)]
#[scale_info(skip_type_params(T))]
pub struct StableExchangeData<T: Config> {
	receiver: T::AccountId,
	amount: BalanceOf<T>,
	redeemed: bool,
}

impl<T: Config> Default for StableExchangeData<T> {
	fn default() -> Self {
		StableExchangeData {
			receiver: T::AccountId::decode(&mut TrailingZeroInput::zeroes()).unwrap(),
			amount: 0u32.into(),
			redeemed: false,
		}
	}
}

#[derive(Encode, Decode, Clone, Default, PartialEq, RuntimeDebug)]
pub struct ModelExpertAddMemberParams {
	app_id: u32,
	model_id: Vec<u8>,
	kpt_profit_rate: u32,
}

#[derive(Encode, Decode, Clone, Default, PartialEq, RuntimeDebug)]
pub struct ModelExpertDelMemberParams<Account> {
	app_id: u32,
	model_id: Vec<u8>,
	member: Account,
}

#[derive(Encode, Decode, Clone, PartialEq, RuntimeDebug)]
pub struct AppKeyManageParams<T: Config> {
	admin: AuthAccountId,
	app_id: u32,
	member: T::AccountId,
}

impl<T: Config> Default for AppKeyManageParams<T> {
	fn default() -> Self {
		AppKeyManageParams {
			admin: AuthAccountId::decode(&mut TrailingZeroInput::zeroes()).unwrap(),
			app_id: 0,
			member: T::AccountId::decode(&mut TrailingZeroInput::zeroes()).unwrap(),
		}
	}
}

#[derive(Encode, Decode, Clone, Default, PartialEq, RuntimeDebug)]
pub struct FinanceMemberParams<Account, Balance> {
	deposit: Balance,
	member: Account,
}

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	/// The current storage version.
	const STORAGE_VERSION: StorageVersion = StorageVersion::new(4);

	#[pallet::pallet]
	#[pallet::storage_version(STORAGE_VERSION)]
	#[pallet::without_storage_info]
	pub struct Pallet<T>(_);

	/// Configure the pallet by specifying the parameters and types on which it depends.
	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// Because this pallet emits events, it depends on the runtime's definition of an event.
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// The runtime's definition of a Currency.
		type Currency: ReservableCurrency<Self::AccountId>;

		/// Minimal deposit for finance member
		type MinFinanceMemberDeposit: Get<BalanceOf<Self>>;
	}

	// The pallet's runtime storage items.
	// https://docs.substrate.io/main-docs/build/runtime-storage/
	#[pallet::storage]
	#[pallet::getter(fn finance_members)]
	pub(super) type FinanceMembers<T: Config> = StorageValue<_, Vec<T::AccountId>, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn finance_root)]
	pub(super) type FinanceRoot<T: Config> = StorageValue<_, Option<T::AccountId>, ValueQuery>;

	// Finance member deposit records
	#[pallet::storage]
	#[pallet::getter(fn finance_member_deposit)]
	pub(super) type FinanceMemberDeposit<T: Config> =
		StorageMap<_, Twox64Concat, T::AccountId, BalanceOf<T>, ValueQuery>;

	// Investor members, system level
	#[pallet::storage]
	#[pallet::getter(fn investor_members)]
	pub(super) type InvestorMembers<T: Config> = StorageValue<_, Vec<T::AccountId>, ValueQuery>;

	// app level admin members key is app_id
	#[pallet::storage]
	#[pallet::getter(fn app_admins)]
	pub(super) type AppAdmins<T: Config> =
		StorageMap<_, Twox64Concat, u32, Vec<T::AccountId>, ValueQuery>;

	// App ID => App Keys
	#[pallet::storage]
	#[pallet::getter(fn app_keys)]
	pub(super) type AppKeys<T: Config> =
		StorageMap<_, Twox64Concat, u32, Vec<T::AccountId>, ValueQuery>;

	// AppId => AppData
	#[pallet::storage]
	#[pallet::getter(fn app_data_map)]
	pub(super) type AppDataMap<T: Config> =
		StorageMap<_, Twox64Concat, u32, AppData<T>, ValueQuery>;

	// app level platform comment experts, key is app_id, managed by app_admins
	#[pallet::storage]
	#[pallet::getter(fn app_platform_expert_members)]
	pub(super) type AppPlatformExpertMembers<T: Config> =
		StorageMap<_, Twox64Concat, u32, Vec<T::AccountId>, ValueQuery>;

	// The set of model creators. Stored as a map, key is app_id & model id
	#[pallet::storage]
	#[pallet::getter(fn model_creators)]
	pub(super) type ModelCreators<T: Config> = StorageDoubleMap<
		_,
		Twox64Concat,
		u32,
		Twox64Concat,
		Vec<u8>,
		Option<T::AccountId>,
		ValueQuery,
	>;

	// Expert members, key is app_id & model id
	#[pallet::storage]
	#[pallet::getter(fn expert_members)]
	pub(super) type ExpertMembers<T: Config> = StorageDoubleMap<
		_,
		Twox64Concat,
		u32,
		Twox64Concat,
		Vec<u8>,
		Vec<T::AccountId>,
		ValueQuery,
	>;

	// Expert member profit rate, key is app_id & model id
	#[pallet::storage]
	#[pallet::getter(fn expert_member_profit_rate)]
	pub(super) type ExpertMemberProfitRate<T: Config> =
		StorageDoubleMap<_, Twox64Concat, u32, Twox64Concat, Vec<u8>, u32, ValueQuery>;

	// New account benefit records, app_id user_id -> u32 record first time user KPT drop
	#[pallet::storage]
	#[pallet::getter(fn new_account_benefit_records)]
	pub(super) type NewAccountBenefitRecords<T: Config> =
		StorageDoubleMap<_, Twox64Concat, u32, Twox64Concat, Vec<u8>, BalanceOf<T>, ValueQuery>;

	// app_id cash_receipt ->
	#[pallet::storage]
	#[pallet::getter(fn stable_exchange_records)]
	pub(super) type StableExchangeRecords<T: Config> = StorageDoubleMap<
		_,
		Twox64Concat,
		u32,
		Twox64Concat,
		Vec<u8>,
		StableExchangeData<T>,
		ValueQuery,
	>;

	// app_id stash account(for redeem receiver)
	#[pallet::storage]
	#[pallet::getter(fn app_redeem_account)]
	pub(super) type AppRedeemAccount<T: Config> =
		StorageMap<_, Twox64Concat, u32, Option<T::AccountId>, ValueQuery>;

	// Pallets use events to inform users when important changes are made.
	// https://docs.substrate.io/main-docs/build/events-errors/
	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Event documentation should end with an array that provides descriptive names for event
		/// parameters. [something, who]
		FinanceMemberStored { added_member: T::AccountId, who: T::AccountId },
	}

	// Errors inform users that something went wrong.
	#[pallet::error]
	pub enum Error<T> {
		/// Error names should be descriptive.
		NoneValue,
		/// Errors should have helpful documentation associated with them.
		StorageOverflow,
		AlreadyMember,
		/// Cannot give up membership because you are not currently a member
		NotMember,
		NotAppAdmin,
		NotAppIdentity,
		NotModelCreator,
		CallerNotFinanceMemeber,
		CallerNotFinanceRoot,
		MembersLenTooLow,
		BenefitAlreadyDropped,
		NotEnoughFund,
		StableExchangeReceiptExist,
		StableExchangeReceiptNotFound,
		StableRedeemRepeat,
		AppRedeemAcountNotSet,
		StableRedeemAccountNotMatch,
		SignVerifyError,
		AppIdInvalid,
		AuthIdentityNotAppAdmin,
		AppKeysLimitReached,
		AppKeysOnlyOne,
		FinanceMemberSizeOver,
		FinanceMemberDepositTooLow,
		DepositTooSmall,
	}

	#[pallet::genesis_config]
	#[derive(DefaultNoBound)]
	pub struct GenesisConfig<T: Config> {
		/// The root account of the finance controller which can control the finance members
		pub finance_root: Option<T::AccountId>,
	}

	// impl<T: Config> Default for GenesisConfig<T> {
	// 	fn default() -> Self {
	// 		// init with null account
	// 		Self { finance_root: Default::default() }
	// 	}
	// }

	#[pallet::genesis_build]
	impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
		fn build(&self) {
			<FinanceRoot<T>>::put(self.finance_root.clone());
			// init finance members with finance root
			<FinanceMembers<T>>::put(vec![self.finance_root.clone().unwrap()]);
		}
	}

	// Dispatchable functions allows users to interact with the pallet and invoke state changes.
	// These functions materialize as "extrinsics", which are often compared to transactions.
	// Dispatchable functions must be annotated with a weight and must return a DispatchResult.
	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// An example dispatchable that takes a singles value as a parameter, writes the value to
		/// storage and emits an event. This function must be dispatched by a signed extrinsic.
		#[pallet::call_index(0)]
		#[pallet::weight(0)]
		pub fn add_finance_member(
			origin: OriginFor<T>,
			member_account: T::AccountId,
		) -> DispatchResult {
			// Check that the extrinsic was signed and get the signer.
			// This function will return an error if the extrinsic is not signed.
			// https://docs.substrate.io/main-docs/build/origins/
			let who = ensure_signed(origin)?;

			// ensure who is finance root
			ensure!(
				<FinanceRoot<T>>::get().unwrap() == who,
				"Only finance root can add finance member"
			);

			// Update storage.
			<FinanceMembers<T>>::mutate(|members| {
				if !members.contains(&member_account) {
					members.push(member_account.clone());
				}
			});

			// Emit an event.
			Self::deposit_event(Event::FinanceMemberStored { added_member: member_account, who });
			// Return a successful DispatchResultWithPostInfo
			Ok(())
		}
	}

	impl<T: Config> Pallet<T> {
		fn convert_account(origin: &AuthAccountId) -> T::AccountId
		where
			<T as frame_system::Config>::AccountId: std::default::Default,
		{
			let tmp: [u8; 32] = origin.clone().into();
			T::AccountId::decode(&mut &tmp[..]).unwrap_or_default()
		}

		fn verify_sign(pub_key: &AuthAccountId, sign: sr25519::Signature, msg: &[u8]) -> bool {
			let ms: MultiSignature = sign.into();
			ms.verify(msg, &pub_key)
		}

		pub fn is_platform_expert(who: &T::AccountId, app_id: u32) -> bool {
			let members = <AppPlatformExpertMembers<T>>::get(app_id);
			match members.binary_search(who) {
				Ok(_) => true,
				Err(_) => false,
			}
		}

		pub fn is_model_expert(who: &T::AccountId, app_id: u32, model_id: &Vec<u8>) -> bool {
			let members = <ExpertMembers<T>>::get(app_id, model_id);
			match members.binary_search(who) {
				Ok(_) => true,
				Err(_) => false,
			}
		}

		pub fn is_investor(who: &T::AccountId) -> bool {
			let members = InvestorMembers::<T>::get();
			match members.binary_search(who) {
				Ok(_) => true,
				Err(_) => false,
			}
		}

		pub fn is_model_creator(who: &T::AccountId, app_id: u32, model_id: &Vec<u8>) -> bool {
			<ModelCreators<T>>::contains_key(app_id, model_id)
				&& <ModelCreators<T>>::get(app_id, model_id).unwrap() == *who
		}

		pub fn is_app_admin(who: &T::AccountId, app_id: u32) -> bool {
			let members = <AppAdmins<T>>::get(app_id);

			match members.binary_search(who) {
				// If the search succeeds, the caller is already a member, so just return
				Ok(_index) => true,
				// If the search fails, the caller is not a member, so just return
				Err(_) => false,
			}
		}

		pub fn is_app_identity(who: &T::AccountId, app_id: u32) -> bool {
			//let test = who.clone().encode().as_slice();
			let members = <AppKeys<T>>::get(app_id);

			match members.binary_search(who) {
				// If the search succeeds, the caller is already a member, so just return
				Ok(_index) => true,
				// If the search fails, the caller is not a member, so just return
				Err(_) => false,
			}
		}

		pub fn model_experts(app_id: u32, model_id: Vec<u8>) -> Vec<T::AccountId> {
			<ExpertMembers<T>>::get(app_id, &model_id)
		}

		pub fn model_add_expert(app_id: u32, model_id: &Vec<u8>, new_member: &T::AccountId) {
			let mut members = <ExpertMembers<T>>::get(app_id, model_id);

			match members.binary_search(new_member) {
				// If the search succeeds, the caller is already a member, so just return
				Ok(_) => {},
				// If the search fails, the caller is not a member and we learned the index where
				// they should be inserted
				Err(index) => {
					members.insert(index, new_member.clone());
					<ExpertMembers<T>>::insert(app_id, model_id, members);
				},
			}
		}

		pub fn model_remove_expert(app_id: u32, model_id: &Vec<u8>, member: &T::AccountId) {
			let mut members = <ExpertMembers<T>>::get(app_id, model_id);

			match members.binary_search(member) {
				// If the search succeeds, the caller is already a member, so just return
				Ok(index) => {
					members.remove(index);
					<ExpertMembers<T>>::insert(app_id, model_id, members);
				},
				// If the search fails, the caller is not a member, so just return
				Err(_) => {},
			}
		}

		pub fn model_creator(app_id: u32, model_id: &Vec<u8>) -> T::AccountId {
			<ModelCreators<T>>::get(app_id, model_id).unwrap()
		}

		pub fn is_finance_member(who: &T::AccountId) -> bool {
			<FinanceMembers<T>>::get().contains(who)
		}

		pub fn is_finance_root(who: &T::AccountId) -> bool {
			*who == <FinanceRoot<T>>::get().unwrap()
		}

		pub fn slash_finance_member(
			member: &T::AccountId,
			receiver: &T::AccountId,
			amount: BalanceOf<T>,
		) -> DispatchResult {
			let deposit = <FinanceMemberDeposit<T>>::get(member);
			if deposit == 0u32.into() {
				// nothing to do
			} else {
				let slash = min(deposit, amount);
				T::Currency::unreserve(member, slash);
				T::Currency::transfer(member, receiver, slash, KeepAlive)?;
				<FinanceMemberDeposit<T>>::insert(member, deposit - slash);
			}

			Ok(())
		}

		/// return valid finance members (depoist is enough)
		pub fn valid_finance_members() -> Vec<T::AccountId> {
			let min_deposit = T::MinFinanceMemberDeposit::get();
			let members: Vec<T::AccountId> = <FinanceMembers<T>>::get();

			if members.len() == 0 {
				return vec![];
			}

			// read out all deposit
			let mut deposits: Vec<(&T::AccountId, BalanceOf<T>)> = vec![];
			for member in members.iter() {
				deposits.push((member, <FinanceMemberDeposit<T>>::get(member)));
			}

			deposits.sort_by(|a, b| b.1.cmp(&a.1));

			let max = deposits[0];
			if max.1 < min_deposit {
				return vec![];
			}

			let mut pos = 0;
			for deposit in deposits.iter() {
				if deposit.1 < max.1 {
					break;
				}

				pos += 1;
			}

			deposits[..pos]
				.iter()
				.map(|deposit| deposit.0.clone())
				.collect::<Vec<T::AccountId>>()
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate as pallet_members;

	use sp_core::H256;
	use sp_runtime::{
		bounded_vec,
		traits::{BadOrigin, BlakeTwo256, IdentityLookup},
		BuildStorage,
	};

	use frame_support::{
		assert_noop, assert_ok, ord_parameter_types, parameter_types,
		traits::{ConstU32, ConstU64, StorageVersion},
	};
	use frame_system::EnsureSignedBy;

	type Block = frame_system::mocking::MockBlock<Test>;
	type Balance = u64;

	frame_support::construct_runtime!(
		pub enum Test
		{
			System: frame_system::{Pallet, Call, Config<T>, Storage, Event<T>},
			Members: pallet_members::{Pallet, Call, Storage, Config<T>, Event<T>},
			Balances: pallet_balances::{Pallet, Call, Storage, Config<T>, Event<T>},
		}
	);

	parameter_types! {
		pub const ExistentialDeposit: Balance = 1;
	}

	impl pallet_balances::Config for Test {
		type Balance = Balance;
		type RuntimeEvent = RuntimeEvent;
		type DustRemoval = ();
		type ExistentialDeposit = ExistentialDeposit;
		type AccountStore = System;
		type WeightInfo = ();
		type MaxLocks = ConstU32<0>;
		type MaxReserves = ConstU32<0>;
		type ReserveIdentifier = [u8; 8];
		type RuntimeHoldReason = RuntimeHoldReason;
		type RuntimeFreezeReason = RuntimeFreezeReason;
		type FreezeIdentifier = ();
		type MaxHolds = ConstU32<0>;
		type MaxFreezes = ConstU32<0>;
	}

	impl frame_system::Config for Test {
		type BaseCallFilter = frame_support::traits::Everything;
		type BlockWeights = ();
		type BlockLength = ();
		type DbWeight = ();
		type RuntimeOrigin = RuntimeOrigin;
		type Nonce = u64;
		type Hash = H256;
		type RuntimeCall = RuntimeCall;
		type Hashing = BlakeTwo256;
		type AccountId = u64;
		type Lookup = IdentityLookup<Self::AccountId>;
		type Block = Block;
		type RuntimeEvent = RuntimeEvent;
		type BlockHashCount = ConstU64<250>;
		type Version = ();
		type PalletInfo = PalletInfo;
		type AccountData = pallet_balances::AccountData<u64>;
		type OnNewAccount = ();
		type OnKilledAccount = ();
		type SystemWeightInfo = ();
		type SS58Prefix = ();
		type OnSetCode = ();
		type MaxConsumers = ConstU32<16>;
		type RuntimeTask = RuntimeTask;
	}

	impl Config for Test {
		type RuntimeEvent = RuntimeEvent;
		type Currency = Balances;
	}

	const TEST_FINANCE_ROOT: u64 = 1;

	pub(crate) fn new_test_ext() -> sp_io::TestExternalities {
		let mut t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();
		// We use default for brevity, but you can configure as desired if needed.
		pallet_members::GenesisConfig::<Test> {
			finance_root: Some(TEST_FINANCE_ROOT),
			..Default::default()
		}
		.assimilate_storage(&mut t)
		.unwrap();
		t.into()
	}

	#[test]
	fn add_finance_member_works() {
		new_test_ext().execute_with(|| {
			assert_ok!(Members::add_finance_member(RuntimeOrigin::signed(TEST_FINANCE_ROOT), 2));
			assert_eq!(Members::finance_members(), vec![TEST_FINANCE_ROOT, 2]);
		});
	}
}
