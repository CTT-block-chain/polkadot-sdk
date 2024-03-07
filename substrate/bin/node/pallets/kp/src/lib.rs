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

use rand_chacha::{
	rand_core::{RngCore, SeedableRng},
	ChaChaRng,
};

use frame_system::pallet_prelude::BlockNumberFor;

use node_primitives::{AuthAccountId, Membership, PowerSize};
use scale_info::TypeInfo;
use sp_core::sr25519;
use sp_runtime::{
	traits::{AccountIdConversion, Hash, TrailingZeroInput, Verify},
	MultiSignature, Perbill, Percent, Permill, RuntimeDebug, SaturatedConversion,
};

use sp_std::cmp::Ordering;
use sp_std::cmp::{max, min};
use sp_std::collections::btree_map::BTreeMap;
use sp_std::ops::Add;
use sp_std::prelude::*;
use std::collections::HashMap;

/// Edit this file to define custom logic or remove it if it is not needed.
/// Learn more about FRAME and the core library of Substrate FRAME pallets:
/// <https://docs.substrate.io/reference/frame-pallets/>
pub use pallet::*;

const FLOAT_COMPUTE_PRECISION: PowerSize = 10000;

type BalanceOf<T> =
	<<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

type NegativeImbalanceOf<T> = <<T as Config>::Currency as Currency<
	<T as frame_system::Config>::AccountId,
>>::NegativeImbalance;

pub type PowerRatioType = (u32, Perbill, u32);

pub trait PowerVote<AccountId> {
	fn account_power_ratio(_account: &AccountId) -> PowerRatioType {
		// default return 1
		(1u32, Perbill::one(), 1u32)
	}

	/// (account power / total power) * 10000
	fn account_power_relative(_account: &AccountId) -> u64 {
		1
	}
}

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

#[derive(Encode, Decode, PartialEq, Clone, Copy, RuntimeDebug, TypeInfo)]
pub struct ModelIncomeCurrentStage<T: Config> {
	pub stage: u8,
	pub left: BlockNumberFor<T>,
}

#[derive(Encode, Decode, PartialEq, Clone, Copy, RuntimeDebug, TypeInfo)]
enum ModelIncomeStage {
	NORMAL,
	COLLECTING,
	REWARDING,
	CONFIRMING,
	COMPENSATING,
}

impl From<ModelIncomeStage> for u8 {
	fn from(orig: ModelIncomeStage) -> Self {
		return match orig {
			ModelIncomeStage::NORMAL => 0,
			ModelIncomeStage::COLLECTING => 1,
			ModelIncomeStage::REWARDING => 2,
			ModelIncomeStage::CONFIRMING => 3,
			ModelIncomeStage::COMPENSATING => 4,
		};
	}
}

type CommodityPowerSet = (DocumentPower, DocumentPower, DocumentPower, PowerSize, PowerSize);

#[derive(Encode, Decode, Clone, Default, PartialEq, RuntimeDebug, TypeInfo)]
pub struct AuthParamsCreateModel {
	model_id: Vec<u8>,
}

#[derive(Encode, Decode, Clone, Default, PartialEq, RuntimeDebugNoBound, TypeInfo)]
#[scale_info(skip_type_params(T))]
pub struct ClientParamsCreateModel<T: Config> {
	app_id: u32,
	expert_id: Vec<u8>,
	commodity_name: Vec<u8>,
	commodity_type: u32,
	content_hash: T::Hash,
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
	pub(super) type KPModelDataByIdHash<T: Config> =
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
			NMapKey<Twox64Concat, BlockNumberFor<T>>,
			NMapKey<Twox64Concat, Vec<u8>>,
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
	pub enum Event<T: Config> {
		KnowledgeCreated { who: T::AccountId },
		CommentCreated { who: T::AccountId },
		ModelCreated { who: T::AccountId },
		ModelOwnerTransfered { who: T::AccountId },
		CommodityTypeCreated { commodity_type: u32 },
		AppModelTotal { total: u32 },
		ModelCycleIncome { who: T::AccountId },
		PowerSlashed { who: T::AccountId },
		AppAdded { app_id: u32 },
		AppFinanced { app_id: u32 },
		LeaderBoardsCreated { block: BlockNumberFor<T>, app_id: u32, model_id: Vec<u8> },
		ModelDisputed { who: T::AccountId },
		AppRedeemed { who: T::AccountId },
		AppFinanceUserExchangeStart { who: T::AccountId, user: T::AccountId },
		AppFinanceUserExchangeConfirmed { who: T::AccountId },
		AppFinanceUserExchangeCompensated { who: T::AccountId },
		AppCycleIncomeUserExchangeConfirmed { who: T::AccountId },
		ModelIncomeRewarded { who: T::AccountId },
		AppCycleIncomeRedeem { who: T::AccountId },
		AppIncomeUserExchangeCompensated { who: T::AccountId },
		TechFundWithdrawed { who: T::AccountId },
		ModelDepositAdded { who: T::AccountId },
	}

