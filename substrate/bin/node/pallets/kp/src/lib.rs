#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
//use sp_std::vec::Vec;
use frame_support::{
	ensure,
	traits::{
		Contains, Currency, EnsureOrigin, ExistenceRequirement::KeepAlive, Get, OnUnbalanced,
		Randomness, ReservableCurrency,
	},
	DefaultNoBound, PalletId, RuntimeDebugNoBound,
};

use frame_system::pallet_prelude::BlockNumberFor;

use node_primitives::{AuthAccountId, Membership, PowerSize};
use scale_info::TypeInfo;
use sp_core::sr25519;
use sp_runtime::{
	traits::{Hash, TrailingZeroInput, Verify},
	MultiSignature, Perbill, Percent, Permill, RuntimeDebug,
};
use sp_std::cmp::min;
use sp_std::cmp::Ordering;
use sp_std::ops::Add;
use sp_std::prelude::*;
use std::collections::HashMap;

/// Edit this file to define custom logic or remove it if it is not needed.
/// Learn more about FRAME and the core library of Substrate FRAME pallets:
/// <https://docs.substrate.io/reference/frame-pallets/>
pub use pallet::*;

type BalanceOf<T> =
	<<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

type NegativeImbalanceOf<T> = <<T as Config>::Currency as Currency<
	<T as frame_system::Config>::AccountId,
>>::NegativeImbalance;

#[derive(Encode, Decode, PartialEq, Clone, RuntimeDebug, TypeInfo)]
pub enum ModelStatus {
	ENABLED = 0,
	DISABLED = 1,
}

impl Default for ModelStatus {
	fn default() -> Self {
		ModelStatus::ENABLED
	}
}

impl From<u8> for ModelStatus {
	fn from(orig: u8) -> Self {
		return match orig {
			0x0 => ModelStatus::ENABLED,
			0x1 => ModelStatus::DISABLED,
			_ => ModelStatus::ENABLED,
		};
	}
}

#[derive(Encode, Decode, Clone, RuntimeDebug, TypeInfo)]
#[scale_info(skip_type_params(T))]
pub struct KPModelData<T: Config> {
	app_id: u32,
	model_id: Vec<u8>,
	expert_id: Vec<u8>,
	status: ModelStatus,
	commodity_name: Vec<u8>,
	commodity_type: u32,
	content_hash: T::Hash,
	sender: T::AccountId,
	owner: AuthAccountId,
	create_reward: BalanceOf<T>,
}

impl<T: Config> Default for KPModelData<T> {
	fn default() -> Self {
		KPModelData {
			app_id: 0,
			model_id: Vec::new(),
			expert_id: Vec::new(),
			status: ModelStatus::DISABLED,
			commodity_name: Vec::new(),
			commodity_type: 0,
			content_hash: T::Hash::default(),
			sender: T::AccountId::decode(&mut TrailingZeroInput::zeroes()).unwrap(),
			owner: AuthAccountId::decode(&mut TrailingZeroInput::zeroes()).unwrap(),
			create_reward: BalanceOf::<T>::default(),
		}
	}
}

#[derive(Encode, Decode, Clone, PartialEq, Default, RuntimeDebug, TypeInfo)]
#[scale_info(skip_type_params(T))]
pub struct KPCommentAccountRecord {
	count: PowerSize,
	fees: PowerSize,
	positive_count: PowerSize,
}

#[derive(Encode, Decode, Clone, Default, PartialEq, RuntimeDebug, TypeInfo)]
pub struct DocumentPower {
	attend: PowerSize,
	content: PowerSize,
	judge: PowerSize,
}

impl Add for DocumentPower {
	type Output = Self;

	fn add(self, other: Self) -> Self {
		Self {
			attend: self.attend + other.attend,
			content: self.content + other.content,
			judge: self.judge + other.judge,
		}
	}
}

impl<'a, 'b> Add<&'b DocumentPower> for &'a DocumentPower {
	type Output = DocumentPower;

	fn add(self, other: &'b DocumentPower) -> DocumentPower {
		DocumentPower {
			attend: self.attend + other.attend,
			content: self.content + other.content,
			judge: self.judge + other.judge,
		}
	}
}

pub trait PowerSum {
	fn total(&self) -> PowerSize;
}

impl PowerSum for DocumentPower {
	fn total(&self) -> PowerSize {
		self.attend + self.content + self.judge
	}
}

#[derive(Encode, Decode, Clone, Copy, Default, PartialEq, RuntimeDebug, TypeInfo)]
pub struct KPProductPublishData {
	para_issue_rate: PowerSize,
	self_issue_rate: PowerSize,
	refer_count: PowerSize,
}

#[derive(Encode, Decode, Clone, Default, PartialEq, RuntimeDebug, TypeInfo)]
pub struct KPProductPublishRateMax {
	para_issue_rate: PowerSize,
	self_issue_rate: PowerSize,
	refer_count: PowerSize,
}

#[derive(Encode, Decode, Clone, Default, PartialEq, RuntimeDebug, TypeInfo)]
pub struct KPProductIdentifyData {
	goods_price: PowerSize,
	ident_rate: PowerSize,
	ident_consistence: PowerSize,
	seller_consistence: PowerSize,
	cart_id: Vec<u8>,
}

#[derive(Encode, Decode, Clone, Default, PartialEq, RuntimeDebug, TypeInfo)]
pub struct KPProductIdentifyRateMax {
	ident_rate: PowerSize,
	ident_consistence: PowerSize,
	seller_consistence: PowerSize,
}

#[derive(Encode, Decode, Clone, Default, PartialEq, RuntimeDebug, TypeInfo)]
pub struct KPProductTryData {
	goods_price: PowerSize,
	offset_rate: PowerSize,
	true_rate: PowerSize,
	seller_consistence: PowerSize,
	cart_id: Vec<u8>,
}

#[derive(Encode, Decode, Clone, Default, PartialEq, RuntimeDebug, TypeInfo)]
pub struct KPProductTryRateMax {
	offset_rate: PowerSize,
	true_rate: PowerSize,
	seller_consistence: PowerSize,
}

#[derive(Encode, Decode, Clone, Default, PartialEq, RuntimeDebug, TypeInfo)]
pub struct KPProductChooseData {
	sell_count: PowerSize,
	try_count: PowerSize,
}

