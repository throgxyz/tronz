use alloy_json_abi::{
    Constructor, Error as JsonAbiError, Event, EventParam, Fallback, Function, JsonAbi, Param,
    Receive, StateMutability,
};

use crate::{
    TronAbi, TronAbiConversionError, TronAbiEntry, TronAbiEntryType, TronAbiParam,
    TronAbiStateMutability,
};

fn validate_name(name: &str, allow_empty: bool) -> Result<(), TronAbiConversionError> {
    if (allow_empty && name.is_empty()) || alloy_json_abi::parser::is_valid_identifier(name) {
        Ok(())
    } else {
        Err(TronAbiConversionError::InvalidName(name.into()))
    }
}

fn param_from_json(param: &Param) -> Result<TronAbiParam, TronAbiConversionError> {
    validate_name(&param.name, true)?;
    if param.ty.starts_with("tuple") && param.components.is_empty() {
        return Err(TronAbiConversionError::IncompleteTuple(param.name.clone()));
    }

    let ty = param.selector_type().into_owned();
    if ty.is_empty() {
        return Err(TronAbiConversionError::InvalidType(param.ty.clone()));
    }

    Ok(TronAbiParam { indexed: false, name: param.name.clone(), ty })
}

fn event_param_from_json(param: &EventParam) -> Result<TronAbiParam, TronAbiConversionError> {
    validate_name(&param.name, true)?;
    if param.ty.starts_with("tuple") && param.components.is_empty() {
        return Err(TronAbiConversionError::IncompleteTuple(param.name.clone()));
    }

    let ty = param.selector_type().into_owned();
    if ty.is_empty() {
        return Err(TronAbiConversionError::InvalidType(param.ty.clone()));
    }

    Ok(TronAbiParam { indexed: param.indexed, name: param.name.clone(), ty })
}

fn param_to_json(param: &TronAbiParam) -> Result<Param, TronAbiConversionError> {
    validate_name(&param.name, true)?;
    if param
        .ty
        .strip_prefix("tuple")
        .is_some_and(|suffix| suffix.is_empty() || suffix.starts_with('['))
    {
        return Err(TronAbiConversionError::IncompleteTuple(param.name.clone()));
    }

    let declaration = if param.name.is_empty() {
        param.ty.clone()
    } else {
        format!("{} {}", param.ty, param.name)
    };
    Param::parse(&declaration).map_err(|_| TronAbiConversionError::InvalidType(param.ty.clone()))
}

fn event_param_to_json(param: &TronAbiParam) -> Result<EventParam, TronAbiConversionError> {
    let param_json = param_to_json(param)?;
    Ok(EventParam {
        ty: param_json.ty,
        name: param_json.name,
        indexed: param.indexed,
        components: param_json.components,
        internal_type: param_json.internal_type,
    })
}

fn state_from_json(value: StateMutability) -> TronAbiStateMutability {
    match value {
        StateMutability::Pure => TronAbiStateMutability::Pure,
        StateMutability::View => TronAbiStateMutability::View,
        StateMutability::NonPayable => TronAbiStateMutability::NonPayable,
        StateMutability::Payable => TronAbiStateMutability::Payable,
    }
}

fn state_to_json(
    value: TronAbiStateMutability,
    constant: bool,
    payable: bool,
    receive: bool,
) -> Result<StateMutability, TronAbiConversionError> {
    match value {
        TronAbiStateMutability::Unknown(0) if constant && (receive || payable) => {
            Err(TronAbiConversionError::ConflictingMutability)
        }
        TronAbiStateMutability::Unknown(0) if receive || payable => Ok(StateMutability::Payable),
        TronAbiStateMutability::Unknown(0) if constant => Ok(StateMutability::View),
        TronAbiStateMutability::Unknown(0) => Ok(StateMutability::NonPayable),
        TronAbiStateMutability::Unknown(other) => {
            Err(TronAbiConversionError::InvalidMutability(other))
        }
        TronAbiStateMutability::Pure => Ok(StateMutability::Pure),
        TronAbiStateMutability::View => Ok(StateMutability::View),
        TronAbiStateMutability::NonPayable => Ok(StateMutability::NonPayable),
        TronAbiStateMutability::Payable => Ok(StateMutability::Payable),
    }
}