	#[pallet::error]
	pub enum Error<T> {
		BalanceNotEnough,
		AddOverflow,
		DocumentAlreadyExisted,
		ProductAlreadyExisted,
		CommentAlreadyExisted,
		ModelAlreadyExisted,
		ModelTypeInvalid,
		ModelNotFoundOrDisabled,
		CommodityTypeExisted,
		ModelOverSizeLimit,
		NotAppAdmin,
		CommentNotFound,
		DocumentNotFound,
		ProductNotFound,
		AppTypeInvalid,
		ReturnRateInvalid,
		AppAdminNotMatchUser,
		AppIdInvalid,
		AppIdReachMax,
		AppAlreadyFinanced,
		AppFinancedLastExchangeNotEnd,
		AppFinancedNotInvestor,
		AppFinancedExchangeRateTooLow,
		AppFinancedParamsInvalid,
		AppFinancedUserExchangeProposalNotExist,
		AppFinancedUserExchangeAlreadyPerformed,
		AppFinancedUserExchangeRecordNotExist,
		AppFinancedUserExchangeOverflow,
		AppFinancedUserExchangeStateWrong,
		AppFinancedUserExchangeEnded,
		AppFinancedUserExchangeConfirmNotEnd,
		AppFinancedUserExchangeConfirmEnded,
		AppFinancedUserExchangeCompensateEnded,
		DocumentIdentifyAlreadyExisted,
		DocumentTryAlreadyExisted,
		LeaderBoardCreateNotPermit,
		AppRedeemTransactionIdRepeat,
		SignVerifyErrorUser,
		SignVerifyErrorAuth,
		AuthIdentityNotAppKey,
		AuthIdentityNotTechMember,
		AuthIdentityNotFinanceMember,
		AuthIdentityNotExpectedFinanceMember,
		ModelCycleIncomeAlreadyExisted,
		ModelCycleRewardAlreadyExisted,
		ModelCycleRewardSlashed,
		ModelIncomeParamsTooLarge,
		ModelIncomeNotInCollectingStage,
		ModelIncomeNotInRewardingStage,
		ModelIncomeNotInConfirmingStage,
		ModelIncomeNotInCompensatingStage,
		ModelIncomeRewardingNotEnd,
		ModelIncomeConfirmingNotEnd,
		ModelCycleIncomeTotalZero,
		ModelCycleIncomeZero,
		AppCycleIncomeZero,
		AppCycleIncomeRateZero,
		NotModelCreator,
		TechFundAmountComputeError,
		CartIdInBlackList,
		NotFoundValidFinanceMember,
	}

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
	impl<T: Config> Pallet<T> {
		#[pallet::call_index(0)]
		#[pallet::weight(0)]
		pub fn create_model(
			origin: OriginFor<T>,
			client_params: ClientParamsCreateModel<T>,
			auth_params: AuthParamsCreateModel,

			app_user_account: AuthAccountId,
			app_user_sign: sr25519::Signature,

			auth_server: AuthAccountId,
			auth_sign: sr25519::Signature,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			ensure!(
				Self::verify_sign(&app_user_account, app_user_sign, &client_params.encode()),
				Error::<T>::SignVerifyErrorUser
			);
			ensure!(
				Self::verify_sign(&auth_server, auth_sign, &auth_params.encode()),
				Error::<T>::SignVerifyErrorAuth
			);

			let ClientParamsCreateModel {
				app_id,
				expert_id,
				commodity_name,
				commodity_type,
				content_hash,
			} = client_params;

			let AuthParamsCreateModel { model_id } = auth_params;

			ensure!(T::Membership::is_valid_app(app_id), Error::<T>::AppIdInvalid);

			// check if valid auth server
			ensure!(
				T::Membership::is_valid_app_key(app_id, &Self::convert_account(&auth_server)),
				Error::<T>::AuthIdentityNotAppKey
			);

			ensure!(
				!<KPModelDataByIdHash<T>>::contains_key(app_id, &model_id),
				Error::<T>::ModelAlreadyExisted
			);

			// print(commodity_type);

			// check if valid commodity_type
			ensure!(
				<CommodityTypeMap<T>>::contains_key(commodity_type),
				Error::<T>::ModelTypeInvalid
			);

			let count = <AppModelCount<T>>::get(app_id);
			let max_models = <AppModelTotalConfig<T>>::get(app_id);
			if max_models > 0 {
				ensure!(count < max_models, Error::<T>::ModelOverSizeLimit);
			}

			// print("checking deposit");
			// deposit
			let user_account = Self::convert_account(&app_user_account);
			let value = T::ModelCreateDeposit::get();
			T::Currency::reserve(&user_account, value)?;
			<KPModelDepositMap<T>>::insert(app_id, &model_id, value);

			// let type_key = T::Hashing::hash_of(&(app_id, commodity_type));
			let should_transfer =
				!<ModelFirstTypeBenefitRecord<T>>::contains_key(app_id, commodity_type);
			let create_reward = T::Membership::set_model_creator(
				app_id,
				&model_id,
				&(Self::convert_account(&app_user_account)),
				should_transfer,
			);

			if should_transfer {
				<ModelFirstTypeBenefitRecord<T>>::insert(app_id, commodity_type, true);
			}

			let model = KPModelData {
				app_id,
				model_id: model_id.clone(),
				expert_id,
				status: ModelStatus::ENABLED,
				commodity_name,
				commodity_type,
				content_hash,
				sender: who.clone(),
				owner: app_user_account,
				create_reward,
			};

			<KPModelDataByIdHash<T>>::insert(app_id, &model_id, &model);
			<AppModelCount<T>>::insert(app_id, count + 1);

			Self::deposit_event(Event::ModelCreated { who });
			Ok(())
		}
	}

	impl<T: Config> Pallet<T> {
		fn verify_sign(pub_key: &AuthAccountId, sign: sr25519::Signature, msg: &[u8]) -> bool {
			let ms: MultiSignature = sign.into();
			ms.verify(msg, &pub_key)
		}

		pub fn kp_total_power() -> PowerSize {
			<TotalPower<T>>::get()
		}

		pub fn kp_account_power(account: T::AccountId) -> PowerSize {
			<MinerPowerByAccount<T>>::get(account)
		}

		fn convert_account(origin: &AuthAccountId) -> T::AccountId {
			let tmp: [u8; 32] = origin.clone().into();
			T::AccountId::decode(&mut &tmp[..]).unwrap()
		}

		pub fn kp_auth_account_power(account: AuthAccountId) -> PowerSize {
			let account_id = Self::convert_account(&account);
			Self::kp_account_power(account_id)
		}

		pub fn power_factor(p: u128) -> PowerRatioType {
			match p {
				0..=100000 => {
					(1u32, Perbill::from_rational_approximation::<u128>(3, 20_0000), p as u32)
				}, //1 + (3 / 20) * x,
				_ => (
					0u32,
					Perbill::from_rational_approximation::<u128>(p + 17_0000u128, p + 162_0000u128),
					16u32,
				), // 16 * (x + 17) / (x + 162),
			}
		}

		pub fn balance_apply_power(b: BalanceOf<T>, factor: PowerRatioType) -> BalanceOf<T> {
			let mut converted: BalanceOf<T>;

			match factor {
				(num, frac, frac_cond) => {
					converted = b * num.into();
					let divided = frac * b;
					converted += divided * frac_cond.into();
				},
			}

			converted
		}

		pub fn kp_account_power_ratio(account: &T::AccountId) -> PowerRatioType {
			let p = <MinerPowerByAccount<T>>::get(account) as u128;
			Self::power_factor(p)
		}

		pub fn kp_account_power_ratio_by_mini(account: &T::AccountId) -> u64 {
			let factor = Self::kp_account_power_ratio(account);
			// 1 unit convert to how much
			let converted = Self::balance_apply_power(T::Currency::minimum_balance(), factor);
			converted.saturated_into::<u64>()
		}

		pub fn kp_staking_to_vote(account: &T::AccountId, stake: BalanceOf<T>) -> BalanceOf<T> {
			Self::balance_apply_power(stake, Self::kp_account_power_ratio(account))
		}