#[derive(Encode, Decode, Clone, Default, PartialEq, RuntimeDebug, TypeInfo)]
pub struct KPProductChooseDataMax {
	sell_count: PowerSize,
	try_count: PowerSize,
}

#[derive(Encode, Decode, Clone, Default, PartialEq, RuntimeDebug, TypeInfo)]
pub struct KPModelCreateData {
	producer_count: PowerSize,
	product_count: PowerSize,
}

#[derive(Encode, Decode, Clone, Default, PartialEq, RuntimeDebug, TypeInfo)]
pub struct KPModelCreateDataMax {
	producer_count: PowerSize,
	product_count: PowerSize,
}

#[derive(Encode, Decode, Clone, PartialEq, RuntimeDebug, TypeInfo)]
pub enum DocumentSpecificData {
	ProductPublish(KPProductPublishData),
	ProductIdentify(KPProductIdentifyData),
	ProductTry(KPProductTryData),
	ProductChoose(KPProductChooseData),
	ModelCreate(KPModelCreateData),
}

impl Default for DocumentSpecificData {
	fn default() -> Self {
		DocumentSpecificData::ProductPublish(KPProductPublishData::default())
	}
}

#[derive(Encode, Decode, PartialEq, Clone, RuntimeDebug, TypeInfo)]
pub enum DocumentType {
	ProductPublish = 0,
	ProductIdentify,
	ProductTry,

	// this two types need special process
	ProductChoose,
	ModelCreate,

	Unknown,
}

impl Default for DocumentType {
	fn default() -> Self {
		DocumentType::ProductPublish
	}
}

impl From<u8> for DocumentType {
	fn from(orig: u8) -> Self {
		return match orig {
			0 => DocumentType::ProductPublish,
			1 => DocumentType::ProductIdentify,
			2 => DocumentType::ProductTry,
			3 => DocumentType::ProductChoose,
			4 => DocumentType::ModelCreate,
			_ => DocumentType::Unknown,
		};
	}
}

impl From<DocumentType> for u8 {
	fn from(orig: DocumentType) -> Self {
		return match orig {
			DocumentType::ProductPublish => 0,
			DocumentType::ProductIdentify => 1,
			DocumentType::ProductTry => 2,
			DocumentType::ProductChoose => 3,
			DocumentType::ModelCreate => 4,
			_ => 5,
		};
	}
}

#[derive(Encode, Decode, PartialEq, Clone, Copy, RuntimeDebug, TypeInfo)]
pub enum CommentTrend {
	Positive = 0,
	Negative = 1,
	Empty = 2,
}

impl Default for CommentTrend {
	fn default() -> Self {
		CommentTrend::Empty
	}
}

impl From<u8> for CommentTrend {
	fn from(orig: u8) -> Self {
		return match orig {
			0x0 => CommentTrend::Positive,
			0x1 => CommentTrend::Negative,
			_ => CommentTrend::Empty,
		};
	}
}

#[derive(Encode, Decode, RuntimeDebug, TypeInfo)]
#[scale_info(skip_type_params(T))]
pub struct KPDocumentData<T: Config> {
	app_id: u32,
	document_id: Vec<u8>,
	model_id: Vec<u8>,
	product_id: Vec<u8>,
	content_hash: T::Hash,
	sender: T::AccountId,
	owner: AuthAccountId,
	document_type: DocumentType,
	document_data: DocumentSpecificData,
	comment_count: PowerSize,
	comment_total_fee: PowerSize,
	comment_positive_count: PowerSize,
	expert_trend: CommentTrend,
	platform_trend: CommentTrend,
}

impl<T: Config> Default for KPDocumentData<T> {
	fn default() -> Self {
		KPDocumentData {
			app_id: 0,
			document_id: Vec::new(),
			model_id: Vec::new(),
			product_id: Vec::new(),
			content_hash: T::Hash::default(),
			sender: T::AccountId::decode(&mut TrailingZeroInput::zeroes()).unwrap(),
			owner: AuthAccountId::decode(&mut TrailingZeroInput::zeroes()).unwrap(),
			document_type: DocumentType::default(),
			document_data: DocumentSpecificData::default(),
			comment_count: PowerSize::default(),
			comment_total_fee: PowerSize::default(),
			comment_positive_count: PowerSize::default(),
			expert_trend: CommentTrend::default(),
			platform_trend: CommentTrend::default(),
		}
	}
}

#[derive(Encode, Decode, Clone, RuntimeDebug, TypeInfo)]
#[scale_info(skip_type_params(T))]
pub struct KPCommentData<T: Config> {
	app_id: u32,
	document_id: Vec<u8>,
	comment_id: Vec<u8>,
	comment_hash: T::Hash,
	comment_fee: PowerSize,
	comment_trend: u8,
	sender: T::AccountId,
	owner: AuthAccountId,
}

impl<T: Config> Default for KPCommentData<T> {
	fn default() -> Self {
		KPCommentData {
			app_id: 0,
			document_id: Vec::new(),
			comment_id: Vec::new(),
			comment_hash: T::Hash::default(),
			comment_fee: PowerSize::default(),
			comment_trend: 0,
			sender: T::AccountId::decode(&mut TrailingZeroInput::zeroes()).unwrap(),
			owner: AuthAccountId::decode(&mut TrailingZeroInput::zeroes()).unwrap(),
		}
	}
}

#[derive(Encode, Decode, Clone, PartialEq, Default, RuntimeDebug, TypeInfo)]
pub struct CommentMaxRecord {
	max_count: PowerSize,
	max_fee: PowerSize,
	max_positive: PowerSize,

	// for document, this is the max of document's total comment cost/count
	// for account, this is the max of account's total comment fees/count
	max_unit_fee: PowerSize,
}

#[derive(Encode, Decode, Clone, Default, Eq, RuntimeDebug, TypeInfo)]
pub struct CommodityTypeData {
	type_id: u32,
	type_desc: Vec<u8>,
}

impl PartialEq for CommodityTypeData {
	fn eq(&self, other: &Self) -> bool {
		self.type_id == other.type_id
	}
}

impl Ord for CommodityTypeData {
	fn cmp(&self, other: &Self) -> Ordering {
		self.type_id.cmp(&other.type_id)
	}
}