fn entry_from_json(
    entry_type: TronAbiEntryType,
    name: String,
    inputs: Vec<TronAbiParam>,
    outputs: Vec<TronAbiParam>,
    state_mutability: TronAbiStateMutability,
    anonymous: bool,
) -> TronAbiEntry {
    let constant =
        matches!(state_mutability, TronAbiStateMutability::Pure | TronAbiStateMutability::View);
    let payable = state_mutability == TronAbiStateMutability::Payable;
    TronAbiEntry {
        entry_type,
        name,
        inputs,
        outputs,
        anonymous,
        constant,
        payable,
        state_mutability,
    }
}

impl TronAbi {
    /// Converts an Alloy JSON ABI into TRON's metadata model.
    ///
    /// Tuple component types are flattened to their canonical selector form
    /// because TRON metadata has no `components` or `internalType` fields. This
    /// preserves component types but discards component names and internal
    /// Solidity types. Item order follows Alloy's grouped iteration order, not
    /// the original JSON array order.
    ///
    /// # Errors
    ///
    /// Returns an error if an item contains an invalid identifier or a tuple
    /// does not include the component types needed to form its selector type.
    ///
    /// # Examples
    ///
    /// ```
    /// use tronz_abi::{JsonAbi, TronAbi};
    ///
    /// let json_abi = JsonAbi::parse([
    ///     "function balanceOf(address owner)(uint256 balance)",
    ///     "event Transfer(address indexed from, address indexed to, uint256 value)",
    /// ])?;
    /// let tron_abi = TronAbi::try_from_json_abi(&json_abi)?;
    ///
    /// assert_eq!(tron_abi.len(), 2);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn try_from_json_abi(abi: &JsonAbi) -> Result<Self, TronAbiConversionError> {
        let mut entries = Vec::with_capacity(abi.len());

        if let Some(item) = &abi.constructor {
            let inputs = item.inputs.iter().map(param_from_json).collect::<Result<_, _>>()?;
            entries.push(entry_from_json(
                TronAbiEntryType::Constructor,
                String::new(),
                inputs,
                vec![],
                state_from_json(item.state_mutability),
                false,
            ));
        }
        if let Some(item) = &abi.fallback {
            entries.push(entry_from_json(
                TronAbiEntryType::Fallback,
                String::new(),
                vec![],
                vec![],
                state_from_json(item.state_mutability),
                false,
            ));
        }
        if let Some(item) = &abi.receive {
            entries.push(entry_from_json(
                TronAbiEntryType::Receive,
                String::new(),
                vec![],
                vec![],
                state_from_json(item.state_mutability),
                false,
            ));
        }
        for item in abi.functions() {
            validate_name(&item.name, false)?;
            let inputs = item.inputs.iter().map(param_from_json).collect::<Result<_, _>>()?;
            let outputs = item.outputs.iter().map(param_from_json).collect::<Result<_, _>>()?;
            entries.push(entry_from_json(
                TronAbiEntryType::Function,
                item.name.clone(),
                inputs,
                outputs,
                state_from_json(item.state_mutability),
                false,
            ));
        }
        for item in abi.events() {
            validate_name(&item.name, false)?;
            let inputs = item.inputs.iter().map(event_param_from_json).collect::<Result<_, _>>()?;
            entries.push(entry_from_json(
                TronAbiEntryType::Event,
                item.name.clone(),
                inputs,
                vec![],
                TronAbiStateMutability::Unknown(0),
                item.anonymous,
            ));
        }
        for item in abi.errors() {
            validate_name(&item.name, false)?;
            let inputs = item.inputs.iter().map(param_from_json).collect::<Result<_, _>>()?;
            entries.push(entry_from_json(
                TronAbiEntryType::Error,
                item.name.clone(),
                inputs,
                vec![],
                TronAbiStateMutability::Unknown(0),
                false,
            ));
        }