		fn model_income_cycle_index(block: BlockNumberFor<T>) -> BlockNumberFor<T> {
			block / T::ModelIncomeCyclePeriod::get()
		}

		fn model_income_stage(block: BlockNumberFor<T>) -> (ModelIncomeStage, BlockNumberFor<T>) {
			let cycle_index = Self::model_income_cycle_index(block);
			let cycle_blocks = T::ModelIncomeCyclePeriod::get();

			if cycle_index == 0u32.into() {
				return (ModelIncomeStage::NORMAL, cycle_blocks - block);
			}

			let collecting = T::ModelIncomeCollectingPeriod::get();
			let rewarding_blocks = T::ModelIncomeRewardingPeriod::get();
			let confirming_blocks = rewarding_blocks / 2u32.into();
			let compensating_blocks = rewarding_blocks / 2u32.into();
			let progress = block % T::ModelIncomeCyclePeriod::get();

			return if progress < collecting {
				(ModelIncomeStage::COLLECTING, collecting - progress)
			} else if progress < collecting + rewarding_blocks {
				(ModelIncomeStage::REWARDING, collecting + rewarding_blocks - progress)
			} else if progress < collecting + rewarding_blocks + confirming_blocks {
				(
					ModelIncomeStage::CONFIRMING,
					collecting + rewarding_blocks + confirming_blocks - progress,
				)
			} else if progress
				< collecting + rewarding_blocks + confirming_blocks + compensating_blocks
			{
				(
					ModelIncomeStage::COMPENSATING,
					collecting + rewarding_blocks + confirming_blocks + compensating_blocks
						- progress,
				)
			} else {
				(ModelIncomeStage::NORMAL, cycle_blocks - progress)
			};
		}

		pub fn model_income_current_stage() -> ModelIncomeCurrentStage<T> {
			let block = frame_system::Pallet::<T>::block_number();
			let stage = Self::model_income_stage(block);

			ModelIncomeCurrentStage { stage: stage.0.into(), left: stage.1 }
		}

		pub fn app_finance_record(app_id: u32, proposal_id: Vec<u8>) -> AppFinancedData<T> {
			<AppFinancedRecord<T>>::get(app_id, &proposal_id)
		}

		pub fn app_finance_exchange_accounts(
			app_id: u32,
			proposal_id: Vec<u8>,
		) -> Vec<T::AccountId> {
			<AppFinancedUserExchangeSet<T>>::get(app_id, &proposal_id)
		}

		pub fn app_finance_exchange_data(
			app_id: u32,
			proposal_id: Vec<u8>,
			account: T::AccountId,
		) -> AppFinancedUserExchangeData<T> {
			<AppFinancedUserExchangeRecord<T>>::get((app_id, &proposal_id, &account)).unwrap()
		}

		fn compute_commodity_power(power: &CommodityPowerSet) -> PowerSize {
			power.0.total() + power.1.total() + power.2.total() + power.3 + power.4
		}

		fn get_leader_item(
			app_id: u32,
			model_id: &Vec<u8>,
			cart_id: &Vec<u8>,
		) -> Option<(usize, CommodityLeaderBoardData<T>)> {
			if !<LeaderBoardCommoditySet<T>>::contains_key((app_id, model_id, cart_id)) {
				return None;
			} else {
				// go through specified leader board
				let hash = T::Hashing::hash_of(cart_id);
				let board = <AppModelCommodityLeaderBoards<T>>::get(app_id, model_id);
				for (pos, item) in board.iter().enumerate() {
					if item.cart_id_hash == hash {
						return Some((pos, item.clone()));
					}
				}
			}

			None
		}

		fn update_realtime_power_leader_boards(
			app_id: u32,
			model_id: &Vec<u8>,
			cart_id: &Vec<u8>,
			power: PowerSize,
			owner: T::AccountId,
		) -> Option<u32> {
			// get leader board
			let leader_key = T::Hashing::hash_of(&(app_id, model_id));
			let mut board = <AppModelCommodityLeaderBoards<T>>::get(app_id, model_id);

			let board_item = CommodityLeaderBoardData {
				cart_id_hash: T::Hashing::hash_of(cart_id),
				cart_id: cart_id.clone(),
				power,
				owner,
			};

			// check leader set double map to make sure this item is already in leader board
			match Self::get_leader_item(app_id, model_id, cart_id) {
				Some((index, org_item)) => {
					if org_item == board_item {
						// power no change, do nothing
						return Some(0);
					} else {
						// remove old one, reinsert
						board.remove(index);
					}
				},
				None => {},
			}

			// now we can find the proper position for this new power item
			match board.binary_search_by(|probe| probe.cmp(&board_item).reverse()) {
				Ok(index) => {
					// always get the end
					board.insert(index + 1, board_item);
				},
				Err(index) => {
					// not found, index is closet position(upper)
					board.insert(index, board_item);
				},
			}

			// update leader set
			<LeaderBoardCommoditySet<T>>::insert((app_id, model_id, cart_id), ());

			// check if reach board max
			if T::AppLeaderBoardMaxPos::get() < board.len() as u32 {
				// print("leader board full, drop last one");
				let removed = board.pop()?;
				<LeaderBoardCommoditySet<T>>::remove((app_id, model_id, cart_id));
			}

			// update board
			<AppModelCommodityLeaderBoards<T>>::insert(app_id, model_id, &board);

			Some(0)
		}

		fn update_purchase_power(
			power_set: &CommodityPowerSet,
			app_id: u32,
			model_id: &Vec<u8>,
			cart_id: &Vec<u8>,
			owner: &T::AccountId,
		) {
			if <KPPurchaseBlackList<T>>::contains_key(app_id, cart_id) {
				// print("meet slashed purchase commodity");
				return;
			}

			let power = Self::compute_commodity_power(power_set);
			// read out total power
			let mut total_power = Self::kp_total_power();
			let mut account_power = <MinerPowerByAccount<T>>::get(owner);

			// check if this has been added to total power before
			if <KPPurchasePowerByIdHash<T>>::contains_key(app_id, cart_id) {
				let org_power_set = <KPPurchasePowerByIdHash<T>>::get(app_id, cart_id);
				// only add a diff to total power
				let org_power = Self::compute_commodity_power(&org_power_set);

				// for total power
				if total_power >= org_power {
					total_power -= org_power;
				} else {
					// print("process total power unexpected");
					total_power = 0;
				}

				// for account power (account power is collect of user's purchase power sum)
				if account_power >= org_power {
					account_power -= org_power;
				} else {
					// print("process account powser unexpected");
					account_power = 0;
				}
			}

			total_power += power;
			<TotalPower<T>>::put(total_power);
			<KPPurchasePowerByIdHash<T>>::insert(app_id, cart_id, power_set);

			account_power += power;
			<MinerPowerByAccount<T>>::insert(owner, account_power);

			// update model board
			Self::update_realtime_power_leader_boards(
				app_id,
				model_id,
				cart_id,
				power,
				owner.clone(),
			);
			// uupdate app board
			Self::update_realtime_power_leader_boards(
				app_id,
				&vec![],
				cart_id,
				power,
				owner.clone(),
			);
		}

