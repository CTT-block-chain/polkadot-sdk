#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
//use sp_std::vec::Vec;
use frame_support::{
	ensure,
	traits::{
		Contains, Currency, EnsureOrigin, ExistenceRequirement::KeepAlive, Get, OnUnbalanced,
		Randomness, ReservableCurrency, WithdrawReasons,
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

#[derive(Encode, Decode, Clone, PartialEq, RuntimeDebug, TypeInfo)]
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

impl<T: Config> Default for AppFinancedUserExchangeData<T> {
	fn default() -> Self {
		AppFinancedUserExchangeData {
			exchange_amount: BalanceOf::<T>::default(),
			status: 0,
			pay_id: Vec::new(),
		}
	}
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

#[derive(Encode, Decode, Clone, Default, PartialEq, RuntimeDebugNoBound, TypeInfo)]
#[scale_info(skip_type_params(T))]
pub struct ModelKeyParams {
	app_id: u32,
	model_id: Vec<u8>,
}

#[derive(Encode, Decode, Clone, Default, PartialEq, RuntimeDebugNoBound, TypeInfo)]
#[scale_info(skip_type_params(T))]
pub struct ClientParamsCreatePublishDoc<T: Config> {
	app_id: u32,
	document_id: Vec<u8>,
	model_id: Vec<u8>,
	product_id: Vec<u8>,
	content_hash: T::Hash,
	para_issue_rate: PowerSize,
	self_issue_rate: PowerSize,
}

#[derive(Encode, Decode, Clone, Default, PartialEq, RuntimeDebugNoBound, TypeInfo)]
#[scale_info(skip_type_params(T))]
pub struct ClientParamsCreateIdentifyDoc<T: Config> {
	app_id: u32,
	document_id: Vec<u8>,
	product_id: Vec<u8>,
	content_hash: T::Hash,
	goods_price: PowerSize,
	ident_rate: PowerSize,
	ident_consistence: PowerSize,
	seller_consistence: PowerSize,
	cart_id: Vec<u8>,
}

#[derive(Encode, Decode, Clone, Default, PartialEq, RuntimeDebugNoBound, TypeInfo)]
#[scale_info(skip_type_params(T))]
pub struct ClientParamsCreateTryDoc<T: Config> {
	app_id: u32,
	document_id: Vec<u8>,
	product_id: Vec<u8>,
	content_hash: T::Hash,
	goods_price: PowerSize,
	offset_rate: PowerSize,
	true_rate: PowerSize,
	seller_consistence: PowerSize,
	cart_id: Vec<u8>,
}

#[derive(Encode, Decode, Clone, Default, PartialEq, RuntimeDebugNoBound, TypeInfo)]
#[scale_info(skip_type_params(T))]
pub struct ClientParamsCreateChooseDoc<T: Config> {
	app_id: u32,
	document_id: Vec<u8>,
	model_id: Vec<u8>,
	product_id: Vec<u8>,
	content_hash: T::Hash,
	sell_count: PowerSize,
	try_count: PowerSize,
}

#[derive(Encode, Decode, Clone, Default, PartialEq, RuntimeDebugNoBound, TypeInfo)]
#[scale_info(skip_type_params(T))]
pub struct ClientParamsCreateModelDoc<T: Config> {
	app_id: u32,
	document_id: Vec<u8>,
	model_id: Vec<u8>,
	product_id: Vec<u8>,
	content_hash: T::Hash,
	producer_count: PowerSize,
	product_count: PowerSize,
}

#[derive(Encode, Decode, Clone, PartialEq, RuntimeDebugNoBound, TypeInfo)]
#[scale_info(skip_type_params(T))]
pub struct CommentData<T: Config> {
	app_id: u32,
	document_id: Vec<u8>,
	comment_id: Vec<u8>,
	comment_hash: T::Hash,
	comment_fee: PowerSize,
	comment_trend: u8,
}

#[derive(Encode, Decode, Clone, Default, PartialEq, RuntimeDebugNoBound, TypeInfo)]
pub struct ModelIncomeCollectingParam {
	app_id: u32,
	model_ids: Vec<Vec<u8>>,
	incomes: Vec<u64>,
}

#[derive(Encode, Decode, Clone, Default, PartialEq, RuntimeDebugNoBound, TypeInfo)]
#[scale_info(skip_type_params(T))]
pub struct AppIncomeRedeemParams<T: Config> {
	account: T::AccountId,
	app_id: u32,
	cycle: BlockNumberFor<T>,
	exchange_amount: BalanceOf<T>,
}

#[derive(Encode, Decode, Clone, Default, PartialEq, RuntimeDebugNoBound, TypeInfo)]
#[scale_info(skip_type_params(T))]
pub struct AppIncomeRedeemConfirmParams<T: Config> {
	account: T::AccountId,
	app_id: u32,
	pay_id: Vec<u8>,
	cycle: BlockNumberFor<T>,
}

#[derive(Encode, Decode, Clone, Default, PartialEq, RuntimeDebugNoBound, TypeInfo)]
#[scale_info(skip_type_params(T))]
pub struct AddAppParams<T: Config> {
	app_type: Vec<u8>,
	app_name: Vec<u8>,
	app_key: T::AccountId,
	app_admin_key: T::AccountId,
	return_rate: u32,
}

#[derive(Encode, Decode, Clone, Default, PartialEq, RuntimeDebugNoBound, TypeInfo)]
#[scale_info(skip_type_params(T))]
pub struct AppFinancedProposalParams<T: Config> {
	account: T::AccountId,
	app_id: u32,
	proposal_id: Vec<u8>,
	exchange: BalanceOf<T>,
	amount: BalanceOf<T>,
}

#[derive(Encode, Decode, Clone, Default, PartialEq, RuntimeDebugNoBound, TypeInfo)]
#[scale_info(skip_type_params(T))]
pub struct AppFinancedUserExchangeParams<T: Config> {
	account: T::AccountId,
	app_id: u32,
	proposal_id: Vec<u8>,
	exchange_amount: BalanceOf<T>,
}

#[derive(Encode, Decode, Clone, Default, PartialEq, RuntimeDebugNoBound, TypeInfo)]
#[scale_info(skip_type_params(T))]
pub struct AppFinancedUserExchangeConfirmParams<T: Config> {
	account: T::AccountId,
	app_id: u32,
	pay_id: Vec<u8>,
	proposal_id: Vec<u8>,
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
		T::AccountId,
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
		Key = (
			NMapKey<Twox64Concat, u32>,
			NMapKey<Twox64Concat, BlockNumberFor<T>>,
			NMapKey<Twox64Concat, T::AccountId>,
		),
		Value = AppFinancedUserExchangeData<T>,
	>;

	// (AppId & cycle index) -> user accounts set
	#[pallet::storage]
	#[pallet::getter(fn app_cycle_income_exchange_set)]
	pub(super) type AppCycleIncomeExchangeSet<T: Config> = StorageDoubleMap<
		_,
		Twox64Concat,
		u32,
		Twox64Concat,
		BlockNumberFor<T>,
		Vec<T::AccountId>,
		ValueQuery,
	>;

	// (AppId & cycle index) -> this cycle finance member account
	#[pallet::storage]
	#[pallet::getter(fn app_cycle_income_finance_member)]
	pub(super) type AppCycleIncomeFinanceMember<T: Config> = StorageDoubleMap<
		_,
		Twox64Concat,
		u32,
		Twox64Concat,
		BlockNumberFor<T>,
		Option<T::AccountId>,
		ValueQuery,
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

		#[pallet::call_index(1)]
		#[pallet::weight(0)]
		pub fn model_owner_release(
			origin: OriginFor<T>,
			params: ModelKeyParams,
			app_user_account: AuthAccountId,
			app_user_sign: sr25519::Signature,

			auth_server: AuthAccountId,
			auth_sign: sr25519::Signature,
		) -> DispatchResult {
			let _who = ensure_signed(origin)?;

			let encode = params.encode();
			ensure!(
				Self::verify_sign(&app_user_account, app_user_sign, &encode),
				Error::<T>::SignVerifyErrorUser
			);
			ensure!(
				Self::verify_sign(&auth_server, auth_sign, &encode),
				Error::<T>::SignVerifyErrorAuth
			);

			let ModelKeyParams { app_id, model_id } = params;

			let owner = Self::convert_account(&app_user_account);
			ensure!(
				T::Membership::is_model_creator(&owner, app_id, &model_id),
				Error::<T>::NotModelCreator
			);
			// check if valid auth server
			let admin = Self::convert_account(&auth_server);
			ensure!(T::Membership::is_app_admin(&admin, app_id), Error::<T>::NotAppAdmin);

			// check if model valid
			ensure!(Self::is_valid_model(app_id, &model_id), Error::<T>::ModelNotFoundOrDisabled);

			let reserve_amount = <KPModelDepositMap<T>>::get(app_id, &model_id);
			// reserver app admin
			T::Currency::reserve(&admin, reserve_amount)?;

			// transfer owner
			T::Membership::transfer_model_owner(app_id, &model_id, &admin);

			// release owner's
			T::Currency::unreserve(&owner, reserve_amount);

			// update model data store
			<KPModelDataByIdHash<T>>::mutate(app_id, &model_id, |model| {
				model.owner = auth_server;
			});

			Self::deposit_event(Event::ModelOwnerTransfered { who: owner });
			Ok(())
		}

		#[pallet::call_index(2)]
		#[pallet::weight(0)]
		pub fn add_model_deposit(
			origin: OriginFor<T>,
			app_id: u32,
			model_id: Vec<u8>,
			amount: BalanceOf<T>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			// make sure who is model creator
			ensure!(
				T::Membership::is_model_creator(&who, app_id, &model_id),
				Error::<T>::NotModelCreator
			);

			// add deposit
			T::Currency::reserve(&who, amount)?;
			// update record
			<KPModelDepositMap<T>>::mutate(app_id, &model_id, |value| {
				*value += amount;
			});

			Self::deposit_event(Event::ModelDepositAdded { who });
			Ok(())
		}

		#[pallet::call_index(3)]
		#[pallet::weight(0)]
		pub fn create_product_publish_document(
			origin: OriginFor<T>,
			client_params: ClientParamsCreatePublishDoc<T>,

			app_user_account: AuthAccountId,
			app_user_sign: sr25519::Signature,

			auth_server: AuthAccountId,
			auth_sign: sr25519::Signature,
		) -> DispatchResult {
			// Check it was signed and get the signer. See also: ensure_root and ensure_none
			let who = ensure_signed(origin)?;

			let encode = client_params.encode();
			ensure!(
				Self::verify_sign(&app_user_account, app_user_sign, &encode),
				Error::<T>::SignVerifyErrorUser
			);
			ensure!(
				Self::verify_sign(&auth_server, auth_sign, &encode),
				Error::<T>::SignVerifyErrorAuth
			);

			let ClientParamsCreatePublishDoc {
				app_id,
				document_id,
				model_id,
				product_id,
				content_hash,
				para_issue_rate,
				self_issue_rate,
			} = client_params;

			ensure!(T::Membership::is_valid_app(app_id), Error::<T>::AppIdInvalid);

			// check if valid auth server
			ensure!(
				T::Membership::is_valid_app_key(app_id, &Self::convert_account(&auth_server)),
				Error::<T>::AuthIdentityNotAppKey
			);

			//let doc_key_hash = T::Hashing::hash_of(&(app_id, &document_id));
			ensure!(
				!<KPDocumentDataByIdHash<T>>::contains_key(app_id, &document_id),
				Error::<T>::DocumentAlreadyExisted
			);

			// extract percent rates data

			// Validation checks:
			// check if product_id already existed
			//let product_key_hash = T::Hashing::hash_of(&(app_id, &product_id));
			ensure!(
				!<KPDocumentProductIndexByIdHash<T>>::contains_key(app_id, &product_id),
				Error::<T>::ProductAlreadyExisted
			);

			// check if model valid
			ensure!(Self::is_valid_model(app_id, &model_id), Error::<T>::ModelNotFoundOrDisabled);

			let doc = KPDocumentData {
				sender: who.clone(),
				owner: app_user_account.clone(),
				document_type: DocumentType::ProductPublish,
				app_id,
				document_id: document_id.clone(),
				model_id,
				product_id: product_id.clone(),
				content_hash,
				document_data: DocumentSpecificData::ProductPublish(KPProductPublishData {
					para_issue_rate,
					self_issue_rate,
					refer_count: 0,
				}),
				..Default::default()
			};

			Self::process_document_content_power(&doc);
			Self::process_commodity_power(&doc);

			// create document record
			<KPDocumentDataByIdHash<T>>::insert(app_id, &document_id, &doc);

			// create product id -> document id record
			<KPDocumentProductIndexByIdHash<T>>::insert(app_id, &product_id, &document_id);

			Self::deposit_event(Event::KnowledgeCreated { who });
			Ok(())
		}

		#[pallet::call_index(4)]
		#[pallet::weight(0)]
		pub fn create_product_identify_document(
			origin: OriginFor<T>,
			client_params: ClientParamsCreateIdentifyDoc<T>,

			app_user_account: AuthAccountId,
			app_user_sign: sr25519::Signature,

			auth_server: AuthAccountId,
			auth_sign: sr25519::Signature,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let encode = client_params.encode();
			ensure!(
				Self::verify_sign(&app_user_account, app_user_sign, &encode),
				Error::<T>::SignVerifyErrorUser
			);
			ensure!(
				Self::verify_sign(&auth_server, auth_sign, &encode),
				Error::<T>::SignVerifyErrorAuth
			);

			let ClientParamsCreateIdentifyDoc {
				app_id,
				document_id,
				product_id,
				content_hash,
				goods_price,
				ident_rate,
				ident_consistence,
				seller_consistence,
				cart_id,
			} = client_params;

			ensure!(T::Membership::is_valid_app(app_id), Error::<T>::AppIdInvalid);

			// check if valid auth server
			ensure!(
				T::Membership::is_valid_app_key(app_id, &Self::convert_account(&auth_server)),
				Error::<T>::AuthIdentityNotAppKey
			);

			//let doc_key_hash = T::Hashing::hash_of(&(app_id, &document_id));
			ensure!(
				!<KPDocumentDataByIdHash<T>>::contains_key(app_id, &document_id),
				Error::<T>::DocumentAlreadyExisted
			);

			//let product_key_hash = T::Hashing::hash_of(&(app_id, &product_id));
			ensure!(
				<KPDocumentProductIndexByIdHash<T>>::contains_key(app_id, &product_id),
				Error::<T>::ProductNotFound
			);

			//let key = T::Hashing::hash_of(&(app_id, &cart_id));
			ensure!(
				!<KPCartProductIdentifyIndexByIdHash<T>>::contains_key(app_id, &cart_id),
				Error::<T>::DocumentIdentifyAlreadyExisted
			);

			let model_id = Self::get_model_id_from_product(app_id, &product_id).unwrap_or_default();

			// create doc
			let doc = KPDocumentData {
				sender: who.clone(),
				owner: app_user_account.clone(),
				document_type: DocumentType::ProductIdentify,
				app_id,
				document_id: document_id.clone(),
				model_id,
				product_id,
				content_hash,
				document_data: DocumentSpecificData::ProductIdentify(KPProductIdentifyData {
					goods_price,
					ident_rate,
					ident_consistence,
					seller_consistence,
					cart_id: cart_id.clone(),
				}),
				..Default::default()
			};

			// process content power
			Self::process_document_content_power(&doc);
			Self::process_commodity_power(&doc);

			// create cartid -> product identify document id record
			<KPCartProductIdentifyIndexByIdHash<T>>::insert(app_id, &cart_id, &document_id);

			// create document record
			<KPDocumentDataByIdHash<T>>::insert(app_id, &document_id, &doc);

			Self::increase_commodity_count(
				app_id,
				&doc.model_id,
				&cart_id,
				DocumentType::ProductIdentify,
				&Self::convert_account(&doc.owner),
			);

			Self::deposit_event(Event::KnowledgeCreated { who });
			Ok(())
		}

		#[pallet::call_index(5)]
		#[pallet::weight(0)]
		pub fn create_product_try_document(
			origin: OriginFor<T>,
			client_params: ClientParamsCreateTryDoc<T>,

			app_user_account: AuthAccountId,
			app_user_sign: sr25519::Signature,

			auth_server: AuthAccountId,
			auth_sign: sr25519::Signature,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let encode = client_params.encode();
			ensure!(
				Self::verify_sign(&app_user_account, app_user_sign, &encode),
				Error::<T>::SignVerifyErrorUser
			);
			ensure!(
				Self::verify_sign(&auth_server, auth_sign, &encode),
				Error::<T>::SignVerifyErrorAuth
			);

			let ClientParamsCreateTryDoc {
				app_id,
				document_id,
				product_id,
				content_hash,
				goods_price,
				offset_rate,
				true_rate,
				seller_consistence,
				cart_id,
			} = client_params;

			ensure!(T::Membership::is_valid_app(app_id), Error::<T>::AppIdInvalid);

			// check if valid auth server
			ensure!(
				T::Membership::is_valid_app_key(app_id, &Self::convert_account(&auth_server)),
				Error::<T>::AuthIdentityNotAppKey
			);

			//let doc_key_hash = T::Hashing::hash_of(&(app_id, &document_id));
			ensure!(
				!<KPDocumentDataByIdHash<T>>::contains_key(app_id, &document_id),
				Error::<T>::DocumentAlreadyExisted
			);

			//let product_key_hash = T::Hashing::hash_of(&(app_id, &product_id));
			ensure!(
				<KPDocumentProductIndexByIdHash<T>>::contains_key(app_id, &product_id),
				Error::<T>::ProductNotFound
			);

			//let key = T::Hashing::hash_of(&(app_id, &cart_id));
			ensure!(
				!<KPCartProductTryIndexByIdHash<T>>::contains_key(app_id, &cart_id),
				Error::<T>::DocumentTryAlreadyExisted
			);

			let model_id = Self::get_model_id_from_product(app_id, &product_id).unwrap_or_default();

			// create doc
			let doc = KPDocumentData {
				sender: who.clone(),
				owner: app_user_account.clone(),
				document_type: DocumentType::ProductTry,
				app_id,
				document_id: document_id.clone(),
				model_id,
				product_id,
				content_hash,
				document_data: DocumentSpecificData::ProductTry(KPProductTryData {
					goods_price,
					offset_rate,
					true_rate,
					seller_consistence,
					cart_id: cart_id.clone(),
				}),
				..Default::default()
			};

			// process content power
			Self::process_document_content_power(&doc);
			Self::process_commodity_power(&doc);

			// create cartid -> product identify document id record

			<KPCartProductTryIndexByIdHash<T>>::insert(app_id, &cart_id, &document_id);

			// create document record
			<KPDocumentDataByIdHash<T>>::insert(app_id, &document_id, &doc);

			Self::increase_commodity_count(
				app_id,
				&doc.model_id,
				&cart_id,
				DocumentType::ProductTry,
				&Self::convert_account(&doc.owner),
			);

			Self::deposit_event(Event::KnowledgeCreated { who });
			Ok(())
		}

		#[pallet::call_index(6)]
		#[pallet::weight(0)]
		pub fn create_product_choose_document(
			origin: OriginFor<T>,
			client_params: ClientParamsCreateChooseDoc<T>,

			app_user_account: AuthAccountId,
			app_user_sign: sr25519::Signature,

			auth_server: AuthAccountId,
			auth_sign: sr25519::Signature,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let encode = client_params.encode();
			ensure!(
				Self::verify_sign(&app_user_account, app_user_sign, &encode),
				Error::<T>::SignVerifyErrorUser
			);
			ensure!(
				Self::verify_sign(&auth_server, auth_sign, &encode),
				Error::<T>::SignVerifyErrorAuth
			);

			let ClientParamsCreateChooseDoc {
				app_id,
				document_id,
				model_id,
				product_id,
				content_hash,
				sell_count,
				try_count,
			} = client_params;

			ensure!(T::Membership::is_valid_app(app_id), Error::<T>::AppIdInvalid);

			// check if valid auth server
			ensure!(
				T::Membership::is_valid_app_key(app_id, &Self::convert_account(&auth_server)),
				Error::<T>::AuthIdentityNotAppKey
			);

			//let doc_key_hash = T::Hashing::hash_of(&(app_id, &document_id));

			ensure!(
				!<KPDocumentDataByIdHash<T>>::contains_key(app_id, &document_id),
				Error::<T>::DocumentAlreadyExisted
			);

			// create doc
			let doc = KPDocumentData {
				sender: who.clone(),
				owner: app_user_account.clone(),
				document_type: DocumentType::ProductChoose,
				app_id,
				document_id: document_id.clone(),
				model_id,
				product_id,
				content_hash,
				document_data: DocumentSpecificData::ProductChoose(KPProductChooseData {
					sell_count,
					try_count,
				}),
				..Default::default()
			};

			// process content power
			Self::process_document_content_power(&doc);
			Self::process_commodity_power(&doc);

			// create document record
			<KPDocumentDataByIdHash<T>>::insert(app_id, &document_id, &doc);

			Self::deposit_event(Event::KnowledgeCreated { who });
			Ok(())
		}

		#[pallet::call_index(7)]
		#[pallet::weight(0)]
		pub fn create_model_create_document(
			origin: OriginFor<T>,
			client_params: ClientParamsCreateModelDoc<T>,

			app_user_account: AuthAccountId,
			app_user_sign: sr25519::Signature,

			auth_server: AuthAccountId,
			auth_sign: sr25519::Signature,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let encode = client_params.encode();
			ensure!(
				Self::verify_sign(&app_user_account, app_user_sign, &encode),
				Error::<T>::SignVerifyErrorUser
			);
			ensure!(
				Self::verify_sign(&auth_server, auth_sign, &encode),
				Error::<T>::SignVerifyErrorAuth
			);

			let ClientParamsCreateModelDoc {
				app_id,
				document_id,
				model_id,
				product_id,
				content_hash,
				producer_count,
				product_count,
			} = client_params;

			ensure!(T::Membership::is_valid_app(app_id), Error::<T>::AppIdInvalid);

			// check if valid auth server
			ensure!(
				T::Membership::is_valid_app_key(app_id, &Self::convert_account(&auth_server)),
				Error::<T>::AuthIdentityNotAppKey
			);

			//let doc_key_hash = T::Hashing::hash_of(&(app_id, &document_id));

			ensure!(
				!<KPDocumentDataByIdHash<T>>::contains_key(app_id, &document_id),
				Error::<T>::DocumentAlreadyExisted
			);

			ensure!(Self::is_valid_model(app_id, &model_id), Error::<T>::ModelNotFoundOrDisabled);

			// create doc
			let doc = KPDocumentData {
				sender: who.clone(),
				owner: app_user_account.clone(),
				document_type: DocumentType::ModelCreate,
				app_id,
				document_id: document_id.clone(),
				model_id,
				product_id,
				content_hash,
				document_data: DocumentSpecificData::ModelCreate(KPModelCreateData {
					producer_count,
					product_count,
				}),
				..Default::default()
			};

			// process content power
			Self::process_document_content_power(&doc);
			Self::process_commodity_power(&doc);

			// create document record
			<KPDocumentDataByIdHash<T>>::insert(app_id, &document_id, &doc);

			Self::deposit_event(Event::KnowledgeCreated { who });
			Ok(())
		}

		#[pallet::call_index(8)]
		#[pallet::weight(0)]
		pub fn create_comment(
			origin: OriginFor<T>,
			comment_data: CommentData<T>,

			app_user_account: AuthAccountId,
			app_user_sign: sr25519::Signature,

			auth_server: AuthAccountId,
			auth_sign: sr25519::Signature,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let buf = comment_data.encode();
			ensure!(
				Self::verify_sign(&app_user_account, app_user_sign, &buf),
				Error::<T>::SignVerifyErrorUser
			);
			ensure!(
				Self::verify_sign(&auth_server, auth_sign, &buf),
				Error::<T>::SignVerifyErrorAuth
			);

			let CommentData {
				app_id,
				document_id,
				comment_id,
				comment_hash,
				comment_fee,
				comment_trend,
			} = comment_data;

			ensure!(T::Membership::is_valid_app(app_id), Error::<T>::AppIdInvalid);

			// check if valid auth server
			ensure!(
				T::Membership::is_valid_app_key(app_id, &Self::convert_account(&auth_server)),
				Error::<T>::AuthIdentityNotAppKey
			);

			// TODO: check platform & expert member role

			// make sure this comment not exist
			//let key = T::Hashing::hash_of(&(app_id, &comment_id));
			ensure!(
				!<KPCommentDataByIdHash<T>>::contains_key(app_id, &comment_id),
				Error::<T>::CommentAlreadyExisted
			);

			//let doc_key_hash = T::Hashing::hash_of(&(app_id, &document_id));

			let comment = KPCommentData {
				sender: who.clone(),
				owner: app_user_account.clone(),
				app_id,
				document_id: document_id.clone(),
				comment_id: comment_id.clone(),
				comment_fee,
				comment_trend,
				comment_hash,
			};

			Self::process_comment_power(&comment);

			// read out related document, trigger account power update
			let doc = Self::kp_document_data_by_idhash(app_id, &document_id);
			Self::process_commodity_power(&doc);

			// create comment record
			<KPCommentDataByIdHash<T>>::insert(app_id, &comment_id, &comment);

			Self::deposit_event(Event::CommentCreated { who });
			Ok(())
		}

		#[pallet::call_index(10)]
		#[pallet::weight(0)]
		pub fn create_commodity_type(
			origin: OriginFor<T>,
			type_id: u32,
			type_desc: Vec<u8>,
		) -> DispatchResult {
			ensure_root(origin)?;
			ensure!(
				!<CommodityTypeMap<T>>::contains_key(type_id),
				Error::<T>::CommodityTypeExisted
			);

			let mut types = <CommodityTypeSets<T>>::get();

			let type_data = CommodityTypeData { type_id, type_desc: type_desc.clone() };

			match types.binary_search(&type_data) {
				Ok(_) => Err(Error::<T>::CommodityTypeExisted.into()),
				Err(index) => {
					types.insert(index, type_data);
					<CommodityTypeSets<T>>::put(types);

					// insert into CommodityTypeMap
					<CommodityTypeMap<T>>::insert(type_id, type_desc);

					Self::deposit_event(Event::CommodityTypeCreated { commodity_type: type_id });
					Ok(())
				},
			}
		}

		#[pallet::call_index(11)]
		#[pallet::weight(0)]
		pub fn set_app_model_total(
			origin: OriginFor<T>,
			app_id: u32,
			total: u32,
		) -> DispatchResult {
			ensure_root(origin)?;

			<AppModelTotalConfig<T>>::insert(app_id, total);

			Self::deposit_event(Event::AppModelTotal { total });
			Ok(())
		}

		#[pallet::call_index(12)]
		#[pallet::weight(0)]
		pub fn set_model_income(
			origin: OriginFor<T>,
			params: ModelIncomeCollectingParam,
			user_key: AuthAccountId,
			user_sign: sr25519::Signature,
			auth_key: AuthAccountId,
			auth_sign: sr25519::Signature,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			ensure!(
				T::Membership::is_finance_member(&Self::convert_account(&auth_key)),
				Error::<T>::AuthIdentityNotFinanceMember
			);

			let buf = params.encode();
			ensure!(Self::verify_sign(&user_key, user_sign, &buf), Error::<T>::SignVerifyErrorUser);
			ensure!(Self::verify_sign(&auth_key, auth_sign, &buf), Error::<T>::SignVerifyErrorAuth);

			let ModelIncomeCollectingParam { app_id, model_ids, incomes } = params;

			ensure!(
				T::Membership::is_app_admin(&Self::convert_account(&user_key), app_id),
				Error::<T>::NotAppAdmin
			);
			ensure!(incomes.len() <= 100, Error::<T>::ModelIncomeParamsTooLarge);

			let block = frame_system::Pallet::<T>::block_number();
			ensure!(
				Self::model_income_stage(block).0 == ModelIncomeStage::COLLECTING,
				Error::<T>::ModelIncomeNotInCollectingStage
			);

			let cycle_index = Self::model_income_cycle_index(block);

			for idx in 0..incomes.len() {
				let model_id = &model_ids[idx];
				let income = incomes[idx];

				if !Self::is_valid_model(app_id, model_id) {
					//print("model id not found or disabled, ignore");
					continue;
				}

				// check if last cycle slashed
				//let sub_key = T::Hashing::hash_of(&(app_id, model_id));
				if <ModelSlashCycleRewardIndex<T>>::contains_key(app_id, model_id)
					&& <ModelSlashCycleRewardIndex<T>>::get(app_id, model_id)
						== cycle_index - 1u32.into()
				{
					//print("model last cycle slashed, ignore");
					continue;
				}

				// check if it is existed already
				if <ModelCycleIncome<T>>::contains_key((cycle_index, app_id, model_id)) {
					//print("model income current cycle exist, ignore");
					continue;
				}

				// add this model income to cycle total
				let result = match <ModelCycleIncomeTotal<T>>::get(cycle_index).checked_add(income)
				{
					Some(r) => r,
					None => return Err(<Error<T>>::AddOverflow.into()),
				};
				<ModelCycleIncomeTotal<T>>::insert(cycle_index, result);
				<ModelCycleIncome<T>>::insert((cycle_index, app_id, model_id), income);

				// update app cycle total
				<AppCycleIncome<T>>::mutate(cycle_index, app_id, |record| {
					record.income += income;
					record.cycle = cycle_index;
					record.app_id = app_id;
				});
			}

			Self::deposit_event(Event::ModelCycleIncome { who });
			Ok(())
		}

		#[pallet::call_index(13)]
		#[pallet::weight(0)]
		pub fn request_model_reward(
			origin: OriginFor<T>,
			app_id: u32,
			model_id: Vec<u8>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			ensure!(Self::is_valid_model(app_id, &model_id), Error::<T>::ModelNotFoundOrDisabled);
			// make sure who is creaor of this model id
			ensure!(
				T::Membership::is_model_creator(&who, app_id, &model_id),
				Error::<T>::NotModelCreator
			);

			let block = frame_system::Pallet::<T>::block_number();
			ensure!(
				Self::model_income_stage(block).0 == ModelIncomeStage::REWARDING,
				Error::<T>::ModelIncomeNotInRewardingStage
			);

			let cycle_index = Self::model_income_cycle_index(block);
			//let sub_key = T::Hashing::hash_of(&(app_id, &model_id));
			ensure!(
				!<ModelCycleIncomeRewardRecords<T>>::contains_key((cycle_index, app_id, &model_id)),
				Error::<T>::ModelCycleRewardAlreadyExisted
			);

			// check if it was slashed this cycle
			if <ModelSlashCycleRewardIndex<T>>::contains_key(app_id, &model_id) {
				ensure!(
					<ModelSlashCycleRewardIndex<T>>::get(app_id, &model_id)
						!= cycle_index - 1u32.into(),
					Error::<T>::ModelCycleRewardSlashed
				);
			}

			// now compute reward
			let total_reward = T::ModelCycleIncomeRewardTotal::get();

			let cycle_income_total = <ModelCycleIncomeTotal<T>>::get(cycle_index);
			ensure!(cycle_income_total > 0, Error::<T>::ModelCycleIncomeTotalZero);

			let cycle_income =
				<ModelCycleIncome<T>>::get((cycle_index, app_id, &model_id)).unwrap();
			ensure!(cycle_income > 0, Error::<T>::ModelCycleIncomeZero);

			let per = Permill::from_rational_approximation(cycle_income, cycle_income_total);
			let reward = per * total_reward;

			// transfer now
			let treasury_account: T::AccountId =
				T::TreasuryModuleId::get().into_account_truncating();
			T::Currency::transfer(&treasury_account, &who, reward, KeepAlive)?;

			// update global total reward
			let total = <ModelIncomeRewardTotal<T>>::get() + reward;
			<ModelIncomeRewardTotal<T>>::put(total);

			// update records
			<ModelCycleIncomeRewardRecords<T>>::insert((cycle_index, app_id, &model_id), reward);

			<ModelCycleIncomeRewardStore<T>>::mutate(cycle_index, |store| {
				store.push(ModelCycleIncomeReward {
					account: who.clone(),
					app_id,
					model_id: model_id.clone(),
					reward,
				})
			});

			Self::deposit_event(Event::ModelIncomeRewarded { who });
			Ok(())
		}

		#[pallet::call_index(14)]
		#[pallet::weight(0)]
		pub fn app_income_redeem_request(
			origin: OriginFor<T>,
			params: AppIncomeRedeemParams<T>,
			app_user_account: AuthAccountId,
			app_user_sign: sr25519::Signature,

			auth_server: AuthAccountId,
			auth_sign: sr25519::Signature,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let buf = params.encode();
			ensure!(
				Self::verify_sign(&app_user_account, app_user_sign, &buf),
				Error::<T>::SignVerifyErrorUser
			);
			ensure!(
				Self::verify_sign(&auth_server, auth_sign, &buf),
				Error::<T>::SignVerifyErrorAuth
			);

			let AppIncomeRedeemParams { account, app_id, cycle, exchange_amount } = params;

			ensure!(T::Membership::is_valid_app(app_id), Error::<T>::AppIdInvalid);
			ensure!(
				T::Membership::is_app_admin(&Self::convert_account(&auth_server), app_id),
				Error::<T>::NotAppAdmin
			);

			// check if current model cycle match
			let block = frame_system::Pallet::<T>::block_number();
			ensure!(
				Self::model_income_stage(block).0 == ModelIncomeStage::REWARDING,
				Error::<T>::ModelIncomeNotInRewardingStage
			);

			// check if user has performed this exchange
			//let ukey = Self::app_income_exchange_record_key(app_id, cycle, &account);
			ensure!(
				!<AppCycleIncomeExchangeRecords<T>>::contains_key((app_id, cycle, &account)),
				Error::<T>::AppFinancedUserExchangeAlreadyPerformed
			);

			//let fkey = T::Hashing::hash_of(&(app_id, cycle));
			let finance_member: T::AccountId;
			// check if we have specified a finance member to do confirm
			if !<AppCycleIncomeFinanceMember<T>>::contains_key(app_id, cycle) {
				// random choose one
				finance_member = Self::choose_finance_member()?;
				<AppCycleIncomeFinanceMember<T>>::insert(
					app_id,
					cycle,
					Some(finance_member.clone()),
				);
			} else {
				finance_member = <AppCycleIncomeFinanceMember<T>>::get(app_id, cycle).unwrap();
			}

			// read app cycle income record
			let mut record = <AppCycleIncome<T>>::get(cycle, app_id);
			// make sure has enough income counted
			ensure!(record.income > 0, Error::<T>::AppCycleIncomeZero);
			// first caller will trigger balance setup
			if record.initial == 0u32.into() {
				// read out app income rate
				match T::Membership::get_app_setting(app_id) {
					(rate, ..) => {
						// compute initial balance
						ensure!(rate > 0, Error::<T>::AppCycleIncomeRateZero);
						let per = Permill::from_rational_approximation(rate, 10000);
						let cent: BalanceOf<T> = ((per * record.income) as u32).into();
						// got cent, needs to convert to balance
						record.initial = cent * 1000000000000u128.saturated_into();
						record.balance = record.initial;

						// update count
						<AppCycleIncomeCount<T>>::put(<AppCycleIncomeCount<T>>::get() + 1);
					},
				}
			}

			// reserve finance fee
			let fee = Permill::from_rational_approximation(T::RedeemFeeRate::get(), 1000u32)
				* exchange_amount;
			let amount = exchange_amount + fee;

			// make sure balance is enough
			ensure!(record.balance > amount, Error::<T>::AppFinancedUserExchangeOverflow);

			// reserve exchange_amount from user account
			T::Currency::reserve(&account, amount)?;
			record.balance -= exchange_amount;

			<AppCycleIncome<T>>::insert(cycle, app_id, &record);

			// record user exchange record AppCycleIncomeExchangeRecords
			<AppCycleIncomeExchangeRecords<T>>::insert(
				(app_id, cycle, &account),
				AppFinancedUserExchangeData { exchange_amount, status: 1, ..Default::default() },
			);

			let mut accounts = <AppCycleIncomeExchangeSet<T>>::get(app_id, cycle);
			accounts.push(account.clone());
			<AppCycleIncomeExchangeSet<T>>::insert(app_id, cycle, accounts);

			Self::deposit_event(Event::AppCycleIncomeRedeem { who });
			Ok(())
		}

		#[pallet::call_index(15)]
		#[pallet::weight(0)]
		pub fn app_income_redeem_confirm(
			origin: OriginFor<T>,
			params: AppIncomeRedeemConfirmParams<T>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let AppIncomeRedeemConfirmParams { account, app_id, cycle, pay_id } = params;

			ensure!(T::Membership::is_valid_app(app_id), Error::<T>::AppIdInvalid);

			// get this cycle's finance member
			//let member_key = T::Hashing::hash_of(&(app_id, cycle));
			let finance_member = <AppCycleIncomeFinanceMember<T>>::get(app_id, cycle).unwrap();
			ensure!(finance_member == who, Error::<T>::AuthIdentityNotExpectedFinanceMember);

			//let ukey = Self::app_income_exchange_record_key(app_id, cycle, &account);
			// make sure record exist
			ensure!(
				<AppCycleIncomeExchangeRecords<T>>::contains_key((app_id, cycle, &account)),
				Error::<T>::AppFinancedUserExchangeRecordNotExist
			);

			// make sure state is 1
			let record =
				<AppCycleIncomeExchangeRecords<T>>::get((app_id, cycle, &account)).unwrap();
			ensure!(record.status == 1, Error::<T>::AppFinancedUserExchangeStateWrong);

			// check if current model cycle match
			let block = frame_system::Pallet::<T>::block_number();
			let state = Self::model_income_stage(block);
			ensure!(
				state.0 == ModelIncomeStage::CONFIRMING || state.0 == ModelIncomeStage::REWARDING,
				Error::<T>::ModelIncomeNotInConfirmingStage
			);

			let fee = Permill::from_rational_approximation(T::RedeemFeeRate::get(), 1000u32)
				* record.exchange_amount;
			// unreserve account balance
			T::Currency::unreserve(&account, record.exchange_amount + fee);

			// give fee to finance member
			T::Currency::transfer(&account, &finance_member, fee, KeepAlive)?;

			// burn process
			let (debit, credit) = T::Currency::pair(record.exchange_amount);
			T::BurnDestination::on_unbalanced(credit);

			if let Err(problem) =
				T::Currency::settle(&account, debit, WithdrawReasons::TRANSFER, KeepAlive)
			{
				// print("Inconsistent state - couldn't settle imbalance");
				// Nothing else to do here.
				drop(problem);
			}

			// update store
			<AppCycleIncomeExchangeRecords<T>>::mutate((app_id, cycle, &account), |record| {
				if let Some(record_value) = record {
					record_value.status = 2;
					record_value.pay_id = pay_id;
				}
			});

			<AppCycleIncomeBurnTotal<T>>::put(
				<AppCycleIncomeBurnTotal<T>>::get() + record.exchange_amount,
			);

			Self::deposit_event(Event::AppCycleIncomeUserExchangeConfirmed { who: account });
			Ok(())
		}

		#[pallet::call_index(16)]
		#[pallet::weight(0)]
		pub fn app_income_redeem_compensate(
			origin: OriginFor<T>,
			app_id: u32,
			cycle: BlockNumberFor<T>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			//let fkey = T::Hashing::hash_of(&(app_id, cycle));
			//let ukey = Self::app_income_exchange_record_key(app_id, cycle, &who);
			// check record exist
			ensure!(
				<AppCycleIncomeExchangeRecords<T>>::contains_key((app_id, cycle, &who)),
				Error::<T>::AppFinancedUserExchangeRecordNotExist
			);

			let record = <AppCycleIncomeExchangeRecords<T>>::get((app_id, cycle, &who)).unwrap();
			ensure!(record.status == 1, Error::<T>::AppFinancedUserExchangeStateWrong);

			let block = frame_system::Pallet::<T>::block_number();
			let stage = Self::model_income_stage(block);

			// check if current model cycle match
			ensure!(
				stage.0 == ModelIncomeStage::COMPENSATING,
				Error::<T>::ModelIncomeNotInCompensatingStage
			);

			// unlock balance
			T::Currency::unreserve(&who, record.exchange_amount);

			// get slash from finance member
			let finance_member = <AppCycleIncomeFinanceMember<T>>::get(app_id, cycle).unwrap();

			let status = if T::Membership::slash_finance_member(
				&finance_member,
				&who,
				record.exchange_amount,
			)
			.is_ok()
			{
				3
			} else {
				4
			};

			<AppCycleIncomeExchangeRecords<T>>::mutate((app_id, cycle, &who), |record| {
				if let Some(record_value) = record {
					record_value.status = status;
				}
			});

			Self::deposit_event(Event::AppIncomeUserExchangeCompensated { who });
			Ok(())
		}

		#[pallet::call_index(17)]
		#[pallet::weight(0)]
		pub fn democracy_slash_commodity_power(
			origin: OriginFor<T>,
			app_id: u32,
			cart_id: Vec<u8>,
			comment_id: Vec<u8>,
			_reporter_account: T::AccountId,
		) -> DispatchResult {
			ensure_root(origin)?;
			ensure!(T::Membership::is_valid_app(app_id), Error::<T>::AppIdInvalid);

			// check if this cart id already in blacklist
			ensure!(
				!Self::is_commodity_in_black_list(app_id, cart_id.clone()),
				Error::<T>::CartIdInBlackList
			);

			// read out comment to get related document owner
			//let comment_key = T::Hashing::hash_of(&(app_id, &comment_id));
			ensure!(
				<KPCommentDataByIdHash<T>>::contains_key(app_id, &comment_id),
				Error::<T>::CommentNotFound
			);
			let comment = <KPCommentDataByIdHash<T>>::get(app_id, &comment_id);

			//let doc_key = T::Hashing::hash_of(&(app_id, &comment.document_id));
			ensure!(
				<KPDocumentDataByIdHash<T>>::contains_key(app_id, &comment.document_id),
				Error::<T>::DocumentNotFound
			);

			let doc = <KPDocumentDataByIdHash<T>>::get(app_id, &comment.document_id);

			// get model id from publish doc
			let model_id =
				Self::get_model_id_from_product(app_id, &doc.product_id).unwrap_or_default();

			// perform slash
			//let key_hash = T::Hashing::hash_of(&(app_id, &cart_id));
			let owner_account = Self::convert_account(&doc.owner);
			Self::slash_power(app_id, &cart_id, &owner_account);
			Self::remove_leader_board_item(app_id, &model_id, &cart_id);

			Self::add_commodity_power_slash_record(app_id, &comment_id, &cart_id);

			Self::deposit_event(Event::PowerSlashed { who: owner_account });
			Ok(())
		}

		#[pallet::call_index(18)]
		#[pallet::weight(0)]
		pub fn democracy_model_dispute(
			origin: OriginFor<T>,
			app_id: u32,
			model_id: Vec<u8>,
			dispute_type: ModelDisputeType,
			comment_id: Vec<u8>,
			reporter_account: T::AccountId,
		) -> DispatchResult {
			ensure_root(origin)?;

			ensure!(T::Membership::is_valid_app(app_id), Error::<T>::AppIdInvalid);

			// get model creator account
			ensure!(Self::is_valid_model(app_id, &model_id), Error::<T>::ModelNotFoundOrDisabled);

			// let key = T::Hashing::hash_of(&(app_id, &model_id));
			let model = <KPModelDataByIdHash<T>>::get(app_id, &model_id);
			// according dispute type to decide slash
			let owner_account = Self::convert_account(&model.owner);

			Self::model_dispute(app_id, &model_id, dispute_type, &owner_account, &reporter_account);

			// update store
			Self::add_model_dispute_record(app_id, &model_id, &comment_id, dispute_type);

			Self::deposit_event(Event::ModelDisputed { who: owner_account });
			Ok(())
		}

		#[pallet::call_index(19)]
		#[pallet::weight(0)]
		pub fn democracy_add_app(
			origin: OriginFor<T>,
			params: AddAppParams<T>,
			app_user_account: AuthAccountId,
			app_user_sign: sr25519::Signature,
		) -> DispatchResult {
			ensure_root(origin)?;

			let buf = params.encode();
			ensure!(
				Self::verify_sign(&app_user_account, app_user_sign, &buf),
				Error::<T>::SignVerifyErrorUser
			);

			let AddAppParams { app_type, app_name, app_key, app_admin_key, return_rate } = params;

			// check app_type
			ensure!(<AppIdRange<T>>::contains_key(&app_type), Error::<T>::AppTypeInvalid);
			//print("democracy_add_app pass type check");
			// check return_rate
			ensure!(return_rate > 0 && return_rate < 10000, Error::<T>::ReturnRateInvalid);
			// check app_admin_key match app_user_account
			ensure!(
				Self::convert_account(&app_user_account) == app_admin_key,
				Error::<T>::AppAdminNotMatchUser
			);

			// generate app_id
			let app_info = <AppIdRange<T>>::get(&app_type);
			let (current_id, stake, max, num, max_models) = app_info;

			// check if reach max
			if max > 0 {
				ensure!(num < max, Error::<T>::AppIdReachMax);
			}

			// reserve balance
			if stake > 0u32.into() {
				T::Currency::reserve(&app_admin_key, stake)?;
			}

			let app_id = current_id + 1;
			// set admin and idenetity key
			T::Membership::config_app_admin(&app_admin_key, app_id);
			T::Membership::config_app_key(&app_key, app_id);
			T::Membership::config_app_setting(app_id, return_rate, app_name, stake);

			// config max model
			<AppModelTotalConfig<T>>::insert(app_id, max_models);

			// update app_id range store
			<AppIdRange<T>>::mutate(&app_type, |info| {
				info.0 = app_id;
				info.3 += 1;
			});
			Self::deposit_event(Event::AppAdded { app_id });
			Ok(())
		}

		#[pallet::call_index(20)]
		#[pallet::weight(0)]
		pub fn democracy_app_financed(
			origin: OriginFor<T>,
			params: AppFinancedProposalParams<T>,
			app_user_account: AuthAccountId,
			app_user_sign: sr25519::Signature,

			auth_server: AuthAccountId,
			auth_sign: sr25519::Signature,
		) -> DispatchResult {
			ensure_root(origin)?;

			let current_block = frame_system::Pallet::<T>::block_number();

			// check if last exchange cycle ended
			let (app_id, purpose_id) = <AppFinancedLast<T>>::get();
			if <AppFinancedRecord<T>>::contains_key(app_id, &purpose_id) {
				let last_record = <AppFinancedRecord<T>>::get(app_id, &purpose_id);
				ensure!(
					last_record.exchange_end_block < current_block,
					Error::<T>::AppFinancedLastExchangeNotEnd
				);
			}

			// only finance memebers allow auth
			ensure!(
				T::Membership::is_finance_member(&Self::convert_account(&auth_server)),
				Error::<T>::AuthIdentityNotFinanceMember
			);

			let buf = params.encode();
			ensure!(
				Self::verify_sign(&app_user_account, app_user_sign, &buf),
				Error::<T>::SignVerifyErrorUser
			);
			ensure!(
				Self::verify_sign(&auth_server, auth_sign, &buf),
				Error::<T>::SignVerifyErrorAuth
			);

			let AppFinancedProposalParams { account, app_id, proposal_id, exchange, amount } =
				params;

			ensure!(T::Membership::is_valid_app(app_id), Error::<T>::AppIdInvalid);

			ensure!(
				amount > 0u32.into() && exchange > 0u32.into(),
				Error::<T>::AppFinancedParamsInvalid
			);

			let min_exchange = T::KptExchangeMinRate::get() * amount;
			ensure!(exchange >= min_exchange, Error::<T>::AppFinancedExchangeRateTooLow);

			//let key = T::Hashing::hash_of(&(app_id, &proposal_id));
			ensure!(
				!<AppFinancedRecord<T>>::contains_key(app_id, &proposal_id),
				Error::<T>::AppAlreadyFinanced
			);

			// start transfer amount
			ensure!(T::Membership::is_investor(&account), Error::<T>::AppFinancedNotInvestor);

			let total_balance = T::Currency::total_issuance_excluding_fund();

			let treasury_account: T::AccountId =
				T::FinTreasuryModuleId::get().into_account_truncating();
			T::Currency::transfer(&treasury_account, &account, amount, KeepAlive)?;

			<AppFinancedRecord<T>>::insert(
				app_id,
				&proposal_id,
				AppFinancedData::<T> {
					app_id,
					proposal_id: proposal_id.clone(),
					amount,
					exchange,
					block: current_block,
					total_balance,
					exchanged: 0u32.into(),
					exchange_end_block: current_block + T::AppFinanceExchangePeriod::get(),
				},
			);

			// recrod it as last
			<AppFinancedLast<T>>::put((app_id, proposal_id));
			// update count
			<AppFinancedCount<T>>::put(<AppFinancedCount<T>>::get() + 1);

			Self::deposit_event(Event::AppFinanced { app_id });
			Ok(())
		}

		#[pallet::call_index(21)]
		#[pallet::weight(0)]
		pub fn app_financed_user_exchange_request(
			origin: OriginFor<T>,
			params: AppFinancedUserExchangeParams<T>,
			app_user_account: AuthAccountId,
			app_user_sign: sr25519::Signature,

			auth_server: AuthAccountId,
			auth_sign: sr25519::Signature,
		) -> DispatchResult {
			let _who = ensure_signed(origin)?;

			let buf = params.encode();
			ensure!(
				Self::verify_sign(&app_user_account, app_user_sign, &buf),
				Error::<T>::SignVerifyErrorUser
			);
			ensure!(
				Self::verify_sign(&auth_server, auth_sign, &buf),
				Error::<T>::SignVerifyErrorAuth
			);

			let AppFinancedUserExchangeParams { account, app_id, proposal_id, exchange_amount } =
				params;

			ensure!(T::Membership::is_valid_app(app_id), Error::<T>::AppIdInvalid);
			ensure!(
				T::Membership::is_app_admin(&Self::convert_account(&auth_server), app_id),
				Error::<T>::NotAppAdmin
			);

			// check if app financed record exist
			//let fkey = T::Hashing::hash_of(&(app_id, &proposal_id));
			ensure!(
				<AppFinancedRecord<T>>::contains_key(app_id, &proposal_id),
				Error::<T>::AppFinancedUserExchangeProposalNotExist
			);
			// check if user has performed this exchange
			//let ukey = Self::app_financed_exchange_record_key(app_id, &proposal_id, &account);
			ensure!(
				!<AppFinancedUserExchangeRecord<T>>::contains_key((app_id, &proposal_id, &account)),
				Error::<T>::AppFinancedUserExchangeAlreadyPerformed
			);

			// read financed record
			let mut financed_record = <AppFinancedRecord<T>>::get(app_id, &proposal_id);

			// check if exchange end
			ensure!(
				financed_record.exchange_end_block > frame_system::Pallet::<T>::block_number(),
				Error::<T>::AppFinancedUserExchangeEnded
			);

			// make sure exchanged not overflow (this should not happen, if happen should be serious bug)
			ensure!(
				financed_record.exchanged + exchange_amount <= financed_record.exchange,
				Error::<T>::AppFinancedUserExchangeOverflow
			);

			// get finance member AppFinanceFinanceMember
			let finance_member: T::AccountId;
			// check if we have specified a finance member to do confirm
			if !<AppFinanceFinanceMember<T>>::contains_key(app_id, &proposal_id) {
				// random choose one
				finance_member = Self::choose_finance_member()?;
				<AppFinanceFinanceMember<T>>::insert(
					app_id,
					&proposal_id,
					Some(finance_member.clone()),
				);
			} else {
				finance_member = <AppFinanceFinanceMember<T>>::get(app_id, &proposal_id).unwrap();
			}

			// reserve finance fee
			let fee = Permill::from_rational_approximation(T::RedeemFeeRate::get(), 1000u32)
				* exchange_amount;
			let amount = exchange_amount + fee;

			// reserve exchange_amount from user account
			T::Currency::reserve(&account, amount)?;

			financed_record.exchanged += exchange_amount;
			<AppFinancedRecord<T>>::insert(app_id, &proposal_id, financed_record);

			// AppFinancedUserExchangeData
			<AppFinancedUserExchangeRecord<T>>::insert(
				(app_id, &proposal_id, &account),
				AppFinancedUserExchangeData { exchange_amount, status: 1, ..Default::default() },
			);

			let mut accounts = <AppFinancedUserExchangeSet<T>>::get(app_id, &proposal_id);
			accounts.push(account.clone());
			<AppFinancedUserExchangeSet<T>>::insert(app_id, &proposal_id, accounts);

			Self::deposit_event(Event::AppFinanceUserExchangeStart {
				who: account,
				user: finance_member,
			});
			Ok(())
		}

		#[pallet::call_index(22)]
		#[pallet::weight(0)]
		pub fn app_financed_user_exchange_confirm(
			origin: OriginFor<T>,
			params: AppFinancedUserExchangeConfirmParams<T>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let AppFinancedUserExchangeConfirmParams { account, app_id, proposal_id, pay_id } =
				params;

			// get this cycle's finance member
			//let member_key = T::Hashing::hash_of(&(app_id, &proposal_id));
			let finance_member = <AppFinanceFinanceMember<T>>::get(app_id, &proposal_id).unwrap();
			ensure!(finance_member == who, Error::<T>::AuthIdentityNotExpectedFinanceMember);

			ensure!(T::Membership::is_valid_app(app_id), Error::<T>::AppIdInvalid);

			//let ukey = Self::app_financed_exchange_record_key(app_id, &proposal_id, &account);
			// make sure record exist
			ensure!(
				<AppFinancedUserExchangeRecord<T>>::contains_key((app_id, &proposal_id, &account)),
				Error::<T>::AppFinancedUserExchangeRecordNotExist
			);

			// make sure state is 1
			let record =
				<AppFinancedUserExchangeRecord<T>>::get((app_id, &proposal_id, &account)).unwrap();
			ensure!(record.status == 1, Error::<T>::AppFinancedUserExchangeStateWrong);

			// make sure not over confirm end stage
			//let fkey = T::Hashing::hash_of(&(app_id, &proposal_id));
			let financed_record = <AppFinancedRecord<T>>::get(app_id, &proposal_id);
			let current_block = frame_system::Pallet::<T>::block_number();
			let end = financed_record.exchange_end_block
				+ T::AppFinanceExchangePeriod::get() / 2u32.into();
			ensure!(end >= current_block, Error::<T>::AppFinancedUserExchangeConfirmEnded);

			let fee = Permill::from_rational_approximation(T::RedeemFeeRate::get(), 1000u32)
				* record.exchange_amount;
			// unreserve account balance
			T::Currency::unreserve(&account, record.exchange_amount + fee);

			// give fee to finance member
			T::Currency::transfer(&account, &finance_member, fee, KeepAlive)?;

			// burn process
			let (debit, credit) = T::Currency::pair(record.exchange_amount);
			T::BurnDestination::on_unbalanced(credit);

			if let Err(problem) =
				T::Currency::settle(&account, debit, WithdrawReasons::TRANSFER, KeepAlive)
			{
				//print("Inconsistent state - couldn't settle imbalance");
				// Nothing else to do here.
				drop(problem);
			}

			// update store
			<AppFinancedUserExchangeRecord<T>>::mutate(
				(app_id, &proposal_id, &account),
				|record| {
					if let Some(record) = record {
						record.status = 2;
						record.pay_id = pay_id;
					}
				},
			);

			<AppFinancedBurnTotal<T>>::put(
				<AppFinancedBurnTotal<T>>::get() + record.exchange_amount,
			);

			Self::deposit_event(Event::AppFinanceUserExchangeConfirmed { who: account });
			Ok(())
		}

		#[pallet::call_index(23)]
		#[pallet::weight(0)]
		pub fn app_finance_redeem_compensate(
			origin: OriginFor<T>,
			app_id: u32,
			proposal_id: Vec<u8>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			//let fkey = T::Hashing::hash_of(&(app_id, &proposal_id));
			//let ukey = Self::app_financed_exchange_record_key(app_id, &proposal_id, &who);
			// check record exist
			ensure!(
				<AppFinancedUserExchangeRecord<T>>::contains_key((app_id, &proposal_id, &who)),
				Error::<T>::AppFinancedUserExchangeRecordNotExist
			);

			let record =
				<AppFinancedUserExchangeRecord<T>>::get((app_id, &proposal_id, &who)).unwrap();
			ensure!(record.status == 1, Error::<T>::AppFinancedUserExchangeStateWrong);

			let current_block = frame_system::Pallet::<T>::block_number();
			let financed_record = <AppFinancedRecord<T>>::get(app_id, &proposal_id);
			// make sure current block over end + end
			let confirm_end = financed_record.exchange_end_block
				+ T::AppFinanceExchangePeriod::get() / 2u32.into();
			ensure!(confirm_end < current_block, Error::<T>::AppFinancedUserExchangeConfirmNotEnd);

			// make sure not over compensate end stage
			let end = financed_record.exchange_end_block + T::AppFinanceExchangePeriod::get();
			ensure!(end >= current_block, Error::<T>::AppFinancedUserExchangeCompensateEnded);

			// unlock balance
			T::Currency::unreserve(&who, record.exchange_amount);

			// get slash from finance member
			let finance_member = <AppFinanceFinanceMember<T>>::get(app_id, &proposal_id).unwrap();
			let status = if T::Membership::slash_finance_member(
				&finance_member,
				&who,
				record.exchange_amount,
			)
			.is_ok()
			{
				3
			} else {
				4
			};

			<AppFinancedUserExchangeRecord<T>>::mutate((app_id, &proposal_id, &who), |record| {
				if let Some(record) = record {
					record.status = status;
				}
			});

			Self::deposit_event(Event::AppFinanceUserExchangeCompensated { who });
			Ok(())
		}

		#[pallet::call_index(24)]
		#[pallet::weight(0)]
		pub fn create_power_leader_board(
			origin: OriginFor<T>,
			app_id: u32,
			model_id: Vec<u8>,
		) -> DispatchResult {
			ensure_root(origin)?;

			ensure!(T::Membership::is_valid_app(app_id), Error::<T>::AppIdInvalid);

			let current_block = frame_system::Pallet::<T>::block_number();
			// read out last time block number and check distance
			//let last_key = T::Hashing::hash_of(&(app_id, &model_id));

			if <AppLeaderBoardLastTime<T>>::contains_key(app_id, &model_id) {
				let last_block = <AppLeaderBoardLastTime<T>>::get(app_id, &model_id);
				let diff = current_block - last_block;
				ensure!(
					diff > T::AppLeaderBoardInterval::get(),
					Error::<T>::LeaderBoardCreateNotPermit
				);
			}

			Self::leader_board_lottery(current_block, app_id, &model_id);

			Self::deposit_event(Event::LeaderBoardsCreated {
				block: current_block,
				app_id,
				model_id,
			});
			Ok(())
		}

		#[pallet::call_index(25)]
		#[pallet::weight(0)]
		pub fn democracy_tech_fund_withdraw(
			origin: OriginFor<T>,
			receiver: T::AccountId,
			reason: T::Hash,
			dev_type: TechFundWithdrawType,
			dev_level: TechFundWithdrawLevel,
		) -> DispatchResult {
			T::TechMemberOrigin::ensure_origin(origin)?;

			// compute balance
			let amount = Self::compute_tech_fund_withdraw(dev_type, dev_level);
			ensure!(amount > 0u32.into(), Error::<T>::TechFundAmountComputeError);

			let treasury_account: T::AccountId =
				T::TechTreasuryModuleId::get().into_account_truncating();
			T::Currency::transfer(&treasury_account, &receiver, amount, KeepAlive)?;
			//print("pass transfer");

			// Records it
			let mut records = <TechFundWithdrawRecords<T>>::get();
			records.push(TechFundWithdrawData {
				account: receiver.clone(),
				amount,
				dev_level,
				dev_type,
				reason,
			});
			<TechFundWithdrawRecords<T>>::put(records);

			Self::deposit_event(Event::TechFundWithdrawed { who: receiver });
			Ok(())
		}
	}

	/// support functions
	impl<T: Config> Pallet<T> {
		fn is_valid_model(app_id: u32, model_id: &Vec<u8>) -> bool {
			if !<KPModelDataByIdHash<T>>::contains_key(app_id, model_id) {
				return false;
			}

			<KPModelDataByIdHash<T>>::get(app_id, model_id).status == ModelStatus::ENABLED
		}

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
			//let leader_key = T::Hashing::hash_of(&(app_id, model_id));
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
				let _removed = board.pop()?;
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
			//let doc_key_hash = T::Hashing::hash_of(&(app_id, doc_id));
			let doc = Self::kp_document_data_by_idhash(app_id, doc_id);

			//let product_key_hash = T::Hashing::hash_of(&(app_id, &doc.product_id));
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
				//let model_key = T::Hashing::hash_of(&(app_id, model_id));
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
				//let key = T::Hashing::hash_of(&(app_id, &board_item.cart_id));
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

		// triggered when:
		// 1. doc identify/try/choose/model was created
		// 2. any document was commented, doc param is comment target
		fn process_commodity_power(doc: &KPDocumentData<T>) -> Option<u32> {
			let commodity_owner = Self::convert_account(&doc.owner);
			// read document owner action power
			//let key = T::Hashing::hash_of(&(&commodity_owner, doc.app_id));
			let owner_account_power = Self::account_attend_power_map(&commodity_owner, doc.app_id);
			// read doc power
			//let doc_key = T::Hashing::hash_of(&(doc.app_id, &doc.document_id));
			let doc_power = <KPDocumentPowerByIdHash<T>>::get(doc.app_id, &doc.document_id);

			let update_publish = |commodity_power: &mut CommodityPowerSet| {
				//let publish_doc_key = T::Hashing::hash_of(&(doc.app_id, &doc.product_id));
				//let publish_doc_id =
				//	<KPDocumentProductIndexByIdHash<T>>::get(doc.app_id, &doc.product_id);
				// read out publish document power
				// let publish_power_key = T::Hashing::hash_of(&(doc.app_id, &publish_doc_id));
				commodity_power.0 = <KPDocumentPowerByIdHash<T>>::get(doc.app_id, &doc.product_id);
			};

			match &doc.document_data {
				DocumentSpecificData::ProductIdentify(data) => {
					// let commodity_key = T::Hashing::hash_of(&(doc.app_id, &data.cart_id));
					let is_slashed =
						<KPPurchaseBlackList<T>>::contains_key(doc.app_id, &data.cart_id);
					if !is_slashed {
						let mut commodity_power =
							<KPPurchasePowerByIdHash<T>>::get(doc.app_id, &data.cart_id);
						// update commodity power
						commodity_power.3 = owner_account_power;
						// update doc power
						commodity_power.1 = doc_power;
						// update publish power
						update_publish(&mut commodity_power);
						// update price power
						commodity_power.4 = Self::compute_price_power(data.goods_price);

						let model_id =
							Self::get_model_id_from_product(doc.app_id, &doc.product_id)?;

						Self::update_purchase_power(
							&commodity_power,
							doc.app_id,
							&model_id,
							&data.cart_id,
							&commodity_owner,
						);
					}
				},
				DocumentSpecificData::ProductTry(data) => {
					//let commodity_key = T::Hashing::hash_of(&(doc.app_id, &data.cart_id));
					let is_slashed =
						<KPPurchaseBlackList<T>>::contains_key(doc.app_id, &data.cart_id);
					if !is_slashed {
						let mut commodity_power =
							<KPPurchasePowerByIdHash<T>>::get(doc.app_id, &data.cart_id);
						// update commodity power
						commodity_power.3 = owner_account_power;
						// update doc power
						commodity_power.2 = doc_power;
						// update publish power
						update_publish(&mut commodity_power);
						// update price power
						commodity_power.4 = Self::compute_price_power(data.goods_price);

						let model_id =
							Self::get_model_id_from_product(doc.app_id, &doc.product_id)?;
						Self::update_purchase_power(
							&commodity_power,
							doc.app_id,
							&model_id,
							&data.cart_id,
							&commodity_owner,
						);
					}
				},
				// ignore publish doc
				DocumentSpecificData::ProductPublish(_data) => {
					return None;
				},
				// left is product choose and model create doc, only update commodity power
				_ => {
					<KPMiscDocumentPowerByIdHash<T>>::insert(
						doc.app_id,
						&doc.document_id,
						owner_account_power + doc_power.total(),
					);
				},
			}

			Some(0)
		}

		fn process_comment_power(comment: &KPCommentData<T>) {
			// target compute
			let account_comment_power: PowerSize;
			let doc_comment_power: PowerSize;
			//let doc_key_hash = T::Hashing::hash_of(&(comment.app_id, &comment.document_id));

			// read out document
			let mut doc = Self::kp_document_data_by_idhash(comment.app_id, &comment.document_id);

			//let comment_account_key = T::Hashing::hash_of(&(comment.app_id, &comment.sender));
			let mut account = Self::kp_comment_account_record_map(comment.app_id, &comment.sender);

			account.count += 1;
			account.fees += comment.comment_fee;

			doc.comment_count += 1;
			doc.comment_total_fee += comment.comment_fee;
			if comment.comment_trend == 0 {
				doc.comment_positive_count += 1;
				account.positive_count += 1;
			}

			let mut account_comment_max = Self::comment_max_info_per_account_map(comment.app_id);

			let account_comment_unit_fee = account.fees / account.count;
			let is_account_max_updated = Self::update_comment_max(
				&mut account_comment_max,
				account.count,
				account.fees,
				account.positive_count,
				account_comment_unit_fee,
			);

			let mut account_attend_weight: PowerSize = 0;
			let mut comment_power_weight: PowerSize = 0;
			let mut doc_comment_top_weight: PowerSize = 0;
			let mut doc_judge_weight: u8 = 0;

			// according doc type to decide weight
			match doc.document_type {
				DocumentType::ProductPublish => {
					account_attend_weight = T::TopWeightAccountAttend::get() as PowerSize;
					comment_power_weight = T::DocumentPowerWeightAttend::get() as PowerSize;
					doc_comment_top_weight = T::TopWeightProductPublish::get() as PowerSize;
					doc_judge_weight = T::DocumentPowerWeightJudge::get()
				},
				DocumentType::ProductIdentify => {
					account_attend_weight = T::TopWeightAccountAttend::get() as PowerSize;
					comment_power_weight = T::DocumentPowerWeightAttend::get() as PowerSize;
					doc_comment_top_weight = T::TopWeightDocumentIdentify::get() as PowerSize;
					doc_judge_weight = T::DocumentPowerWeightJudge::get()
				},
				DocumentType::ProductTry => {
					account_attend_weight = T::TopWeightAccountAttend::get() as PowerSize;
					comment_power_weight = T::DocumentPowerWeightAttend::get() as PowerSize;
					doc_comment_top_weight = T::TopWeightDocumentTry::get() as PowerSize;
					doc_judge_weight = T::DocumentPowerWeightJudge::get()
				},
				DocumentType::ProductChoose => {
					account_attend_weight = T::CMPowerAccountAttend::get() as PowerSize;
					comment_power_weight = T::DocumentCMPowerWeightAttend::get() as PowerSize;
					doc_comment_top_weight = 100 as PowerSize;
					doc_judge_weight = T::DocumentCMPowerWeightJudge::get();
				},
				DocumentType::ModelCreate => {
					account_attend_weight = T::CMPowerAccountAttend::get() as PowerSize;
					comment_power_weight = T::DocumentCMPowerWeightAttend::get() as PowerSize;
					doc_comment_top_weight = 100 as PowerSize;
					doc_judge_weight = T::DocumentCMPowerWeightJudge::get();
				},
				_ => {},
			}

			account_comment_power = Self::compute_attend_power(
				Self::compute_comment_action_rate(
					&account_comment_max,
					account.count,
					account.fees,
					account.positive_count,
					account_comment_unit_fee,
				),
				100,
				account_attend_weight,
			);

			// read out document based max record
			let mut doc_comment_max = Self::comment_max_info_per_doc_map(comment.app_id);
			let doc_comment_unit_fee = doc.comment_total_fee / doc.comment_count;
			let is_doc_max_updated = Self::update_comment_max(
				&mut doc_comment_max,
				doc.comment_count,
				doc.comment_total_fee,
				doc.comment_positive_count,
				doc_comment_unit_fee,
			);

			// compute document attend power
			// get this document's compare base first
			let mut compare_base: CommentMaxRecord;
			if <DocumentCommentPowerBase<T>>::contains_key(comment.app_id, &comment.document_id) {
				compare_base =
					<DocumentCommentPowerBase<T>>::get(comment.app_id, &comment.document_id);
				// check if we need to update the compare_base
				let is_compare_base_updated = Self::update_comment_max(
					&mut compare_base,
					doc.comment_count,
					doc.comment_total_fee,
					doc.comment_positive_count,
					doc_comment_unit_fee,
				);

				if is_compare_base_updated {
					<DocumentCommentPowerBase<T>>::insert(
						comment.app_id,
						&comment.document_id,
						&compare_base,
					);
				}
			} else {
				// not exist, this is the first comment of this document
				<DocumentCommentPowerBase<T>>::insert(
					comment.app_id,
					&comment.document_id,
					&doc_comment_max,
				);
				compare_base = doc_comment_max.clone();
			}
			doc_comment_power = Self::compute_attend_power(
				Self::compute_comment_action_rate(
					&compare_base,
					doc.comment_count,
					doc.comment_total_fee,
					doc.comment_positive_count,
					doc_comment_unit_fee,
				),
				comment_power_weight,
				doc_comment_top_weight,
			);

			// chcek if owner's membership
			let mut platform_comment_power: PowerSize = 0;
			let mut is_normal_comment = true;
			if doc.expert_trend == CommentTrend::Empty
				&& T::Membership::is_expert(&comment.sender, doc.app_id, &doc.model_id)
			{
				doc.expert_trend = comment.comment_trend.into();
				platform_comment_power = Self::compute_judge_power(
					Self::compute_doc_trend_power(&doc),
					doc_comment_top_weight,
					doc_judge_weight,
				);

				// give expert comment reward
				Self::give_comment_reward(false, &comment.sender, comment.comment_fee);
				is_normal_comment = false;
			}
			if doc.platform_trend == CommentTrend::Empty
				&& T::Membership::is_platform(&comment.sender, doc.app_id)
			{
				doc.platform_trend = comment.comment_trend.into();
				platform_comment_power = Self::compute_judge_power(
					Self::compute_doc_trend_power(&doc),
					doc_comment_top_weight,
					doc_judge_weight,
				);

				// give platform comment reward
				Self::give_comment_reward(false, &comment.sender, comment.comment_fee);
				is_normal_comment = false;
			}

			if is_normal_comment {
				Self::give_comment_reward(true, &comment.sender, comment.comment_fee);
			}

			// below are write actions

			// update document record

			<KPDocumentDataByIdHash<T>>::insert(comment.app_id, &comment.document_id, &doc);

			// update account record
			<KPCommentAccountRecordMap<T>>::insert(comment.app_id, &comment.sender, &account);

			// update account max if changed
			if is_account_max_updated {
				<CommentMaxInfoPerAccountMap<T>>::insert(comment.app_id, account_comment_max);
			}

			// update doc comment max if changed
			if is_doc_max_updated {
				<CommentMaxInfoPerDocMap<T>>::insert(comment.app_id, doc_comment_max);
			}

			// update account attend power store
			//let key = T::Hashing::hash_of(&(&comment.sender, comment.app_id));
			<AccountAttendPowerMap<T>>::insert(
				&comment.sender,
				comment.app_id,
				account_comment_power,
			);

			// update document attend power store
			Self::update_document_power(&doc, doc_comment_power, platform_comment_power, 0);

			Self::update_document_comment_pool(&comment, &doc);

			// update account statistics
			<AccountStatisticsMap<T>>::mutate(&comment.sender, |info| {
				info.comment_num += 1;
				info.comment_cost_total += comment.comment_fee;

				if comment.comment_trend == 0 {
					info.comment_positive_trend_num += 1;
				} else {
					info.comment_negative_trend_num += 1;
				}

				if comment.comment_fee > info.comment_cost_max {
					info.comment_cost_max = comment.comment_fee;
				}
			});
		}

		fn choose_finance_member() -> Result<T::AccountId, sp_runtime::DispatchError> {
			let (seed, _) = T::Randomness::random(b"ctt_power");
			// seed needs to be guaranteed to be 32 bytes.
			let seed = <[u8; 32]>::decode(&mut TrailingZeroInput::new(seed.as_ref()))
				.expect("input is padded with zeroes; qed");
			let mut rng = ChaChaRng::from_seed(seed);

			let members = T::Membership::valid_finance_members();

			return if let Some(member) = pick_item(&mut rng, &members) {
				Ok(member.clone())
			} else {
				Err(Error::<T>::NotFoundValidFinanceMember.into())
			};
		}

		pub fn is_commodity_in_black_list(app_id: u32, cart_id: Vec<u8>) -> bool {
			<KPPurchaseBlackList<T>>::contains_key(app_id, &cart_id)
		}

		fn get_purchase_power(app_id: u32, cart_id: &Vec<u8>) -> PowerSize {
			let power = <KPPurchasePowerByIdHash<T>>::get(app_id, cart_id);
			Self::compute_commodity_power(&power)
		}

		fn slash_power(app_id: u32, cart_id: &Vec<u8>, power_owner: &T::AccountId) {
			let cart_power = Self::get_purchase_power(app_id, cart_id);
			// print("slash_power");
			// print(cart_power);
			if cart_power > 0 {
				// clear power
				Self::clear_purchase_power(app_id, cart_id);
				// reduce account power
				<MinerPowerByAccount<T>>::mutate(power_owner, |pow| {
					if *pow > cart_power {
						*pow -= cart_power;
					} else {
						*pow = 0
					}
				});

				// update account statistics
				<AccountStatisticsMap<T>>::mutate(power_owner, |info| {
					info.slash_commodity_num += 1;
					info.slash_kp_total += cart_power;
				});
			}
		}

		fn model_dispute(
			app_id: u32,
			model_id: &Vec<u8>,
			dispute_type: ModelDisputeType,
			owner: &T::AccountId,
			reporter: &T::AccountId,
		) {
			let current_block = frame_system::Pallet::<T>::block_number();
			// get current model income cycle index
			let cycle = Self::model_income_cycle_index(current_block);
			//let key = T::Hashing::hash_of(&(app_id, model_id));

			// get model cycle dispute count
			let mut cycle_dispute_count =
				<ModelCycleDisputeCount<T>>::get((cycle, app_id, model_id)).unwrap();

			let cancel_model_cycle_reward = || {
				<ModelSlashCycleRewardIndex<T>>::insert(app_id, model_id, cycle);
			};

			let reporter_reward: BalanceOf<T>;

			match dispute_type {
				ModelDisputeType::NoneIntendNormal => {
					cycle_dispute_count += 1;
					reporter_reward = T::ModelDisputeRewardLv1::get();
				},
				ModelDisputeType::IntendNormal => {
					// check if cycle count reach max
					if cycle_dispute_count >= T::ModelDisputeCycleCount::get() {
						cancel_model_cycle_reward();
					}

					cycle_dispute_count += T::ModelDisputeCycleLv2IncreaseCount::get();
					reporter_reward = T::ModelDisputeRewardLv2::get();
				},
				ModelDisputeType::Serious => {
					if cycle_dispute_count >= T::ModelDisputeCycleCount::get() {
						cancel_model_cycle_reward();
						<KPModelDataByIdHash<T>>::mutate(app_id, model_id, |model| {
							model.status = ModelStatus::DISABLED;
						});

						T::Slash::on_unbalanced(
							T::Currency::slash_reserved(
								owner,
								<KPModelDepositMap<T>>::get(app_id, model_id),
							)
							.0,
						);
					}

					cycle_dispute_count += T::ModelDisputeCycleLv3IncreaseCount::get();
					reporter_reward = T::ModelDisputeRewardLv3::get();
				},
			}

			<ModelCycleDisputeCount<T>>::insert((cycle, app_id, model_id), cycle_dispute_count);

			// reward reporter
			let treasury_account: T::AccountId =
				T::TreasuryModuleId::get().into_account_truncating();
			T::Currency::transfer(&treasury_account, &reporter, reporter_reward, KeepAlive).ok();
		}

		fn compute_tech_fund_withdraw(
			dev_type: TechFundWithdrawType,
			dev_level: TechFundWithdrawLevel,
		) -> BalanceOf<T> {
			let base: BalanceOf<T> = T::TechFundBase::get();

			let type_per = match dev_type {
				TechFundWithdrawType::ChainDev => {
					Permill::from_rational_approximation(45u32, 100u32)
				},
				TechFundWithdrawType::Tctp => Permill::from_rational_approximation(30u32, 100u32),
				TechFundWithdrawType::Model => Permill::from_rational_approximation(8u32, 100u32),
				TechFundWithdrawType::Knowledge => {
					Permill::from_rational_approximation(5u32, 100u32)
				},
				TechFundWithdrawType::ChainAdmin => {
					Permill::from_rational_approximation(12u32, 100u32)
				},
			};

			let level_per = match dev_level {
				TechFundWithdrawLevel::LV1 => Permill::from_rational_approximation(25u32, 100u32),
				TechFundWithdrawLevel::LV2 => Permill::from_rational_approximation(10u32, 100u32),
				TechFundWithdrawLevel::LV3 => Permill::from_rational_approximation(5u32, 100u32),
				TechFundWithdrawLevel::LV4 => Permill::from_rational_approximation(1u32, 100u32),
				TechFundWithdrawLevel::LV5 => Permill::from_rational_approximation(2u32, 1000u32),
			};

			let amount = type_per * base;
			level_per * amount
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
