// Copyright (c) 2019 Alain Brenzikofer
// This file is part of Encointer
//
// Encointer is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// Encointer is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with Encointer.  If not, see <http://www.gnu.org/licenses/>.

//! # Encointer Cross-Chain Transfer Module
//!
//! provides functionality for
//! - upward native token transfer
//!
//! This is intended for intermediate usage only until there are upward transfers available
//! natively on rococo-v1. Currently, this is only possible as a sudo call and it needs to be
//! composed tediously a generic `Xcm::WithdrawAsset` call in the polkadot-ui.

#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{
    debug, decl_error, decl_event, decl_module, decl_storage, dispatch::DispatchResult,
    traits::Get, Parameter,
};
use frame_system::ensure_signed;
use polkadot_parachain::primitives::Id as ParaId;
use rstd::prelude::*;
use rstd::vec;
use sp_runtime::traits::{AtLeast32BitUnsigned, MaybeSerializeDeserialize, Member, StaticLookup};
use xcm::v0::{
    Error as XcmError, ExecuteXcm, Junction, MultiAsset, MultiLocation, NetworkId, Order, Xcm,
};
use xcm_executor::traits::LocationConversion;

const LOG: &str = "encointer";

pub trait Config: frame_system::Config {
    type Event: From<Event<Self>> + Into<<Self as frame_system::Config>::Event>;
    type AccountId32: Into<[u8; 32]>
        + From<[u8; 32]>
        + Clone
        + From<<Self as frame_system::Config>::AccountId>
        + Into<<Self as frame_system::Config>::AccountId>;
    type Balance: Parameter
        + Member
        + AtLeast32BitUnsigned
        + Default
        + Copy
        + MaybeSerializeDeserialize
        + Into<u128>;
    type XcmExecutor: ExecuteXcm;
    type AccountIdConverter: LocationConversion<Self::AccountId>;
    type RelayChainNetworkId: Get<NetworkId>;
    type ParaId: Get<ParaId>;
}

decl_storage! {
    trait Store for Module<T: Config> as XcTransfer {}
}

decl_event! {
    pub enum Event<T> where
        <T as frame_system::Config>::AccountId,
        <T as Config>::Balance,
    {
        /// Transferred DOT to relay chain. [src, dest, amount]
        TransferredUpwards(AccountId, AccountId, Balance),
        /// DOT to relay chain transfer failed. [src, dest, amount]
        TransferUpwardsFailed(XcmError),
    }
}

decl_module! {
    pub struct Module<T: Config> for enum Call where origin: T::Origin {
        fn deposit_event() = default;
        type Error = Error<T>;

        #[weight = 5_000_000]
        pub fn transfer_upwards(origin, recipient: <T::Lookup as StaticLookup>::Source, amount: T::Balance) -> DispatchResult {
            let sender = ensure_signed(origin.clone())?;

            let recipient = T::Lookup::lookup(recipient)?;

            let xcm = Self::make_xcm_upward_transfer(recipient.clone().into(), amount);
            debug::debug!(target: LOG, "executing upward transfer: {:?}", xcm);

            let xcm_origin = T::AccountIdConverter::try_into_location(sender.clone())
                .map_err(|_| Error::<T>::BadLocation)?;

            match T::XcmExecutor::execute_xcm(xcm_origin, xcm) {
                Ok(()) => {
                    Self::deposit_event(RawEvent::TransferredUpwards(sender, recipient, amount))
                },
                Err(e) => Self::deposit_event(RawEvent::TransferUpwardsFailed(e)),
            }

            Ok(())
        }
    }
}

decl_error! {
    pub enum Error for Module<T: Config> {
        /// Location given was invalid or unsupported.
        BadLocation,
        /// The XCM message version is not supported.
        BadVersion,
        /// The XCM message was not executed locally
        ExecutionFailed
    }
}

impl<T: Config> Module<T> {
    // Transfer DOT upwards to relay chain
    fn make_xcm_upward_transfer(recipient: T::AccountId32, amount: T::Balance) -> Xcm {
        Xcm::WithdrawAsset {
            assets: vec![MultiAsset::ConcreteFungible {
                id: MultiLocation::X1(Junction::Parent),
                amount: amount.into(),
            }],
            effects: vec![Order::InitiateReserveWithdraw {
                assets: vec![MultiAsset::All],
                reserve: MultiLocation::X1(Junction::Parent),
                effects: vec![Order::DepositAsset {
                    assets: vec![MultiAsset::All],
                    dest: MultiLocation::X1(Junction::AccountId32 {
                        network: T::RelayChainNetworkId::get(),
                        id: recipient.clone().into(),
                    }),
                }],
            }],
        }
    }
}