		fn clear_purchase_power(app_id: u32, cart_id: &Vec<u8>) {
			let empty_power = DocumentPower { attend: 0, content: 0, judge: 0 };

			if <KPPurchasePowerByIdHash<T>>::contains_key(app_id, cart_id) {
				let org_power_set = <KPPurchasePowerByIdHash<T>>::get(app_id, cart_id);
				let org_power = Self::compute_commodity_power(&org_power_set);
				let mut total_power = <TotalPower<T>>::get();
				if total_power >= org_power {
					total_power -= org_power;
				} else {
					// print("process total power (slash) unexpected");
					total_power = 0;
				}

				<TotalPower<T>>::put(total_power);
			}

			<KPPurchasePowerByIdHash<T>>::insert(
				app_id,
				cart_id,
				&(empty_power.clone(), empty_power.clone(), empty_power.clone(), 0, 0),
			);
			<KPPurchaseBlackList<T>>::insert(app_id, cart_id, true);
		}

		fn get_model_id_from_product(app_id: u32, product_id: &Vec<u8>) -> Option<Vec<u8>> {
			// get model id from publish doc
			if !<KPDocumentProductIndexByIdHash<T>>::contains_key(app_id, product_id) {
				return None;
			}
			let publish_doc_id = <KPDocumentProductIndexByIdHash<T>>::get(app_id, product_id);
			if !<KPDocumentDataByIdHash<T>>::contains_key(app_id, &publish_doc_id) {
				return None;
			}
			let publish_doc = <KPDocumentDataByIdHash<T>>::get(app_id, &publish_doc_id);

			Some(publish_doc.model_id)
		}

		fn get_pub_docid_from_doc(app_id: u32, doc_id: &Vec<u8>) -> Vec<u8> {
			let doc_key_hash = T::Hashing::hash_of(&(app_id, doc_id));
			let doc = Self::kp_document_data_by_idhash(app_id, doc_id);

			let product_key_hash = T::Hashing::hash_of(&(app_id, &doc.product_id));
			<KPDocumentProductIndexByIdHash<T>>::get(app_id, &doc.product_id)
		}

		fn remove_leader_board_item(
			app_id: u32,
			model_id: &Vec<u8>,
			cart_id: &Vec<u8>,
		) -> Option<u32> {
			let (index, _) = Self::get_leader_item(app_id, model_id, cart_id)?;

			let mut board = <AppModelCommodityLeaderBoards<T>>::get(app_id, model_id);
			board.remove(index);
			<AppModelCommodityLeaderBoards<T>>::insert(app_id, model_id, board);

			Some(0)
		}

		fn compute_publish_product_content_power(
			para_issue_rate: Permill,
			self_issue_rate: Permill,
			attend_rate: Permill,
		) -> PowerSize {
			let mut base = Permill::from_percent(T::TopWeightProductPublish::get() as u32)
				* FLOAT_COMPUTE_PRECISION;

			base = Permill::from_percent(T::DocumentPowerWeightContent::get() as u32) * base;

			let mut sub1 = para_issue_rate * base;
			sub1 = Permill::from_percent(T::DocumentPublishWeightParamsRate::get() as u32) * sub1;

			let mut sub2 = self_issue_rate * base;
			sub2 =
				Permill::from_percent(T::DocumentPublishWeightParamsSelfRate::get() as u32) * sub2;

			let mut sub3 = attend_rate * base;
			sub3 = Permill::from_percent(T::DocumentPublishWeightParamsAttendRate::get() as u32)
				* sub3;

			sub1 + sub2 + sub3
		}

		fn compute_identify_content_power(
			ident_rate: Permill,
			ident_consistence: Permill,
			seller_consistence: Permill,
		) -> PowerSize {
			let mut base = Permill::from_percent(T::TopWeightDocumentIdentify::get() as u32)
				* FLOAT_COMPUTE_PRECISION;

			base = Permill::from_percent(T::DocumentPowerWeightContent::get() as u32) * base;

			let mut sub1 = ident_rate * base;
			sub1 = Permill::from_percent(T::DocumentIdentifyWeightParamsRate::get() as u32) * sub1;

			let mut sub2 = ident_consistence * base;
			sub2 = Permill::from_percent(T::DocumentIdentifyWeightCheckRate::get() as u32) * sub2;

			let mut sub3 = seller_consistence * base;
			sub3 =
				Permill::from_percent(T::DocumentIdentifyWeightConsistentRate::get() as u32) * sub3;

			sub1 + sub2 + sub3
		}

		fn compute_try_content_power(
			offset_rate: Permill,
			true_rate: Permill,
			seller_consistence: Permill,
		) -> PowerSize {
			let mut base = Permill::from_percent(T::TopWeightDocumentTry::get() as u32)
				* FLOAT_COMPUTE_PRECISION;

			base = Permill::from_percent(T::DocumentPowerWeightContent::get() as u32) * base;

			let mut sub1 = offset_rate * base;
			sub1 = Permill::from_percent(T::DocumentTryWeightBiasRate::get() as u32) * sub1;

			let mut sub2 = true_rate * base;
			sub2 = Permill::from_percent(T::DocumentTryWeightTrueRate::get() as u32) * sub2;

			let mut sub3 = seller_consistence * base;
			sub3 = Permill::from_percent(T::DocumentTryWeightConsistentRate::get() as u32) * sub3;

			sub1 + sub2 + sub3
		}

		fn compute_choose_content_power(
			sell_count_rate: Permill,
			try_count_rate: Permill,
		) -> PowerSize {
			let base = Permill::from_percent(T::DocumentCMPowerWeightContent::get() as u32)
				* FLOAT_COMPUTE_PRECISION;

			let mut sub1 = sell_count_rate * base;
			sub1 = Permill::from_percent(T::DocumentChooseWeightSellCount::get() as u32) * sub1;

			let mut sub2 = try_count_rate * base;
			sub2 = Permill::from_percent(T::DocumentChooseWeightTryCount::get() as u32) * sub2;

			sub1 + sub2
		}

