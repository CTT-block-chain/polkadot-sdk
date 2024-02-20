#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
//use sp_std::vec::Vec;
use frame_support::{
	ensure, fail,
	traits::{Currency, ExistenceRequirement::KeepAlive, Get, ReservableCurrency},
	DefaultNoBound,
};

use node_primitives::{AuthAccountId, Membership};
use scale_info::TypeInfo;
use sp_core::sr25519;
use sp_core::sr25519::Pair;
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

#[derive(Encode, Decode, Clone, Default, PartialEq, RuntimeDebug, TypeInfo)]
#[scale_info(skip_type_params(T))]
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

		/// Max finance members
		type MaxFinanceMembers: Get<u32>;
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
		FinanceMemberStored {
			added_member: T::AccountId,
			who: T::AccountId,
		},
		/// Added a member
		MemberAdded {
			who: T::AccountId,
		},
		/// Removed a member
		MemberRemoved {
			who: T::AccountId,
		},
		AppAdminSet {
			who: T::AccountId,
		},
		AppKeysSet {
			who: T::AccountId,
		},
		ModelCreatorAdded {
			who: T::AccountId,
		},
		NewUserBenefitDropped {
			who: T::AccountId,
			balance: BalanceOf<T>,
		},
		StableExchanged {
			who: T::AccountId,
		},
		AppRedeemAccountSet {
			who: T::AccountId,
		},
		AppRedeemed {
			who: T::AccountId,
			target: T::AccountId,
			balance: BalanceOf<T>,
		},
		FinanceMemberDeposit {
			who: T::AccountId,
		},
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
			params: FinanceMemberParams<T::AccountId, BalanceOf<T>>,
			app_user_account: AuthAccountId,
			app_user_sign: sr25519::Signature,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			log::trace!(
				target: crate::LOG_TARGET,
				"start add_finance_member: who: {:?}, params: {:?}, app_user_account: {:?}, app_user_sign: {:?}",
				who,
				params,
				app_user_account,
				app_user_sign
			);

			ensure!(Self::is_finance_root(&who), Error::<T>::CallerNotFinanceRoot);

			let buf = params.encode();
			ensure!(
				Self::verify_sign(&app_user_account, app_user_sign, &buf),
				Error::<T>::SignVerifyError
			);

			let FinanceMemberParams { deposit, member } = params;

			ensure!(
				deposit >= T::MinFinanceMemberDeposit::get(),
				Error::<T>::FinanceMemberDepositTooLow
			);

			let mut members = FinanceMembers::<T>::get();
			ensure!(
				(members.len() as u32) < T::MaxFinanceMembers::get(),
				Error::<T>::FinanceMemberSizeOver
			);

			match members.binary_search(&member) {
				// If the search succeeds, the caller is already a member, so just return
				Ok(_) => Err(Error::<T>::AlreadyMember.into()),
				// If the search fails, the caller is not a member and we learned the index where
				// they should be inserted
				Err(index) => {
					// make sure deposit success
					T::Currency::reserve(&member, deposit)?;

					<FinanceMemberDeposit<T>>::insert(&member, deposit);

					members.insert(index, member.clone());
					FinanceMembers::<T>::put(members);
					Self::deposit_event(Event::MemberAdded { who: member });
					Ok(())
				},
			}
		}

		#[pallet::call_index(1)]
		#[pallet::weight(0)]
		pub fn add_investor_member(
			origin: OriginFor<T>,
			app_id: u32,
			new_member: T::AccountId,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			ensure!(Self::is_valid_app(app_id), Error::<T>::AppIdInvalid);

			// check if who is app admin
			ensure!(Self::is_app_admin(&who, app_id), Error::<T>::NotAppAdmin);

			let mut members = InvestorMembers::<T>::get();
			//ensure!(members.len() < MAX_MEMBERS, Error::<T>::MembershipLimitReached);

			// We don't want to add duplicate members, so we check whether the potential new
			// member is already present in the list. Because the list is always ordered, we can
			// leverage the binary search which makes this check O(log n).
			match members.binary_search(&new_member) {
				// If the search succeeds, the caller is already a member, so just return
				Ok(_) => Err(Error::<T>::AlreadyMember.into()),
				// If the search fails, the caller is not a member and we learned the index where
				// they should be inserted
				Err(index) => {
					members.insert(index, new_member.clone());
					InvestorMembers::<T>::put(members);
					Self::deposit_event(Event::MemberAdded { who });
					Ok(())
				},
			}
		}

		#[pallet::call_index(2)]
		#[pallet::weight(0)]
		pub fn remove_investor_member(
			origin: OriginFor<T>,
			app_id: u32,
			old_member: T::AccountId,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			ensure!(Self::is_valid_app(app_id), Error::<T>::AppIdInvalid);
			// check if who is app admin
			ensure!(Self::is_app_admin(&who, app_id), Error::<T>::NotAppAdmin);

			let mut members = InvestorMembers::<T>::get();

			// We have to find out if the member exists in the sorted vec, and, if so, where.
			match members.binary_search(&old_member) {
				// If the search succeeds, the caller is a member, so remove her
				Ok(index) => {
					members.remove(index);
					InvestorMembers::<T>::put(members);
					Self::deposit_event(Event::MemberRemoved { who: old_member });
					Ok(())
				},
				// If the search fails, the caller is not a member, so just return
				Err(_) => Err(Error::<T>::NotMember.into()),
			}
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

	impl<T: Config> Membership<T::AccountId, BalanceOf<T>> for Pallet<T> {
		fn is_platform(who: &T::AccountId, app_id: u32) -> bool {
			Self::is_app_admin(who, app_id)
		}

		fn is_expert(who: &T::AccountId, app_id: u32, model_id: &Vec<u8>) -> bool {
			Self::is_model_expert(who, app_id, model_id)
		}

		fn is_app_admin(who: &T::AccountId, app_id: u32) -> bool {
			Self::is_app_admin(who, app_id)
		}

		fn is_investor(who: &T::AccountId) -> bool {
			Self::is_investor(who)
		}

		fn is_finance_member(who: &T::AccountId) -> bool {
			Self::is_finance_member(who)
		}

		fn set_model_creator(
			app_id: u32,
			model_id: &Vec<u8>,
			creator: &T::AccountId,
			is_give_benefit: bool,
		) -> BalanceOf<T> {
			let deposit = T::MinFinanceMemberDeposit::get();
			if is_give_benefit {
				<NewAccountBenefitRecords<T>>::insert(app_id, model_id, deposit);
			}

			<ModelCreators<T>>::insert(app_id, model_id, Some(creator.clone()));
			deposit
		}

		fn transfer_model_owner(app_id: u32, model_id: &Vec<u8>, new_owner: &T::AccountId) {
			<ModelCreators<T>>::insert(app_id, model_id, Some(new_owner.clone()));
		}

		fn is_model_creator(who: &T::AccountId, app_id: u32, model_id: &Vec<u8>) -> bool {
			Self::is_model_creator(who, app_id, model_id)
		}

		fn config_app_admin(who: &T::AccountId, app_id: u32) {
			let mut members = <AppAdmins<T>>::get(app_id);
			members.push(who.clone());
			<AppAdmins<T>>::insert(app_id, members);
		}

		fn config_app_key(who: &T::AccountId, app_id: u32) {
			let mut members = <AppKeys<T>>::get(app_id);
			members.push(who.clone());
			<AppKeys<T>>::insert(app_id, members);
		}

		fn config_app_setting(app_id: u32, rate: u32, name: Vec<u8>, stake: BalanceOf<T>) {
			<AppDataMap<T>>::insert(app_id, &AppData { return_rate: rate, name, stake });
		}

		fn get_app_setting(app_id: u32) -> (u32, Vec<u8>, BalanceOf<T>) {
			let setting = <AppDataMap<T>>::get(app_id);
			(setting.return_rate, setting.name, setting.stake)
		}

		fn is_valid_app(app_id: u32) -> bool {
			<AppDataMap<T>>::contains_key(app_id)
		}

		fn is_valid_app_key(app_id: u32, app_key: &T::AccountId) -> bool {
			Self::is_app_identity(app_key, app_id)
		}

		fn valid_finance_members() -> Vec<T::AccountId> {
			Self::valid_finance_members()
		}

		fn slash_finance_member(
			member: &T::AccountId,
			receiver: &T::AccountId,
			amount: BalanceOf<T>,
		) -> DispatchResult {
			Self::slash_finance_member(member, receiver, amount)
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate as pallet_members;

	use sp_core::{Pair, H256};
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
		pub const MinFinanceMemberDeposit: u64 = 0;
		pub const MaxFinanceMembers: u32 = 16;
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
		type MinFinanceMemberDeposit = MinFinanceMemberDeposit;
		type MaxFinanceMembers = MaxFinanceMembers;
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

		pallet_balances::GenesisConfig::<Test> {
			balances: vec![(1, 10), (2, 200), (3, 30), (4, 40), (5, 50), (6, 60)],
		}
		.assimilate_storage(&mut t)
		.unwrap();

		t.into()
	}

	#[test]
	fn add_finance_member_works() {
		new_test_ext().execute_with(|| {
			let params: FinanceMemberParams<u64, Balance> =
				FinanceMemberParams { deposit: 100, member: 2 };
			let buf = params.encode();

			let pair: sr25519::Pair =
				Pair::from_string("//Alice", None).expect("Static values are valid; qed");

			let public_key = pair.public();
			let app_user_account = AuthAccountId::from(public_key);
			let sign = pair.sign(&buf);

			assert_ok!(Members::add_finance_member(
				RuntimeOrigin::signed(TEST_FINANCE_ROOT),
				params,
				app_user_account,
				sign
			));
			assert_eq!(Members::finance_members(), vec![TEST_FINANCE_ROOT, 2]);
		});
	}
}