        Ok(Self { entries })
    }

    /// Converts TRON contract metadata into an Alloy JSON ABI.
    ///
    /// Entry order is not retained because [`JsonAbi`] groups functions,
    /// events, and errors by name. Legacy `constant` and `payable` flags are
    /// used only when `state_mutability` is `Unknown(0)`.
    ///
    /// Each entry is normalized to the fields supported by its Solidity JSON
    /// ABI item kind. Fields with no meaning for that kind, such as outputs on
    /// an event or `anonymous` on a function, are ignored.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid identifiers or types, bare `tuple` types,
    /// unknown entry or mutability values, conflicting legacy mutability flags,
    /// or duplicate constructor, fallback, and receive entries.
    ///
    /// # Examples
    ///
    /// ```
    /// use tronz_abi::{
    ///     JsonAbi, TronAbi, TronAbiEntry, TronAbiEntryType, TronAbiParam, TronAbiStateMutability,
    /// };
    ///
    /// let tron_abi = TronAbi {
    ///     entries: vec![TronAbiEntry {
    ///         entry_type: TronAbiEntryType::Function,
    ///         name: "balanceOf".into(),
    ///         inputs: vec![TronAbiParam::new("owner", "address")],
    ///         outputs: vec![TronAbiParam::new("balance", "uint256")],
    ///         constant: true,
    ///         state_mutability: TronAbiStateMutability::View,
    ///         ..Default::default()
    ///     }],
    /// };
    /// let json_abi = JsonAbi::try_from(&tron_abi)?;
    ///
    /// assert!(json_abi.function("balanceOf").is_some());
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn try_to_json_abi(&self) -> Result<JsonAbi, TronAbiConversionError> {
        let mut result = JsonAbi::new();

        for entry in &self.entries {
            match entry.entry_type {
                TronAbiEntryType::Constructor => {
                    if result.constructor.is_some() {
                        return Err(TronAbiConversionError::DuplicateEntry("constructor"));
                    }
                    let inputs =
                        entry.inputs.iter().map(param_to_json).collect::<Result<_, _>>()?;
                    result.constructor = Some(Constructor {
                        inputs,
                        state_mutability: state_to_json(
                            entry.state_mutability,
                            entry.constant,
                            entry.payable,
                            false,
                        )?,
                    });
                }
                TronAbiEntryType::Function => {
                    validate_name(&entry.name, false)?;
                    let inputs =
                        entry.inputs.iter().map(param_to_json).collect::<Result<_, _>>()?;
                    let outputs =
                        entry.outputs.iter().map(param_to_json).collect::<Result<_, _>>()?;
                    let function = Function {
                        name: entry.name.clone(),
                        inputs,
                        outputs,
                        state_mutability: state_to_json(
                            entry.state_mutability,
                            entry.constant,
                            entry.payable,
                            false,
                        )?,
                    };
                    result.functions.entry(entry.name.clone()).or_default().push(function);
                }
                TronAbiEntryType::Event => {
                    validate_name(&entry.name, false)?;
                    let inputs =
                        entry.inputs.iter().map(event_param_to_json).collect::<Result<_, _>>()?;
                    let event =
                        Event { name: entry.name.clone(), inputs, anonymous: entry.anonymous };
                    result.events.entry(entry.name.clone()).or_default().push(event);
                }
                TronAbiEntryType::Fallback => {
                    if result.fallback.is_some() {
                        return Err(TronAbiConversionError::DuplicateEntry("fallback"));
                    }
                    result.fallback = Some(Fallback {
                        state_mutability: state_to_json(
                            entry.state_mutability,
                            entry.constant,
                            entry.payable,
                            false,
                        )?,
                    });
                }
                TronAbiEntryType::Receive => {
                    if result.receive.is_some() {
                        return Err(TronAbiConversionError::DuplicateEntry("receive"));
                    }
                    result.receive = Some(Receive {
                        state_mutability: state_to_json(
                            entry.state_mutability,
                            entry.constant,
                            entry.payable,
                            true,
                        )?,
                    });
                }
                TronAbiEntryType::Error => {
                    validate_name(&entry.name, false)?;
                    let inputs =
                        entry.inputs.iter().map(param_to_json).collect::<Result<_, _>>()?;
                    let error = JsonAbiError { name: entry.name.clone(), inputs };
                    result.errors.entry(entry.name.clone()).or_default().push(error);
                }
                TronAbiEntryType::Unknown(value) => {
                    return Err(TronAbiConversionError::UnknownEntryType(value));
                }
            }
        }

        Ok(result)
    }
}