		fn compute_model_content_power(
			producer_count_rate: Permill,
			product_count_rate: Permill,
		) -> PowerSize {
			let base = Permill::from_percent(T::DocumentCMPowerWeightContent::get() as u32)
				* FLOAT_COMPUTE_PRECISION;

			let mut sub1 = producer_count_rate * base;
			sub1 = Permill::from_percent(T::DocumentModelWeightProducerCount::get() as u32) * sub1;

			let mut sub2 = product_count_rate * base;
			sub2 = Permill::from_percent(T::DocumentModelWeightProductCount::get() as u32) * sub2;

			sub1 + sub2
		}

		fn compute_attend_power(
			rates: (Permill, Permill, Permill, Permill),
			second_weight: PowerSize,
			top_weight: PowerSize,
		) -> PowerSize {
			let mut base = Permill::from_percent(top_weight as u32) * FLOAT_COMPUTE_PRECISION;

			base = Permill::from_percent(second_weight as u32) * base;

			let mut sub1 = rates.0 * base;
			sub1 = Permill::from_percent(T::CommentPowerWeightCount::get() as u32) * sub1;

			let mut sub2 = rates.1 * base;
			sub2 = Permill::from_percent(T::CommentPowerWeightCost::get() as u32) * sub2;

			let mut sub3 = rates.2 * base;
			sub3 = Permill::from_percent(T::CommentPowerWeightPerCost::get() as u32) * sub3;

			let mut sub4 = rates.3 * base;
			sub4 = Permill::from_percent(T::CommentPowerWeightPositive::get() as u32) * sub4;

			sub1 + sub2 + sub3 + sub4
		}

		fn compute_judge_power(
			origin_power: Permill,
			top_weight: PowerSize,
			document_weight: u8,
		) -> PowerSize {
			let mut base = Permill::from_percent(top_weight as u32) * FLOAT_COMPUTE_PRECISION;
			base = Permill::from_percent(document_weight as u32) * base;

			origin_power * base
		}

		fn compute_price_power(commodity_price: PowerSize) -> PowerSize {
			let max = <MaxGoodsPrice<T>>::get();
			if max == 0 {
				0
			} else {
				let base = Permill::from_percent(T::TopWeightAccountStake::get() as u32)
					* FLOAT_COMPUTE_PRECISION;
				Permill::from_rational_approximation(commodity_price as u32, max as u32) * base
			}
		}

		fn compute_comment_action_rate(
			max: &CommentMaxRecord,
			count: PowerSize,
			fee: PowerSize,
			positive: PowerSize,
			unit_fee: PowerSize,
		) -> (Permill, Permill, Permill, Permill) {
			let mut positive_rate = Permill::from_percent(0);
			let count_rate =
				Permill::from_rational_approximation(count as u32, max.max_count as u32);
			let cost_rate = Permill::from_rational_approximation(fee as u32, max.max_fee as u32);
			let unit_cost_rate =
				Permill::from_rational_approximation(unit_fee as u32, max.max_unit_fee as u32);

			if max.max_positive > 0 {
				positive_rate =
					Permill::from_rational_approximation(positive as u32, max.max_positive as u32);
			}

			(count_rate, cost_rate, unit_cost_rate, positive_rate)
		}

		fn update_max<F>(rate: PowerSize, mut max: PowerSize, updater: F) -> Permill
		where
			F: Fn(PowerSize) -> (),
		{
			if rate > max {
				max = rate;
				updater(max);
			}

			if rate > 0 {
				return Permill::from_rational_approximation(rate, max);
			}

			Permill::from_percent(0)
		}

		fn update_comment_max(
			max: &mut CommentMaxRecord,
			count: PowerSize,
			fees: PowerSize,
			positive: PowerSize,
			unit_fee: PowerSize,
		) -> bool {
			let mut is_updated = false;

			if count > max.max_count {
				max.max_count = count;
				is_updated = true;
			}
			if fees > max.max_fee {
				max.max_fee = fees;
				is_updated = true;
			}
			if positive > max.max_positive {
				max.max_positive = positive;
				is_updated = true;
			}
			if unit_fee > max.max_unit_fee {
				max.max_unit_fee = unit_fee;
				is_updated = true;
			}

			is_updated
		}

		fn compute_doc_trend_power(doc: &KPDocumentData<T>) -> Permill {
			match doc {
				KPDocumentData { expert_trend, platform_trend, .. } => {
					let et = *expert_trend as u8;
					let pt = *platform_trend as u8;

					match et ^ pt {
						// 01 10, 10 01  single negative
						0b11 => Permill::from_percent(25), //0.25,
						// 00 00, 01 01, 10 10
						0b00 => match et & pt {
							0b00 => Permill::from_percent(100), //1.0,
							0b01 => Permill::from_percent(0),   //0.0,
							0b10 => Permill::from_percent(50),  //0.5,
							// unexpected!!!
							_ => {
								//print("unexpected");
								Permill::from_percent(0)
							},
						},
						// 00 01, 01 00 positive and negative
						0b01 => Permill::from_rational_approximation(375u32, 1000u32), //0.375,
						// 00 10, 10 00 single positive
						0b10 => Permill::from_percent(75), //0.75,
						// unexpected!!!
						_ => {
							//print("unexpected");
							Permill::from_percent(0)
						},
					}
				},
			}
		}

		fn increase_commodity_count(
			app_id: u32,
			model_id: &Vec<u8>,
			cart_id: &Vec<u8>,
			doc_type: DocumentType,
			owner_id: &T::AccountId,
		) {
			let update_store = || {
				<AppCommodityCount<T>>::mutate(app_id, |count| {
					*count = *count + 1;
				});
				let model_key = T::Hashing::hash_of(&(app_id, model_id));
				<AppModelCommodityCount<T>>::mutate(app_id, model_id, |count| {
					*count = *count + 1;
				});
				<AccountStatisticsMap<T>>::mutate(owner_id, |info| {
					info.create_commodity_num += 1;
				});

				// update account commodity store record
				let mut owner_cart_ids = <AccountCommoditySet<T>>::get(&owner_id, app_id);
				owner_cart_ids.push(cart_id.clone());

				<AccountCommoditySet<T>>::insert(&owner_id, app_id, owner_cart_ids);
			};

			match doc_type {
				DocumentType::ProductTry => {
					// check if another identify document exist
					if <KPCartProductIdentifyIndexByIdHash<T>>::contains_key(app_id, cart_id) {
						return;
					}
					update_store();
				},
				DocumentType::ProductIdentify => {
					if <KPCartProductTryIndexByIdHash<T>>::contains_key(app_id, cart_id) {
						return;
					}
					update_store();
				},
				_ => {},
			}
		}