impl PartialOrd for CommodityTypeData {
	fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
		Some(self.cmp(other))
	}
}

#[derive(Encode, Decode, Clone, PartialEq, RuntimeDebug, TypeInfo)]
#[scale_info(skip_type_params(T))]
pub struct AppIncomeCycleRecord<T: Config> {
	pub initial: BalanceOf<T>,
	pub balance: BalanceOf<T>,
	pub cycle: BlockNumberFor<T>,
	pub app_id: u32,
	pub income: u64,
}

impl<T: Config> Default for AppIncomeCycleRecord<T> {
	fn default() -> Self {
		AppIncomeCycleRecord {
			initial: BalanceOf::<T>::default(),
			balance: BalanceOf::<T>::default(),
			cycle: BlockNumberFor::<T>::default(),
			app_id: 0,
			income: 0,
		}
	}
}

#[derive(Encode, Decode, Clone, Default, PartialEq, RuntimeDebug, TypeInfo)]
#[scale_info(skip_type_params(T))]
pub struct AppFinancedUserExchangeData<T: Config> {
	pub exchange_amount: BalanceOf<T>,
	// 0: initial state,
	// 1: reserved,
	// 2: received cash and burned,
	// 3: not receive cash but got slash from finance member
	pub status: u8,
	pub pay_id: Vec<u8>,
}

#[derive(Encode, Decode, PartialEq, Default, Clone, RuntimeDebug, TypeInfo)]
#[scale_info(skip_type_params(T))]
pub struct ModelCycleIncomeReward<T: Config> {
	account: T::AccountId,
	app_id: u32,
	model_id: Vec<u8>,
	reward: BalanceOf<T>,
}

#[derive(Encode, Decode, Clone, PartialEq, RuntimeDebug, TypeInfo)]
#[scale_info(skip_type_params(T))]
pub struct AppFinancedData<T: Config> {
	pub app_id: u32,
	pub proposal_id: Vec<u8>,
	pub amount: BalanceOf<T>,
	pub exchange: BalanceOf<T>,
	pub block: BlockNumberFor<T>,
	pub total_balance: BalanceOf<T>,
	pub exchanged: BalanceOf<T>,
	pub exchange_end_block: BlockNumberFor<T>,
}

impl<T: Config> Default for AppFinancedData<T> {
	fn default() -> Self {
		AppFinancedData {
			app_id: 0,
			proposal_id: Vec::new(),
			amount: BalanceOf::<T>::default(),
			exchange: BalanceOf::<T>::default(),
			block: BlockNumberFor::<T>::default(),
			total_balance: BalanceOf::<T>::default(),
			exchanged: BalanceOf::<T>::default(),
			exchange_end_block: BlockNumberFor::<T>::default(),
		}
	}
}

#[derive(Encode, Decode, Default, Clone, Eq, RuntimeDebug, TypeInfo)]
#[scale_info(skip_type_params(T))]
pub struct CommodityLeaderBoardData<T: Config> {
	cart_id: Vec<u8>,
	cart_id_hash: T::Hash,
	power: PowerSize,
	owner: T::AccountId,
}

impl<T: Config> PartialEq for CommodityLeaderBoardData<T> {
	fn eq(&self, other: &Self) -> bool {
		self.power == other.power
	}
}

impl<T: Config> Ord for CommodityLeaderBoardData<T> {
	fn cmp(&self, other: &Self) -> Ordering {
		self.power.cmp(&other.power)
	}
}

impl<T: Config> PartialOrd for CommodityLeaderBoardData<T> {
	fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
		Some(self.cmp(other))
	}
}

#[derive(Encode, Decode, Default, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo)]
#[scale_info(skip_type_params(T))]
pub struct LeaderBoardResult<T: Config> {
	pub accounts: Vec<T::AccountId>,
	pub board: Vec<LeaderBoardItem<T>>,
}

#[derive(Encode, Decode, Default, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo)]
#[scale_info(skip_type_params(T))]
pub struct LeaderBoardItem<T: Config> {
	pub cart_id: Vec<u8>,
	pub power: PowerSize,
	pub owner: T::AccountId,
}

#[derive(Encode, Decode, Default, Clone, Eq, RuntimeDebug, TypeInfo)]
#[scale_info(skip_type_params(T))]
pub struct CommentWeightData<T: Config> {
	account: T::AccountId,
	position: u64,
	cash_cost: PowerSize,
}

impl<T: Config> PartialEq for CommentWeightData<T> {
	fn eq(&self, other: &Self) -> bool {
		self.account == other.account
	}
}

impl<T: Config> Ord for CommentWeightData<T> {
	fn cmp(&self, other: &Self) -> Ordering {
		self.position.cmp(&other.position)
	}
}

impl<T: Config> PartialOrd for CommentWeightData<T> {
	fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
		Some(self.cmp(other))
	}
}

#[derive(Encode, Decode, Clone, Default, RuntimeDebug, TypeInfo)]
pub struct AccountStatistics {
	create_commodity_num: u32,
	slash_commodity_num: u32,
	slash_kp_total: u64,
	comment_num: u32,
	comment_cost_total: u64,
	comment_cost_max: u64,
	comment_positive_trend_num: u32,
	comment_negative_trend_num: u32,
}

#[derive(Encode, Decode, PartialEq, Default, Clone, Copy, RuntimeDebug, TypeInfo)]
pub enum TechFundWithdrawLevel {
	#[default]
	LV1 = 0,
	LV2,
	LV3,
	LV4,
	LV5,
}

#[derive(Encode, Decode, PartialEq, Clone, Default, Copy, RuntimeDebug, TypeInfo)]
pub enum TechFundWithdrawType {
	#[default]
	ChainDev = 0,
	Tctp,
	Model,
	Knowledge,
	ChainAdmin,
}

#[derive(Encode, Decode, PartialEq, Default, Clone, RuntimeDebug, TypeInfo)]
#[scale_info(skip_type_params(T))]
pub struct TechFundWithdrawData<T: Config> {
	account: T::AccountId,
	amount: BalanceOf<T>,
	dev_level: TechFundWithdrawLevel,
	dev_type: TechFundWithdrawType,
	reason: T::Hash,
}

