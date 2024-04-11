use std::collections::{BTreeMap, HashMap};

use serde_json::Value;

use super::allocation::{GenesisAccountAlloc, GenesisAllocation, GenesisContractAlloc};
use super::json::GenesisJsonError;
use super::{FeeTokenConfig, Genesis, GenesisClass, UniversalDeployerConfig};
use crate::block::{BlockHash, BlockNumber, GasPrices};
use crate::class::ClassHash;
use crate::contract::ContractAddress;
use crate::genesis::json::parse_genesis_class_artifacts;
use crate::FieldElement;

#[derive(Debug, thiserror::Error)]
pub enum GenesisBuilderError {
    #[error("Parent hash not set")]
    ParentHashNotSet,
    #[error("State root not set")]
    StateRootNotSet,
    #[error("Timestamp not set")]
    TimestampNotSet,
    #[error("Block number not set")]
    NumberNotSet,
    #[error("Sequencer address not set")]
    SequencerAddressNotSet,
    #[error("L1 gas prices not set")]
    L1GasPricesNotSet,
    #[error("Fee token not set")]
    FeeTokenNotSet,
    #[error("No class found with hash {class_hash:#x} for contract {contract}")]
    UnknownClassHash { contract: ContractAddress, class_hash: ClassHash },
    #[error("Error parsing the class artifact: {0}")]
    ClassParsingError(#[from] GenesisJsonError),
    #[error("Contract allocation is missing a class hash")]
    MissingClassHash,
}

#[derive(Debug)]
pub struct Builder {
    parent_hash: Option<BlockHash>,
    state_root: Option<FieldElement>,
    number: Option<BlockNumber>,
    timestamp: Option<u64>,
    sequencer_address: Option<ContractAddress>,
    gas_prices: Option<GasPrices>,
    fee_token: Option<FeeTokenConfig>,
    udc: Option<UniversalDeployerConfig>,
    raw_classes: Vec<(Value, Option<ClassHash>)>,
    allocations: BTreeMap<ContractAddress, GenesisAllocation>,
    // for compatibility when creating a new builder from an existing genesis
    classes: HashMap<ClassHash, GenesisClass>,
}

impl Builder {
    pub fn parent_hash(self, hash: BlockHash) -> Self {
        Self { parent_hash: Some(hash), ..self }
    }

    pub fn state_root(self, state_root: FieldElement) -> Self {
        Self { state_root: Some(state_root), ..self }
    }

    pub fn number(self, number: BlockNumber) -> Self {
        Self { number: Some(number), ..self }
    }

    pub fn timestamp(self, timestamp: u64) -> Self {
        Self { timestamp: Some(timestamp), ..self }
    }

    pub fn sequencer_address(self, address: ContractAddress) -> Self {
        Self { sequencer_address: Some(address), ..self }
    }

    pub fn gas_prices(self, gas_prices: GasPrices) -> Self {
        Self { gas_prices: Some(gas_prices), ..self }
    }

    pub fn fee_token(self, fee_token: FeeTokenConfig) -> Self {
        Self { fee_token: Some(fee_token), ..self }
    }

    pub fn universal_deployer(self, udc: UniversalDeployerConfig) -> Self {
        Self { udc: Some(udc), ..self }
    }

    pub fn add_classes<I>(mut self, classes: I) -> Self
    where
        I: Iterator<Item = (Value, Option<ClassHash>)>,
    {
        self.raw_classes.extend(classes);
        self
    }

    pub fn add_accounts<I>(mut self, accounts: I) -> Self
    where
        I: IntoIterator<Item = (ContractAddress, GenesisAccountAlloc)>,
    {
        let accounts = accounts
            .into_iter()
            .map(|(address, alloc)| (address, GenesisAllocation::Account(alloc)));
        self.allocations.extend(accounts);
        self
    }

    pub fn add_contracts<I>(mut self, contracts: I) -> Self
    where
        I: IntoIterator<Item = (ContractAddress, GenesisContractAlloc)>,
    {
        let contracts = contracts
            .into_iter()
            .map(|(address, alloc)| (address, GenesisAllocation::Contract(alloc)));
        self.allocations.extend(contracts);
        self
    }

    pub fn build(mut self) -> Result<Genesis, GenesisBuilderError> {
        for (class, hash) in self.raw_classes {
            let (hash, class) = parse_genesis_class_artifacts(hash, class)?;
            self.classes.entry(hash).or_insert(class);
        }

        for (address, alloc) in &mut self.allocations {
            let class_hash = alloc.class_hash().ok_or(GenesisBuilderError::MissingClassHash)?;
            if !self.classes.contains_key(&class_hash) {
                return Err(GenesisBuilderError::UnknownClassHash {
                    class_hash,
                    contract: *address,
                });
            }
        }

        let parent_hash = self.parent_hash.ok_or(GenesisBuilderError::ParentHashNotSet)?;
        let state_root = self.state_root.ok_or(GenesisBuilderError::StateRootNotSet)?;
        let number = self.number.ok_or(GenesisBuilderError::NumberNotSet)?;
        let timestamp = self.timestamp.ok_or(GenesisBuilderError::TimestampNotSet)?;
        let seq_addr = self.sequencer_address.ok_or(GenesisBuilderError::SequencerAddressNotSet)?;
        let fee_token = self.fee_token.ok_or(GenesisBuilderError::FeeTokenNotSet)?;
        let gas_prices = self.gas_prices.ok_or(GenesisBuilderError::L1GasPricesNotSet)?;

        Ok(Genesis {
            parent_hash,
            state_root,
            number,
            timestamp,
            sequencer_address: seq_addr,
            gas_prices,
            fee_token,
            universal_deployer: self.udc,
            classes: self.classes,
            allocations: self.allocations,
        })
    }
}

impl From<Genesis> for Builder {
    fn from(value: Genesis) -> Self {
        Self {
            parent_hash: Some(value.parent_hash),
            state_root: Some(value.state_root),
            number: Some(value.number),
            timestamp: Some(value.timestamp),
            sequencer_address: Some(value.sequencer_address),
            gas_prices: Some(value.gas_prices),
            fee_token: Some(value.fee_token),
            udc: value.universal_deployer,
            raw_classes: Vec::new(),
            allocations: value.allocations,
            classes: value.classes,
        }
    }
}