		/// if not found match, return closet right
		pub fn binary_search_closet<L: PartialOrd>(collection: &[L], target: &L) -> usize {
			let mut lo: usize = 0;
			let mut hi: usize = collection.len();

			while lo < hi {
				let m = (hi - lo) / 2 + lo;

				if *target == collection[m] {
					return m;
				} else if *target < collection[m] {
					hi = m;
				} else {
					lo = m + 1;
				}
			}
			return lo;
		}

		fn leader_board_lottery(block: BlockNumberFor<T>, app_id: u32, model_id: &Vec<u8>) {
			let (seed, _) = T::Randomness::random(b"ctt_power");
			// seed needs to be guaranteed to be 32 bytes.
			let seed = <[u8; 32]>::decode(&mut TrailingZeroInput::new(seed.as_ref()))
				.expect("input is padded with zeroes; qed");
			let mut rng = ChaChaRng::from_seed(seed);
			let mut pdc_map: BTreeMap<T::Hash, ()> = BTreeMap::new();

			//pick_item(&mut rng, &votes)

			// get border items
			let board: Vec<CommodityLeaderBoardData<T>> =
				<AppModelCommodityLeaderBoards<T>>::get(app_id, model_id);

			if board.len() == 0 {
				// print("board empty");
				return;
			}

			// get this board(appid, model_id) total items count
			let total;
			if model_id.len() == 0 {
				total = <AppCommodityCount<T>>::get(app_id);
			} else {
				total = <AppModelCommodityCount<T>>::get(app_id, model_id);
			}

			if total == 0 {
				// print("total commodity empty");
				return;
			}

			// get board items count
			let count: usize;
			if total <= 5 {
				count = total as usize;
			} else {
				let board_count_max = T::AppLeaderBoardMaxPos::get();
				count = min(board_count_max, total * 20 / 100) as usize;
			}

			// load board leaders
			let leaders: Vec<CommodityLeaderBoardData<T>> = (&board[..count]).to_vec();

			// hit records
			let mut records: Vec<T::AccountId> = vec![];

			// get max comment info
			let max = <CommentMaxInfoPerDocMap<T>>::get(app_id);

			let mut attend_lottery = |doc_id: &Vec<u8>, is_pub: bool| {
				let comment_set = <DocumentCommentsAccountPool<T>>::get(app_id, doc_id);
				// this numbers should be put into config
				let hit_max = min(Percent::from_percent(30) * comment_set.len(), 100);
				let mut weight_pool: Vec<u32> = vec![];
				let mut weight_sum: u32 = 0;
				// go through comment set to compute lottery weight
				for comment_data in &comment_set {
					let mut weight1 = Percent::from_rational_approximation(
						comment_data.cash_cost as u32,
						max.max_fee as u32,
					) * 100u32;
					weight1 = Percent::from_percent(88) * weight1;
					let mut weight2 = Percent::from_rational_approximation(
						comment_data.position as u32,
						max.max_count as u32,
					) * 100u32;
					weight2 = Percent::from_percent(8) * weight2;

					if is_pub {
						weight1 = Percent::from_percent(50) * weight1;
						weight2 = Percent::from_percent(50) * weight2;
					}
					let weight = weight1 + weight2;
					weight_pool.push(weight);
					weight_sum += weight;

					// print(weight);
				}

				// now we got all weight info, use weight info array to setup chance array
				// count positions
				let position_count = 100u32;
				let attender_len = weight_pool.len();
				for i in 0..attender_len {
					let attender_weight = weight_pool[i];
					let chance_count =
						Percent::from_rational_approximation(attender_weight, weight_sum)
							* position_count;
					// now weight transfered to position count
					if i == 0 {
						weight_pool[i] = chance_count;
					} else {
						weight_pool[i] = weight_pool[i - 1] + chance_count;
					}
				}

				// now we got total reassigned position info
				let total_positions = *weight_pool.last().unwrap_or(&0);
				//print("total_positions");
				//print(total_positions);
				// start lottery
				for _l in 0..hit_max {
					let pos = pick_usize(&mut rng, total_positions as usize) as u32;
					//print("hit pos:");
					//print(pos);
					// now check which chance hit, binary search weight_pool
					let mut closet_index = Self::binary_search_closet(&weight_pool, &pos);
					if weight_pool[closet_index] <= pos && closet_index < hit_max - 1 {
						// always take is right neighbor
						closet_index += 1;
					}
					//print("hit index:");
					//print(closet_index);
					records.push(comment_set[closet_index].account.clone());
				}
			};

			for index in 0..count {
				let board_item = &leaders[index];
				// read out comment account set
				let key = T::Hashing::hash_of(&(app_id, &board_item.cart_id));
				// check which commodity document exist
				if <KPCartProductIdentifyIndexByIdHash<T>>::contains_key(
					app_id,
					&board_item.cart_id,
				) {
					let doc_id =
						<KPCartProductIdentifyIndexByIdHash<T>>::get(app_id, &board_item.cart_id);
					attend_lottery(&doc_id, false);
					// check publish doc
					let pub_doc_id = Self::get_pub_docid_from_doc(app_id, &doc_id);
					let id_hash = T::Hashing::hash_of(&pub_doc_id);
					if !pdc_map.contains_key(&id_hash) {
						attend_lottery(&pub_doc_id, true);
						pdc_map.insert(id_hash, ());
					}
				}

				if <KPCartProductTryIndexByIdHash<T>>::contains_key(app_id, &board_item.cart_id) {
					let doc_id =
						<KPCartProductTryIndexByIdHash<T>>::get(app_id, &board_item.cart_id);
					attend_lottery(&doc_id, false);
					let pub_doc_id = Self::get_pub_docid_from_doc(app_id, &doc_id);
					let id_hash = T::Hashing::hash_of(&pub_doc_id);
					if !pdc_map.contains_key(&id_hash) {
						attend_lottery(&pub_doc_id, true);
						pdc_map.insert(id_hash, ());
					}
				}
			}

			//print("lottery done");
			//print(records.len());

			// convert leader data to RPC query required
			let mut leader_rpc_data: Vec<LeaderBoardItem<T>> = vec![];

			for item in leaders {
				leader_rpc_data.push(LeaderBoardItem {
					cart_id: item.cart_id.clone(),
					power: item.power,
					owner: item.owner.clone(),
				});
			}

			// write this time record
			let record = LeaderBoardResult { board: leader_rpc_data, accounts: records };
			<AppLeaderBoardRecord<T>>::insert((app_id, block, model_id), &record);
			<AppLeaderBoardLastTime<T>>::insert(app_id, model_id, block);
			// update sequence keys
			let mut keys = <AppLeaderBoardSequenceKeys<T>>::get();
			keys.push((app_id, block, model_id.clone()));
			<AppLeaderBoardSequenceKeys<T>>::put(keys);
		}