#[derive(Encode, Decode, PartialEq, Clone, RuntimeDebug, TypeInfo)]
#[scale_info(skip_type_params(T))]
pub struct ModelDisputeRecord<T: Config> {
	pub app_id: u32,
	pub model_id: Vec<u8>,
	pub comment_id: Vec<u8>,
	pub dispute_type: ModelDisputeType,
	pub block: BlockNumberFor<T>,
}

impl<T: Config> Default for ModelDisputeRecord<T> {
	fn default() -> Self {
		ModelDisputeRecord {
			app_id: 0,
			model_id: Vec::new(),
			comment_id: Vec::new(),
			dispute_type: ModelDisputeType::NoneIntendNormal,
			block: BlockNumberFor::<T>::default(),
		}
	}
}

#[derive(Encode, Decode, PartialEq, Clone, Copy, Default, RuntimeDebug, TypeInfo)]
pub enum ModelDisputeType {
	#[default]
	NoneIntendNormal = 0,
	IntendNormal,
	Serious,
}

#[derive(Encode, Decode, PartialEq, Clone, RuntimeDebug, TypeInfo)]
#[scale_info(skip_type_params(T))]
pub struct CommoditySlashRecord<T: Config> {
	pub app_id: u32,
	pub comment_id: Vec<u8>,
	pub cart_id: Vec<u8>,
	pub block: BlockNumberFor<T>,
}

impl<T: Config> Default for CommoditySlashRecord<T> {
	fn default() -> Self {
		CommoditySlashRecord {
			app_id: 0,
			comment_id: Vec::new(),
			cart_id: Vec::new(),
			block: BlockNumberFor::<T>::default(),
		}
	}
}