impl TryFrom<&JsonAbi> for TronAbi {
    type Error = TronAbiConversionError;

    #[inline]
    fn try_from(abi: &JsonAbi) -> Result<Self, Self::Error> {
        Self::try_from_json_abi(abi)
    }
}

impl TryFrom<JsonAbi> for TronAbi {
    type Error = TronAbiConversionError;

    #[inline]
    fn try_from(abi: JsonAbi) -> Result<Self, Self::Error> {
        Self::try_from_json_abi(&abi)
    }
}

impl TryFrom<&TronAbi> for JsonAbi {
    type Error = TronAbiConversionError;

    #[inline]
    fn try_from(abi: &TronAbi) -> Result<Self, Self::Error> {
        abi.try_to_json_abi()
    }
}

impl TryFrom<TronAbi> for JsonAbi {
    type Error = TronAbiConversionError;

    #[inline]
    fn try_from(abi: TronAbi) -> Result<Self, Self::Error> {
        abi.try_to_json_abi()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_json_abi_round_trip() {
        let abi: JsonAbi = serde_json::from_str(
            r#"[
                {"type":"constructor","inputs":[{"name":"supply","type":"uint256"}],"stateMutability":"nonpayable"},
                {"type":"function","name":"balanceOf","inputs":[{"name":"owner","type":"address"}],"outputs":[{"name":"balance","type":"uint256"}],"stateMutability":"view"},
                {"type":"event","name":"Transfer","inputs":[{"name":"from","type":"address","indexed":true},{"name":"to","type":"address","indexed":true},{"name":"value","type":"uint256","indexed":false}],"anonymous":false},
                {"type":"error","name":"InsufficientBalance","inputs":[{"name":"owner","type":"address"},{"name":"balance","type":"uint256"}]},
                {"type":"fallback","stateMutability":"payable"},
                {"type":"receive","stateMutability":"payable"}
            ]"#,
        )
        .unwrap();

        let tron = TronAbi::try_from(&abi).unwrap();
        let decoded = JsonAbi::try_from(&tron).unwrap();
        assert_eq!(decoded, abi);
    }

    #[test]
    fn tuple_selector_type_is_preserved() {
        let abi: JsonAbi = serde_json::from_str(
            r#"[{
                "type":"function",
                "name":"setPair",
                "inputs":[{
                    "name":"pair",
                    "type":"tuple",
                    "components":[
                        {"name":"amount","type":"uint256"},
                        {"name":"owner","type":"address"}
                    ]
                }],
                "outputs":[],
                "stateMutability":"nonpayable"
            }]"#,
        )
        .unwrap();

        let tron = TronAbi::try_from_json_abi(&abi).unwrap();
        assert_eq!(tron.entries[0].inputs[0].ty, "(uint256,address)");

        let decoded = tron.try_to_json_abi().unwrap();
        let original_type = abi.functions().next().unwrap().inputs[0].selector_type();
        let decoded_type = decoded.functions().next().unwrap().inputs[0].selector_type();
        assert_eq!(decoded_type, original_type);
    }

    #[test]
    fn bare_tuple_remains_readable_but_json_conversion_fails() {
        let tron = TronAbi {
            entries: vec![TronAbiEntry {
                entry_type: TronAbiEntryType::Function,
                name: "setPair".into(),
                inputs: vec![TronAbiParam {
                    name: "pair".into(),
                    ty: "tuple".into(),
                    ..Default::default()
                }],
                state_mutability: TronAbiStateMutability::NonPayable,
                ..Default::default()
            }],
        };

        assert_eq!(
            tron.try_to_json_abi(),
            Err(TronAbiConversionError::IncompleteTuple("pair".into()))
        );
    }

    #[test]
    fn conflicting_legacy_mutability_is_rejected() {
        let tron = TronAbi {
            entries: vec![TronAbiEntry {
                entry_type: TronAbiEntryType::Function,
                name: "conflicting".into(),
                constant: true,
                payable: true,
                ..Default::default()
            }],
        };

        assert_eq!(tron.try_to_json_abi(), Err(TronAbiConversionError::ConflictingMutability));
    }
}