		fn update_document_comment_pool(new_comment: &KPCommentData<T>, doc: &KPDocumentData<T>) {
			let mut pool = <DocumentCommentsAccountPool<T>>::get(doc.app_id, &doc.document_id);

			let pool_item = CommentWeightData {
				account: new_comment.sender.clone(),
				position: doc.comment_count,
				cash_cost: new_comment.comment_fee,
			};

			pool.push(pool_item);
			<DocumentCommentsAccountPool<T>>::insert(doc.app_id, &doc.document_id, pool);
		}

		fn update_max_goods_price(price: PowerSize) {
			let current_max = <MaxGoodsPrice<T>>::get();
			if price > current_max {
				<MaxGoodsPrice<T>>::put(price);
			}
		}

		fn insert_document_power(
			doc: &KPDocumentData<T>,
			content_power: PowerSize,
			judge_power: PowerSize,
		) {
			let power = DocumentPower { attend: 0, content: content_power, judge: judge_power };

			<KPDocumentPowerByIdHash<T>>::insert(doc.app_id, &doc.document_id, &power);
		}

		fn update_document_power(
			doc: &KPDocumentData<T>,
			attend_power: PowerSize,
			judge_power: PowerSize,
			content_power: PowerSize,
		) -> Option<u32> {
			// read out original first
			let mut org_power = <KPDocumentPowerByIdHash<T>>::get(doc.app_id, &doc.document_id);

			if attend_power > 0 {
				org_power.attend = attend_power;
			}

			if judge_power > 0 {
				org_power.judge = judge_power;
			}

			if content_power > 0 {
				org_power.content = content_power;
			}

			// update store
			<KPDocumentPowerByIdHash<T>>::insert(doc.app_id, &doc.document_id, org_power);

			Some(0)
		}

		fn process_publish_doc_content_refer_power(
			app_id: u32,
			product_id: &Vec<u8>,
			refer_increased: PowerSize,
		) {
			let publish_doc_id = <KPDocumentProductIndexByIdHash<T>>::get(app_id, product_id);

			// read out doc
			let mut doc = <KPDocumentDataByIdHash<T>>::get(app_id, &publish_doc_id);
			match doc.document_data {
				DocumentSpecificData::ProductPublish(mut data) => {
					data.refer_count += refer_increased;
					doc.document_data = DocumentSpecificData::ProductPublish(data);
					// check if need to update max
					let params_max = <DocumentPublishMaxParams<T>>::get(doc.app_id);
					let para_issue_rate_p =
						Self::update_max(data.para_issue_rate, params_max.para_issue_rate, |v| {
							<DocumentPublishMaxParams<T>>::mutate(app_id, |max| {
								max.para_issue_rate = v;
							})
						});

					let self_issue_rate_p =
						Self::update_max(data.self_issue_rate, params_max.self_issue_rate, |v| {
							<DocumentPublishMaxParams<T>>::mutate(app_id, |max| {
								max.self_issue_rate = v;
							})
						});

					let attend_rate_p =
						Self::update_max(data.refer_count, params_max.refer_count, |v| {
							<DocumentPublishMaxParams<T>>::mutate(app_id, |max| {
								max.refer_count = v;
							})
						});

					// compute power
					let content_power = Self::compute_publish_product_content_power(
						para_issue_rate_p,
						self_issue_rate_p,
						attend_rate_p,
					);

					Self::update_document_power(&doc, 0, 0, content_power);
				},
				_ => {},
			}

			// update store
			<KPDocumentDataByIdHash<T>>::insert(app_id, &publish_doc_id, &doc);
		}

