use core::{cmp::Ordering, fmt::Debug};

use super::{
    env::internal::{Env as _, EnvBase as _},
    xdr::ScObjectType,
    ConversionError, Env, Object, RawVal, TryFromVal,
};

#[cfg(not(target_family = "wasm"))]
use crate::env::internal::xdr::ScVal;
use crate::{unwrap::UnwrapInfallible, Vec};

/// Address is a universal opaque identifier to use in contracts.
///
/// Address can be used as an input argument (for example, to identify the
/// payment recipient), as a data key (for example, to store the balance), as
/// the authentication & authorization source (for example, to authorize the
/// token transfer) etc.
///
/// See `require_auth` documentation for more details on using Address for
/// authorization.
///
/// Internally, Address may represent a Stellar account or a contract. Contract
/// address may be used to identify the account contracts - special contracts
/// that allow customizing authentication logic and adding custom authorization
/// rules.
///
/// In tests Addresses should be generated via `Address::random()`.
#[derive(Clone)]
pub struct Address {
    env: Env,
    obj: Object,
}

impl Debug for Address {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        #[cfg(target_family = "wasm")]
        write!(f, "Address(..)")?;
        #[cfg(not(target_family = "wasm"))]
        {
            use crate::env::internal::xdr;
            use stellar_strkey::{ed25519, Contract, Strkey};
            let sc_val = ScVal::try_from(self).map_err(|_| core::fmt::Error)?;
            if let ScVal::Object(Some(xdr::ScObject::Address(addr))) = sc_val {
                match addr {
                    xdr::ScAddress::Account(account_id) => {
                        let xdr::AccountId(xdr::PublicKey::PublicKeyTypeEd25519(xdr::Uint256(
                            ed25519,
                        ))) = account_id;
                        let strkey = Strkey::PublicKeyEd25519(ed25519::PublicKey(ed25519));
                        write!(f, "AccountId({})", strkey.to_string())?;
                    }
                    xdr::ScAddress::Contract(contract_id) => {
                        let strkey = Strkey::Contract(Contract(contract_id.0));
                        write!(f, "Contract({})", strkey.to_string())?;
                    }
                }
            } else {
                return Err(core::fmt::Error);
            }
        }
        Ok(())
    }
}

impl Eq for Address {}

impl PartialEq for Address {
    fn eq(&self, other: &Self) -> bool {
        self.partial_cmp(other) == Some(Ordering::Equal)
    }
}

impl PartialOrd for Address {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(Ord::cmp(self, other))
    }
}

impl Ord for Address {
    fn cmp(&self, other: &Self) -> Ordering {
        self.env.check_same_env(&other.env);
        let v = self
            .env
            .obj_cmp(self.obj.to_raw(), other.obj.to_raw())
            .unwrap_infallible();
        v.cmp(&0)
    }
}

impl TryFromVal<Env, Object> for Address {
    type Error = ConversionError;

    fn try_from_val(env: &Env, val: &Object) -> Result<Self, Self::Error> {
        if val.is_obj_type(ScObjectType::Address) {
            Ok(unsafe { Address::unchecked_new(env.clone(), *val) })
        } else {
            Err(ConversionError {})
        }
    }
}

impl TryFromVal<Env, RawVal> for Address {
    type Error = <Address as TryFromVal<Env, Object>>::Error;

    fn try_from_val(env: &Env, val: &RawVal) -> Result<Self, Self::Error> {
        <_ as TryFromVal<_, Object>>::try_from_val(env, &val.try_into()?)
    }
}

impl TryFromVal<Env, Address> for RawVal {
    type Error = ConversionError;

    fn try_from_val(_env: &Env, v: &Address) -> Result<Self, Self::Error> {
        Ok(v.to_raw())
    }
}

impl TryFromVal<Env, &Address> for RawVal {
    type Error = ConversionError;

    fn try_from_val(_env: &Env, v: &&Address) -> Result<Self, Self::Error> {
        Ok(v.to_raw())
    }
}