const LOG_TARGET: &str = "ctt::kp";

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

	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// Because this pallet emits events, it depends on the runtime's definition of an event.
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
		/// The runtime's definition of a Currency.
		type Currency: ReservableCurrency<Self::AccountId>;
		/// Membership control
		type Membership: Membership<Self::AccountId, BalanceOf<Self>>;
		/// TechnicalCommittee member ship check
		type TechMembers: Contains<Self::AccountId>;
		/// Required origin for perform tech commite operation
		type TechMemberOrigin: EnsureOrigin<Self::RuntimeOrigin>;
		/// Handler for the unbalanced reduction when slashing a model create deposit.
		type Slash: OnUnbalanced<NegativeImbalanceOf<Self>>;
		/// Handler for the unbalanced decrease when redeem amount are burned.
		type BurnDestination: OnUnbalanced<NegativeImbalanceOf<Self>>;
		/// Something that provides randomness in the runtime.
		type Randomness: Randomness<Self::Hash, BlockNumberFor<Self>>;
		/// Finance treasury model id
		type FinTreasuryModuleId: Get<PalletId>;
		/// Model treasury model id
		type ModTreasuryModuleId: Get<PalletId>;
		/// Tech treasury model id
		type TechTreasuryModuleId: Get<PalletId>;
		/// Treasury model id
		type TreasuryModuleId: Get<PalletId>;
		/// 5 dimensions weight config
		type TopWeightProductPublish: Get<u8>;
		type TopWeightDocumentIdentify: Get<u8>;
		type TopWeightDocumentTry: Get<u8>;
		type TopWeightAccountAttend: Get<u8>;
		type TopWeightAccountStake: Get<u8>;

		/// Document Power attend weight
		type DocumentPowerWeightAttend: Get<u8>;

		/// Document Power content weight
		type DocumentPowerWeightContent: Get<u8>;

		/// Document Power judge weight
		type DocumentPowerWeightJudge: Get<u8>;

		/// Comment Power count weight
		type CommentPowerWeightCount: Get<u8>;

		/// Comment Power cost weight
		type CommentPowerWeightCost: Get<u8>;

		/// Comment Power cost per uint weight
		type CommentPowerWeightPerCost: Get<u8>;

		/// Comment Power positive weight
		type CommentPowerWeightPositive: Get<u8>;

		type CommentPowerWeight: Get<u8>;

		/// Document Publish content weight
		type DocumentPublishWeightParamsRate: Get<u8>;
		type DocumentPublishWeightParamsSelfRate: Get<u8>;
		type DocumentPublishWeightParamsAttendRate: Get<u8>;

		/// Document Identify content weight
		type DocumentIdentifyWeightParamsRate: Get<u8>;
		type DocumentIdentifyWeightCheckRate: Get<u8>;
		type DocumentIdentifyWeightConsistentRate: Get<u8>;

		/// Document Try content weight
		type DocumentTryWeightBiasRate: Get<u8>;
		type DocumentTryWeightTrueRate: Get<u8>;
		type DocumentTryWeightConsistentRate: Get<u8>;

		/// Below for Choose & Model special documents
		/// Document Choose content weight
		type DocumentChooseWeightSellCount: Get<u8>;
		type DocumentChooseWeightTryCount: Get<u8>;

		/// Document Model content weight
		type DocumentModelWeightProducerCount: Get<u8>;
		type DocumentModelWeightProductCount: Get<u8>;

		/// Document Choose & Model Power attend weight
		type DocumentCMPowerWeightAttend: Get<u8>;

		/// Document Choose & Model Power content weight
		type DocumentCMPowerWeightContent: Get<u8>;

		/// Document Choose & Model Power judge weight
		type DocumentCMPowerWeightJudge: Get<u8>;

		/// Comment Power count weight
		type CommentCMPowerWeightCount: Get<u8>;

		/// Comment Power cost weight
		type CommentCMPowerWeightCost: Get<u8>;

		/// Comment Power cost per uint weight
		type CommentCMPowerWeightPerCost: Get<u8>;

		/// Comment Power positive weight
		type CommentCMPowerWeightPositive: Get<u8>;

		type CMPowerAccountAttend: Get<u8>;

		type ModelCreateDeposit: Get<BalanceOf<Self>>;
		type ModelCycleIncomeRewardTotal: Get<BalanceOf<Self>>;

		/// App financed purpose minimal exchange rate
		type KptExchangeMinRate: Get<Permill>;

		type AppLeaderBoardInterval: Get<BlockNumberFor<Self>>;

		type AppLeaderBoardMaxPos: Get<u32>;

		type AppFinanceExchangePeriod: Get<BlockNumberFor<Self>>;

		type ModelIncomeCyclePeriod: Get<BlockNumberFor<Self>>;
		type ModelIncomeCollectingPeriod: Get<BlockNumberFor<Self>>;
		type ModelIncomeRewardingPeriod: Get<BlockNumberFor<Self>>;

		type ModelDisputeCycleCount: Get<u32>;
		type ModelDisputeCycleLv2IncreaseCount: Get<u32>;
		type ModelDisputeCycleLv3IncreaseCount: Get<u32>;

		type ModelDisputeRewardLv1: Get<BalanceOf<Self>>;
		type ModelDisputeRewardLv2: Get<BalanceOf<Self>>;
		type ModelDisputeRewardLv3: Get<BalanceOf<Self>>;

		// Model dispute slash config
		type ModelDisputeLv1Slash: Get<BalanceOf<Self>>;
		type ModelDisputeDelayTime: Get<BlockNumberFor<Self>>;

		// Base Balance of tech fund
		type TechFundBase: Get<BalanceOf<Self>>;

		type RedeemFeeRate: Get<u32>;

		type CommentRewardNormalRate: Get<u32>;
		type CommentRewardExpertRate: Get<u32>;
	}

	#[pallet::storage]
	#[pallet::getter(fn app_id_range)]
	pub(super) type AppIdRange<T: Config> =
		StorageMap<_, Twox64Concat, Vec<u8>, (u32, BalanceOf<T>, u32, u32, u32), ValueQuery>;

	// (AppId, ModelId) -> KPModelData
	#[pallet::storage]
	#[pallet::getter(fn kp_model_data_by_idhash)]
	pub(super) type KPModelDataByHash<T: Config> =
		StorageDoubleMap<_, Twox64Concat, u32, Twox64Concat, Vec<u8>, KPModelData<T>, ValueQuery>;

	// (AppId, ModelId) -> BalanceOf<T>  deposit value of create model
	#[pallet::storage]
	#[pallet::getter(fn kp_model_deposit_map)]
	pub(super) type KPModelDepositMap<T: Config> =
		StorageDoubleMap<_, Twox64Concat, u32, Twox64Concat, Vec<u8>, BalanceOf<T>, ValueQuery>;

	// (AppId, AuthAccountId) -> KPCommentAccountRecord
	#[pallet::storage]
	#[pallet::getter(fn kp_comment_account_record_map)]
	pub(super) type KPCommentAccountRecordMap<T: Config> = StorageDoubleMap<
		_,
		Twox64Concat,
		u32,
		Twox64Concat,
		AuthAccountId,
		KPCommentAccountRecord,
		ValueQuery,
	>;

	// AuthAccountId -> PowerSize max goods_price
	#[pallet::storage]
	#[pallet::getter(fn kp_account_max_purchase_by_idhash)]
	pub(super) type KPAccountMaxPurchaseByIdHash<T: Config> =
		StorageMap<_, Twox64Concat, AuthAccountId, PowerSize, ValueQuery>;

	// (AppId, CartId) -> PowerSize user computed product identify/try power map
	// (Publish, Identify, Try, OwnerAction, OwnerEconomic)
	#[pallet::storage]
	#[pallet::getter(fn kp_purchase_power_by_idhash)]
	pub(super) type KPPurchasePowerByIdHash<T: Config> = StorageDoubleMap<
		_,
		Twox64Concat,
		u32,
		Twox64Concat,
		Vec<u8>,
		(DocumentPower, DocumentPower, DocumentPower, PowerSize, PowerSize),
		ValueQuery,
	>;

	// Slash commodity power black list (AppId, CartId) -> bool
	#[pallet::storage]
	#[pallet::getter(fn kp_purchase_black_list)]
	pub(super) type KPPurchaseBlackList<T: Config> =
		StorageDoubleMap<_, Twox64Concat, u32, Twox64Concat, Vec<u8>, bool, ValueQuery>;

	// (AppId, DocumentId) -> PowerSize misc document power map (currently for product choose and model create)
	#[pallet::storage]
	#[pallet::getter(fn kp_misc_document_power_by_idhash)]
	pub(super) type KPMiscDocumentPowerByIdHash<T: Config> =
		StorageDoubleMap<_, Twox64Concat, u32, Twox64Concat, Vec<u8>, PowerSize, ValueQuery>;

	// (AppId, DocumentId) -> KPDocumentData
	#[pallet::storage]
	#[pallet::getter(fn kp_document_data_by_idhash)]
	pub(super) type KPDocumentDataByIdHash<T: Config> = StorageDoubleMap<
		_,
		Twox64Concat,
		u32,
		Twox64Concat,
		Vec<u8>,
		KPDocumentData<T>,
		ValueQuery,
	>;

	// (AppId, DocumentId) -> document power
	#[pallet::storage]
	#[pallet::getter(fn kp_document_power_by_idhash)]
	pub(super) type KPDocumentPowerByIdHash<T: Config> =
		StorageDoubleMap<_, Twox64Concat, u32, Twox64Concat, Vec<u8>, DocumentPower, ValueQuery>;

	// (AppId, ProductId) -> DocumentId document index map
	#[pallet::storage]
	#[pallet::getter(fn kp_document_product_index_by_idhash)]
	pub(super) type KPDocumentProductIndexByIdHash<T: Config> =
		StorageDoubleMap<_, Twox64Concat, u32, Twox64Concat, Vec<u8>, Vec<u8>, ValueQuery>;

	// (AppId, CartId) -> Vec<u8> cartid -> product identify document id
	#[pallet::storage]
	#[pallet::getter(fn kp_cart_product_identify_index_by_idhash)]
	pub(super) type KPCartProductIdentifyIndexByIdHash<T: Config> =
		StorageDoubleMap<_, Twox64Concat, u32, Twox64Concat, Vec<u8>, Vec<u8>, ValueQuery>;

	// (AppId, CartId) -> Vec<u8> cartid -> product try document id
	#[pallet::storage]
	#[pallet::getter(fn kp_cart_product_try_index_by_idhash)]
	pub(super) type KPCartProductTryIndexByIdHash<T: Config> =
		StorageDoubleMap<_, Twox64Concat, u32, Twox64Concat, Vec<u8>, Vec<u8>, ValueQuery>;

	// (AppId, CommentId) -> KnowledgeCommentData
	#[pallet::storage]
	#[pallet::getter(fn kp_comment_data_by_idhash)]
	pub(super) type KPCommentDataByIdHash<T: Config> =
		StorageDoubleMap<_, Twox64Concat, u32, Twox64Concat, Vec<u8>, KPCommentData<T>, ValueQuery>;

	// global total knowledge power (only for commodity power)
	#[pallet::storage]
	#[pallet::getter(fn total_power)]
	pub(super) type TotalPower<T: Config> = StorageValue<_, PowerSize, ValueQuery>;

	// miner power table
	#[pallet::storage]
	#[pallet::getter(fn miner_power_by_account)]
	pub(super) type MinerPowerByAccount<T: Config> =
		StorageMap<_, Twox64Concat, T::AccountId, PowerSize, ValueQuery>;

	// account attend power (AccountId, AppId) -> PowerSize
	#[pallet::storage]
	#[pallet::getter(fn account_attend_power_map)]
	pub(super) type AccountAttendPowerMap<T: Config> =
		StorageDoubleMap<_, Twox64Concat, T::AccountId, Twox64Concat, u32, PowerSize, ValueQuery>;

	// global power compute related parameters:
	// AppId -> single document's max comment count
	#[pallet::storage]
	#[pallet::getter(fn comment_max_info_per_doc_map)]
	pub(super) type CommentMaxInfoPerDocMap<T: Config> =
		StorageMap<_, Twox64Concat, u32, CommentMaxRecord, ValueQuery>;

	// compare base of single document comment max record
	// this was created when the document was created, and as a compute base
	// also act as a action power max(will not over it)
	// (AppId, DocumentId) -> CommentMaxRecord
	#[pallet::storage]
	#[pallet::getter(fn document_comment_power_base)]
	pub(super) type DocumentCommentPowerBase<T: Config> =
		StorageDoubleMap<_, Twox64Concat, u32, Twox64Concat, Vec<u8>, CommentMaxRecord, ValueQuery>;

	// AppId -> single account's max comment count
	#[pallet::storage]
	#[pallet::getter(fn comment_max_info_per_account_map)]
	pub(super) type CommentMaxInfoPerAccountMap<T: Config> =
		StorageMap<_, Twox64Concat, u32, CommentMaxRecord, ValueQuery>;

	// Global max goods_price
	#[pallet::storage]
	#[pallet::getter(fn max_goods_price)]
	pub(super) type MaxGoodsPrice<T: Config> = StorageValue<_, PowerSize, ValueQuery>;

	// AppId -> document publish params max
	#[pallet::storage]
	#[pallet::getter(fn document_publish_max_params)]
	pub(super) type DocumentPublishMaxParams<T: Config> =
		StorageMap<_, Twox64Concat, u32, KPProductPublishRateMax, ValueQuery>;

	// AppId -> document identify params max
	#[pallet::storage]
	#[pallet::getter(fn document_identify_max_params)]
	pub(super) type DocumentIdentifyMaxParams<T: Config> =
		StorageMap<_, Twox64Concat, u32, KPProductIdentifyRateMax, ValueQuery>;

	// AppId -> document try params max
	#[pallet::storage]
	#[pallet::getter(fn document_try_max_params)]
	pub(super) type DocumentTryMaxParams<T: Config> =
		StorageMap<_, Twox64Concat, u32, KPProductTryRateMax, ValueQuery>;

	// AppId -> document choose params max
	#[pallet::storage]
	#[pallet::getter(fn document_choose_max_params)]
	pub(super) type DocumentChooseMaxParams<T: Config> =
		StorageMap<_, Twox64Concat, u32, KPProductChooseDataMax, ValueQuery>;

	// AppId -> document model create params max
	#[pallet::storage]
	#[pallet::getter(fn document_model_create_max_params)]
	pub(super) type DocumentModelCreateMaxParams<T: Config> =
		StorageMap<_, Twox64Concat, u32, KPModelCreateDataMax, ValueQuery>;

	// Commodity types set
	#[pallet::storage]
	#[pallet::getter(fn commodity_type_sets)]
	pub(super) type CommodityTypeSets<T: Config> =
		StorageValue<_, Vec<CommodityTypeData>, ValueQuery>;

	// commodity_type_id => type desc map
	#[pallet::storage]
	#[pallet::getter(fn commodity_type_map)]
	pub(super) type CommodityTypeMap<T: Config> =
		StorageMap<_, Twox64Concat, u32, Vec<u8>, ValueQuery>;

	// app_id & commodity_type_id => true/false
	#[pallet::storage]
	#[pallet::getter(fn model_first_type_benefit_record)]
	pub(super) type ModelFirstTypeBenefitRecord<T: Config> =
		StorageDoubleMap<_, Twox64Concat, u32, Twox64Concat, u32, bool, ValueQuery>;

	// app id => u32
	#[pallet::storage]
	#[pallet::getter(fn app_model_total_config)]
	pub(super) type AppModelTotalConfig<T: Config> =
		StorageMap<_, Twox64Concat, u32, u32, ValueQuery>;

	// app id => u32
	#[pallet::storage]
	#[pallet::getter(fn app_model_count)]
	pub(super) type AppModelCount<T: Config> = StorageMap<_, Twox64Concat, u32, u32, ValueQuery>;

	// model year incoming double map, main key is cycle (u64), sub key is hash of AppId & ModelId
	#[pallet::storage]
	#[pallet::getter(fn model_cycle_income)]
	pub(super) type ModelCycleIncome<T: Config> = StorageNMap<
		Key = (
			NMapKey<Twox64Concat, BlockNumberFor<T>>,
			NMapKey<Twox64Concat, u32>,
			NMapKey<Twox64Concat, Vec<u8>>,
		),
		Value = u64,
	>;

	// app cycle income
	#[pallet::storage]
	#[pallet::getter(fn app_cycle_income)]
	pub(super) type AppCycleIncome<T: Config> = StorageDoubleMap<
		_,
		Twox64Concat,
		BlockNumberFor<T>,
		Twox64Concat,
		u32,
		AppIncomeCycleRecord<T>,
		ValueQuery,
	>;

	// app_id, cycle, account
	#[pallet::storage]
	#[pallet::getter(fn app_cycle_income_exchange_records)]
	pub(super) type AppCycleIncomeExchangeRecords<T: Config> = StorageNMap<
		Key = (NMapKey<Twox64Concat, u32>, NMapKey<Twox64Concat, BlockNumberFor<T>>),
		Value = AppFinancedUserExchangeData<T>,
	>;

	// (AppId & cycle index) -> user accounts set
	#[pallet::storage]
	#[pallet::getter(fn app_cycle_income_exchange_set)]
	pub(super) type AppCycleIncomeExchangeSet<T: Config> = StorageNMap<
		Key = (NMapKey<Twox64Concat, u32>, NMapKey<Twox64Concat, BlockNumberFor<T>>),
		Value = Vec<T::AccountId>,
	>;

	// (AppId & cycle index) -> this cycle finance member account
	#[pallet::storage]
	#[pallet::getter(fn app_cycle_income_finance_member)]
	pub(super) type AppCycleIncomeFinanceMember<T: Config> = StorageNMap<
		Key = (NMapKey<Twox64Concat, u32>, NMapKey<Twox64Concat, BlockNumberFor<T>>),
		Value = T::AccountId,
	>;

	// (AppId & proposal id) -> this app finance proposal finance member account
	#[pallet::storage]
	#[pallet::getter(fn app_finance_finance_member)]
	pub(super) type AppFinanceFinanceMember<T: Config> = StorageDoubleMap<
		_,
		Twox64Concat,
		u32,
		Twox64Concat,
		Vec<u8>,
		Option<T::AccountId>,
		ValueQuery,
	>;

	// App cycle income burn total
	#[pallet::storage]
	#[pallet::getter(fn app_cycle_income_burn_total)]
	pub(super) type AppCycleIncomeBurnTotal<T: Config> = StorageValue<_, BalanceOf<T>, ValueQuery>;

	// App cycle income count
	#[pallet::storage]
	#[pallet::getter(fn app_cycle_income_count)]
	pub(super) type AppCycleIncomeCount<T: Config> = StorageValue<_, u32, ValueQuery>;

	// cycle number => total income
	#[pallet::storage]
	#[pallet::getter(fn model_cycle_income_total)]
	pub(super) type ModelCycleIncomeTotal<T: Config> =
		StorageMap<_, Twox64Concat, BlockNumberFor<T>, u64, ValueQuery>;

	// cycle_index (app_id, model_id)
	#[pallet::storage]
	#[pallet::getter(fn model_cycle_income_reward_records)]
	pub(super) type ModelCycleIncomeRewardRecords<T: Config> = StorageNMap<
		Key = (
			NMapKey<Twox64Concat, BlockNumberFor<T>>,
			NMapKey<Twox64Concat, u32>,
			NMapKey<Twox64Concat, Vec<u8>>,
		),
		Value = BalanceOf<T>,
	>;

	// block number -> Vec<ModelCycleIncomeReward>
	#[pallet::storage]
	#[pallet::getter(fn model_cycle_income_reward_store)]
	pub(super) type ModelCycleIncomeRewardStore<T: Config> =
		StorageMap<_, Twox64Concat, BlockNumberFor<T>, Vec<ModelCycleIncomeReward<T>>, ValueQuery>;

	// total model reward sending count
	#[pallet::storage]
	#[pallet::getter(fn model_income_reward_total)]
	pub(super) type ModelIncomeRewardTotal<T: Config> = StorageValue<_, BalanceOf<T>, ValueQuery>;

	// cycle, app_id, model_id
	#[pallet::storage]
	#[pallet::getter(fn model_cycle_dispute_count)]
	pub(super) type ModelCycleDisputeCount<T: Config> = StorageNMap<
		Key = (
			NMapKey<Twox64Concat, BlockNumberFor<T>>,
			NMapKey<Twox64Concat, u32>,
			NMapKey<Twox64Concat, Vec<u8>>,
		),
		Value = u32,
	>;

	// App financed record (AppId & proposal_id)
	#[pallet::storage]
	#[pallet::getter(fn app_financed_record)]
	pub(super) type AppFinancedRecord<T: Config> = StorageDoubleMap<
		_,
		Twox64Concat,
		u32,
		Twox64Concat,
		Vec<u8>,
		AppFinancedData<T>,
		ValueQuery,
	>;

	// Last time app financed record key
	#[pallet::storage]
	#[pallet::getter(fn app_financed_last)]
	pub(super) type AppFinancedLast<T: Config> = StorageValue<_, (u32, Vec<u8>), ValueQuery>;

	// App financed user exchange record (AppId & ProposalId & AccountId -> AppFinancedUserExchangeData)
	#[pallet::storage]
	#[pallet::getter(fn app_financed_user_exchange_record)]
	pub(super) type AppFinancedUserExchangeRecord<T: Config> = StorageNMap<
		Key = (
			NMapKey<Twox64Concat, u32>,
			NMapKey<Twox64Concat, Vec<u8>>,
			NMapKey<Twox64Concat, T::AccountId>,
		),
		Value = AppFinancedUserExchangeData<T>,
	>;

	// (AppId & ProposalId) -> user accounts set
	#[pallet::storage]
	#[pallet::getter(fn app_financed_user_exchange_set)]
	pub(super) type AppFinancedUserExchangeSet<T: Config> = StorageDoubleMap<
		_,
		Twox64Concat,
		u32,
		Twox64Concat,
		Vec<u8>,
		Vec<T::AccountId>,
		ValueQuery,
	>;

	// App finance burn total
	#[pallet::storage]
	#[pallet::getter(fn app_financed_burn_total)]
	pub(super) type AppFinancedBurnTotal<T: Config> = StorageValue<_, BalanceOf<T>, ValueQuery>;

	// App finance count
	#[pallet::storage]
	#[pallet::getter(fn app_financed_count)]
	pub(super) type AppFinancedCount<T: Config> = StorageValue<_, u32, ValueQuery>;

	// App commodity(cart_id) count AppId -> u32
	#[pallet::storage]
	#[pallet::getter(fn app_commodity_count)]
	pub(super) type AppCommodityCount<T: Config> =
		StorageMap<_, Twox64Concat, u32, u32, ValueQuery>;

	// App model commodity(cart_id) count (AppId, ModelId) -> u32
	#[pallet::storage]
	#[pallet::getter(fn app_model_commodity_count)]
	pub(super) type AppModelCommodityCount<T: Config> =
		StorageDoubleMap<_, Twox64Concat, u32, Twox64Concat, Vec<u8>, u32, ValueQuery>;

	// Model commodity realtime power leader boards (AppId, ModelId) => Set of board data
	#[pallet::storage]
	#[pallet::getter(fn app_model_commodity_leader_boards)]
	pub(super) type AppModelCommodityLeaderBoards<T: Config> = StorageDoubleMap<
		_,
		Twox64Concat,
		u32,
		Twox64Concat,
		Vec<u8>,
		Vec<CommodityLeaderBoardData<T>>,
		ValueQuery,
	>;

	// AppId, ModelId, cart_id -> ()
	#[pallet::storage]
	#[pallet::getter(fn leader_board_commodity_set)]
	pub(super) type LeaderBoardCommoditySet<T: Config> = StorageNMap<
		Key = (
			NMapKey<Twox64Concat, u32>,
			NMapKey<Twox64Concat, Vec<u8>>,
			NMapKey<Twox64Concat, Vec<u8>>,
		),
		Value = (),
	>;

	// Leader board history records (AppId, ModelId, BlockNumber) => LeaderBoardResult
	#[pallet::storage]
	#[pallet::getter(fn app_leader_board_record)]
	pub(super) type AppLeaderBoardRecord<T: Config> = StorageNMap<
		Key = (
			NMapKey<Twox64Concat, u32>,
			NMapKey<Twox64Concat, Vec<u8>>,
			NMapKey<Twox64Concat, BlockNumberFor<T>>,
		),
		Value = LeaderBoardResult<T>,
	>;

	// Store AppLeaderBoardRcord keys, for load
	#[pallet::storage]
	#[pallet::getter(fn app_leader_board_sequence_keys)]
	pub(super) type AppLeaderBoardSequenceKeys<T: Config> =
		StorageValue<_, Vec<(u32, BlockNumberFor<T>, Vec<u8>)>, ValueQuery>;

	// Leader board last record (AppId, ModelId) -> BlockNumber
	#[pallet::storage]
	#[pallet::getter(fn app_leader_board_last_time)]
	pub(super) type AppLeaderBoardLastTime<T: Config> = StorageDoubleMap<
		_,
		Twox64Concat,
		u32,
		Twox64Concat,
		Vec<u8>,
		BlockNumberFor<T>,
		ValueQuery,
	>;

	// Document comment order pool (AppId, DocumentId) -> Vec<CommentWeightData>
	#[pallet::storage]
	#[pallet::getter(fn document_comments_account_pool)]
	pub(super) type DocumentCommentsAccountPool<T: Config> = StorageDoubleMap<
		_,
		Twox64Concat,
		u32,
		Twox64Concat,
		Vec<u8>,
		Vec<CommentWeightData<T>>,
		ValueQuery,
	>;

	// Account action statistics
	#[pallet::storage]
	#[pallet::getter(fn account_statistics_map)]
	pub(super) type AccountStatisticsMap<T: Config> =
		StorageMap<_, Twox64Concat, T::AccountId, AccountStatistics, ValueQuery>;

	// Account Created Commodity Set (double map appid(cartid))
	#[pallet::storage]
	#[pallet::getter(fn account_commodity_set)]
	pub(super) type AccountCommoditySet<T: Config> = StorageDoubleMap<
		_,
		Twox64Concat,
		T::AccountId,
		Twox64Concat,
		u32,
		Vec<Vec<u8>>,
		ValueQuery,
	>;

	// Account Created Document Set (double map appid(doc id))
	#[pallet::storage]
	#[pallet::getter(fn account_document_set)]
	pub(super) type AccountDocumentSet<T: Config> = StorageDoubleMap<
		_,
		Twox64Concat,
		T::AccountId,
		Twox64Concat,
		u32,
		Vec<Vec<u8>>,
		ValueQuery,
	>;

	// Withdraw records
	#[pallet::storage]
	#[pallet::getter(fn tech_fund_withdraw_records)]
	pub(super) type TechFundWithdrawRecords<T: Config> =
		StorageValue<_, Vec<TechFundWithdrawData<T>>, ValueQuery>;

	// pre-black list of model dispute, this is a collection which reserved balance lower than required 50%
	#[pallet::storage]
	#[pallet::getter(fn model_black_list_pre)]
	pub(super) type ModelPreBlackList<T: Config> =
		StorageValue<_, Vec<(u32, Vec<u8>, T::AccountId, BlockNumberFor<T>)>, ValueQuery>;

	// app_id, model_id -> block number
	#[pallet::storage]
	#[pallet::getter(fn model_slash_cycle_reward_index)]
	pub(super) type ModelSlashCycleRewardIndex<T: Config> = StorageDoubleMap<
		_,
		Twox64Concat,
		u32,
		Twox64Concat,
		Vec<u8>,
		BlockNumberFor<T>,
		ValueQuery,
	>;

	// (app_id, comment_id) -> ModelDisputeRecord
	#[pallet::storage]
	#[pallet::getter(fn model_dispute_records)]
	pub(super) type ModelDisputeRecords<T: Config> = StorageDoubleMap<
		_,
		Twox64Concat,
		u32,
		Twox64Concat,
		Vec<u8>,
		ModelDisputeRecord<T>,
		ValueQuery,
	>;

	// (app_id, comment_id) -> CommoditySlashRecord
	#[pallet::storage]
	#[pallet::getter(fn commodity_slash_records)]
	pub(super) type CommoditySlashRecords<T: Config> = StorageDoubleMap<
		_,
		Twox64Concat,
		u32,
		Twox64Concat,
		Vec<u8>,
		CommoditySlashRecord<T>,
		ValueQuery,
	>;

	/// event
	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {}

	#[pallet::error]
	pub enum Error<T> {}

	#[pallet::genesis_config]
	#[derive(DefaultNoBound)]
	pub struct GenesisConfig<T: Config> {
		#[serde(skip)]
		pub _config: sp_std::marker::PhantomData<T>,
		pub app_id_range: HashMap<Vec<u8>, (u32, BalanceOf<T>, u32, u32, u32)>,
	}

	#[pallet::genesis_build]
	impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
		fn build(&self) {
			// go through the app_id_range and insert into storage
			for (k, v) in &self.app_id_range {
				AppIdRange::<T>::insert(k, v);
			}
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {}

	impl<T: Config> Pallet<T> {}
}