		// only invoked when creating document
		fn process_document_content_power(doc: &KPDocumentData<T>) {
			let content_power;
			let initial_judge_power;

			match &doc.document_data {
				DocumentSpecificData::ProductPublish(data) => {
					let params_max = <DocumentPublishMaxParams<T>>::get(doc.app_id);
					let para_issue_rate_p =
						Self::update_max(data.para_issue_rate, params_max.para_issue_rate, |v| {
							<DocumentPublishMaxParams<T>>::mutate(doc.app_id, |max| {
								max.para_issue_rate = v;
							})
						});

					let self_issue_rate_p =
						Self::update_max(data.self_issue_rate, params_max.self_issue_rate, |v| {
							<DocumentPublishMaxParams<T>>::mutate(doc.app_id, |max| {
								max.self_issue_rate = v;
							})
						});

					let attend_rate_p =
						Self::update_max(data.refer_count, params_max.refer_count, |v| {
							<DocumentPublishMaxParams<T>>::mutate(doc.app_id, |max| {
								max.refer_count = v;
							})
						});

					// compute power
					content_power = Self::compute_publish_product_content_power(
						para_issue_rate_p,
						self_issue_rate_p,
						attend_rate_p,
					);

					initial_judge_power = Self::compute_judge_power(
						Self::compute_doc_trend_power(&doc),
						T::TopWeightProductPublish::get() as PowerSize,
						T::DocumentPowerWeightJudge::get(),
					);
					Self::insert_document_power(&doc, content_power, initial_judge_power);
				},
				DocumentSpecificData::ProductIdentify(data) => {
					let params_max = <DocumentIdentifyMaxParams<T>>::get(doc.app_id);
					let ident_rate_p =
						Self::update_max(data.ident_rate, params_max.ident_rate, |v| {
							<DocumentIdentifyMaxParams<T>>::mutate(doc.app_id, |max| {
								max.ident_rate = v;
							})
						});

					let ident_consistence_p = Self::update_max(
						data.ident_consistence,
						params_max.ident_consistence,
						|v| {
							<DocumentIdentifyMaxParams<T>>::mutate(doc.app_id, |max| {
								max.ident_consistence = v;
							})
						},
					);

					let seller_consistence_p = Self::update_max(
						data.seller_consistence,
						params_max.seller_consistence,
						|v| {
							<DocumentIdentifyMaxParams<T>>::mutate(doc.app_id, |max| {
								max.seller_consistence = v;
							})
						},
					);

					content_power = Self::compute_identify_content_power(
						ident_rate_p,
						ident_consistence_p,
						seller_consistence_p,
					);

					initial_judge_power = Self::compute_judge_power(
						Self::compute_doc_trend_power(&doc),
						T::TopWeightDocumentIdentify::get() as PowerSize,
						T::DocumentPowerWeightJudge::get(),
					);

					Self::update_max_goods_price(data.goods_price);
					Self::insert_document_power(&doc, content_power, initial_judge_power);
					Self::process_publish_doc_content_refer_power(doc.app_id, &doc.product_id, 1);
				},
				DocumentSpecificData::ProductTry(data) => {
					let params_max = <DocumentTryMaxParams<T>>::get(doc.app_id);
					let offset_rate_p =
						Self::update_max(data.offset_rate, params_max.offset_rate, |v| {
							<DocumentTryMaxParams<T>>::mutate(doc.app_id, |max| {
								max.offset_rate = v;
							})
						});

					let true_rate_p = Self::update_max(data.true_rate, params_max.true_rate, |v| {
						<DocumentTryMaxParams<T>>::mutate(doc.app_id, |max| {
							max.true_rate = v;
						})
					});

					let seller_consistence_p = Self::update_max(
						data.seller_consistence,
						params_max.seller_consistence,
						|v| {
							<DocumentTryMaxParams<T>>::mutate(doc.app_id, |max| {
								max.seller_consistence = v;
							})
						},
					);

					content_power = Self::compute_try_content_power(
						offset_rate_p,
						true_rate_p,
						seller_consistence_p,
					);

					initial_judge_power = Self::compute_judge_power(
						Self::compute_doc_trend_power(&doc),
						T::TopWeightDocumentTry::get() as PowerSize,
						T::DocumentPowerWeightJudge::get(),
					);

					Self::update_max_goods_price(data.goods_price);
					Self::insert_document_power(&doc, content_power, initial_judge_power);
					Self::process_publish_doc_content_refer_power(doc.app_id, &doc.product_id, 1);
				},
				DocumentSpecificData::ProductChoose(data) => {
					let params_max = <DocumentChooseMaxParams<T>>::get(doc.app_id);
					let sell_count_p =
						Self::update_max(data.sell_count, params_max.sell_count, |v| {
							<DocumentChooseMaxParams<T>>::mutate(doc.app_id, |max| {
								max.sell_count = v;
							})
						});

					let try_count_p = Self::update_max(data.try_count, params_max.try_count, |v| {
						<DocumentChooseMaxParams<T>>::mutate(doc.app_id, |max| {
							max.try_count = v;
						})
					});

					content_power = Self::compute_choose_content_power(sell_count_p, try_count_p);

					initial_judge_power = Self::compute_judge_power(
						Self::compute_doc_trend_power(&doc),
						100 as PowerSize,
						T::DocumentCMPowerWeightJudge::get(),
					);
					Self::insert_document_power(&doc, content_power, initial_judge_power);
				},
				DocumentSpecificData::ModelCreate(data) => {
					let params_max = <DocumentModelCreateMaxParams<T>>::get(doc.app_id);
					let producer_count_p =
						Self::update_max(data.producer_count, params_max.producer_count, |v| {
							<DocumentModelCreateMaxParams<T>>::mutate(doc.app_id, |max| {
								max.producer_count = v;
							})
						});

					let product_count_p =
						Self::update_max(data.product_count, params_max.product_count, |v| {
							<DocumentModelCreateMaxParams<T>>::mutate(doc.app_id, |max| {
								max.product_count = v;
							})
						});

					content_power =
						Self::compute_model_content_power(producer_count_p, product_count_p);

					initial_judge_power = Self::compute_judge_power(
						Self::compute_doc_trend_power(&doc),
						100 as PowerSize,
						T::DocumentCMPowerWeightJudge::get(),
					);
					Self::insert_document_power(&doc, content_power, initial_judge_power);
				},
			}

			// update account document store record
			let owner_account = Self::convert_account(&doc.owner);
			let mut owner_doc_ids = <AccountDocumentSet<T>>::get(&owner_account, doc.app_id);
			owner_doc_ids.push(doc.document_id.clone());

			<AccountDocumentSet<T>>::insert(&owner_account, doc.app_id, owner_doc_ids);
		}

		fn give_comment_reward(is_normal: bool, owner: &T::AccountId, cost: u64) {
			let rate = if is_normal {
				Permill::from_percent(T::CommentRewardNormalRate::get())
			} else {
				Permill::from_percent(T::CommentRewardExpertRate::get())
			};

			// convert cost to dollar
			let mut amount: BalanceOf<T> = ((cost / 100) as u32).into();
			amount *= 1_000_000_000_000u64.saturated_into();
			amount = rate * amount;

			let treasury_account: T::AccountId =
				T::TreasuryModuleId::get().into_account_truncating();
			T::Currency::transfer(&treasury_account, owner, amount, KeepAlive).ok();
		}

		fn add_model_dispute_record(
			app_id: u32,
			model_id: &Vec<u8>,
			comment_id: &Vec<u8>,
			dispute_type: ModelDisputeType,
		) {
			let record = ModelDisputeRecord {
				app_id,
				model_id: model_id.clone(),
				comment_id: comment_id.clone(),
				dispute_type,
				block: frame_system::Pallet::<T>::block_number(),
			};

			<ModelDisputeRecords<T>>::insert(app_id, comment_id, record);
		}

		fn add_commodity_power_slash_record(app_id: u32, comment_id: &Vec<u8>, cart_id: &Vec<u8>) {
			let record = CommoditySlashRecord {
				app_id,
				comment_id: comment_id.clone(),
				cart_id: cart_id.clone(),
				block: frame_system::Pallet::<T>::block_number(),
			};

			<CommoditySlashRecords<T>>::insert(app_id, comment_id, record);
		}
	}
}

impl<T: Config> PowerVote<T::AccountId> for Pallet<T> {
	fn account_power_ratio(account: &T::AccountId) -> PowerRatioType {
		Self::kp_account_power_ratio(account)
	}

	fn account_power_relative(account: &T::AccountId) -> u64 {
		/*let total = Self::kp_total_power();
		if total == 0 {
			return 0;
		}*/

		let power = <MinerPowerByAccount<T>>::get(account);
		max(power, 1)
		//Permill::from_rational_approximation(power, total) * 10000
	}
}

/// Pick an item at pseudo-random from the slice, given the `rng`. `None` iff the slice is empty.
fn pick_item<'a, R: RngCore, T>(rng: &mut R, items: &'a [T]) -> Option<&'a T> {
	if items.is_empty() {
		None
	} else {
		Some(&items[pick_usize(rng, items.len() - 1)])
	}
}

/// Pick a new PRN, in the range [0, `max`] (inclusive).
fn pick_usize<'a, R: RngCore>(rng: &mut R, max: usize) -> usize {
	(rng.next_u32() % (max as u32 + 1)) as usize
}