#[cfg(not(target_family = "wasm"))]
impl TryFrom<&Address> for ScVal {
    type Error = ConversionError;
    fn try_from(v: &Address) -> Result<Self, Self::Error> {
        ScVal::try_from_val(&v.env, &v.obj.to_raw())
    }
}

#[cfg(not(target_family = "wasm"))]
impl TryFrom<Address> for ScVal {
    type Error = ConversionError;
    fn try_from(v: Address) -> Result<Self, Self::Error> {
        (&v).try_into()
    }
}

#[cfg(not(target_family = "wasm"))]
impl TryFromVal<Env, ScVal> for Address {
    type Error = ConversionError;
    fn try_from_val(env: &Env, val: &ScVal) -> Result<Self, Self::Error> {
        use soroban_env_host::TryIntoVal;
        <_ as TryFromVal<_, Object>>::try_from_val(
            env,
            &val.try_into_val(env).map_err(|_| ConversionError)?,
        )
    }
}

impl Address {
    /// Ensures that this Address has authorized invocation of the current
    /// contract with the provided arguments.
    ///
    /// During the on-chain execution the Soroban host will perform the needed
    /// authentication (verify the signatures) and ensure the replay prevention.
    /// The contracts don't need to perform this tasks.
    ///
    /// The arguments don't have to match the arguments of the contract
    /// invocation. However, it's considered the best practice to have a
    /// well-defined, deterministic and ledger-state-independent mapping between
    /// the contract invocation arguments and `require_auth` arguments. This
    /// will allow the contract callers to easily build the required signature
    /// payloads and prevent potential authorization failures.
    ///
    /// When called in the tests, the `require_auth` calls are just recorded and
    /// no signatures are required. In order to make sure that the contract
    /// has indeed called `require_auth` for this Address with expected arguments
    /// use `env.verify_top_authorization`.
    ///
    /// ### Panics
    ///
    /// If the invocation is not authorized.
    pub fn require_auth_for_args(&self, args: Vec<RawVal>) {
        self.env.require_auth_for_args(&self, args);
    }

    /// Ensures that this Address has authorized invocation of the current
    /// contract with all the invocation arguments
    ///
    /// This works exactly in the same fashion as `require_auth_for_args`, but
    /// arguments are automatically inferred from the current contract
    /// invocation.
    ///
    /// This is useful when there is only a single Address that needs to
    /// authorize the contract invocation and there are no dynamic arguments
    /// that don't need authorization.
    ///
    /// ### Panics
    ///
    /// If the invocation is not authorized.    
    pub fn require_auth(&self) {
        self.env.require_auth(&self);
    }

    #[inline(always)]
    pub(crate) unsafe fn unchecked_new(env: Env, obj: Object) -> Self {
        Self { env, obj }
    }

    #[inline(always)]
    pub fn env(&self) -> &Env {
        &self.env
    }

    pub fn as_raw(&self) -> &RawVal {
        self.obj.as_raw()
    }

    pub fn to_raw(&self) -> RawVal {
        self.obj.to_raw()
    }

    pub fn as_object(&self) -> &Object {
        &self.obj
    }

    pub fn to_object(&self) -> Object {
        self.obj
    }
}

#[cfg(any(test, feature = "testutils"))]
use crate::env::xdr::{Hash, ScAddress, ScObject};
#[cfg(any(test, feature = "testutils"))]
use crate::{testutils::random, BytesN};
#[cfg(any(test, feature = "testutils"))]
#[cfg_attr(feature = "docs", doc(cfg(feature = "testutils")))]
impl crate::testutils::Address for Address {
    fn from_contract_id(env: &Env, contract_id: &BytesN<32>) -> Self {
        let sc_addr = ScVal::Object(Some(ScObject::Address(ScAddress::Contract(Hash(
            contract_id.to_array(),
        )))));
        Self::try_from_val(env, &sc_addr).unwrap()
    }

    fn random(env: &Env) -> Self {
        let sc_addr = ScVal::Object(Some(ScObject::Address(ScAddress::Contract(Hash(random())))));
        Self::try_from_val(env, &sc_addr).unwrap()
    }
}
