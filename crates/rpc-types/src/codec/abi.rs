//! Exact conversions between TRON ABI domain types and generated protobuf types.

use tronz_abi::{TronAbi, TronAbiEntry, TronAbiEntryType, TronAbiParam, TronAbiStateMutability};

use crate::proto;

type ProtoAbi = proto::smart_contract::Abi;
type ProtoAbiEntry = proto::smart_contract::abi::Entry;
type ProtoAbiParam = proto::smart_contract::abi::entry::Param;

pub(super) fn from_proto(abi: ProtoAbi) -> TronAbi {
    TronAbi {
        entries: abi
            .entrys
            .into_iter()
            .map(|entry| TronAbiEntry {
                entry_type: TronAbiEntryType::from_i32(entry.r#type),
                name: entry.name,
                inputs: entry.inputs.into_iter().map(param_from_proto).collect(),
                outputs: entry.outputs.into_iter().map(param_from_proto).collect(),
                anonymous: entry.anonymous,
                constant: entry.constant,
                payable: entry.payable,
                state_mutability: TronAbiStateMutability::from_i32(entry.state_mutability),
            })
            .collect(),
    }
}

pub(super) fn to_proto(abi: TronAbi) -> ProtoAbi {
    ProtoAbi {
        entrys: abi
            .entries
            .into_iter()
            .map(|entry| ProtoAbiEntry {
                anonymous: entry.anonymous,
                constant: entry.constant,
                name: entry.name,
                inputs: entry.inputs.into_iter().map(param_to_proto).collect(),
                outputs: entry.outputs.into_iter().map(param_to_proto).collect(),
                r#type: entry.entry_type.as_i32(),
                payable: entry.payable,
                state_mutability: entry.state_mutability.as_i32(),
            })
            .collect(),
    }
}

fn param_from_proto(param: ProtoAbiParam) -> TronAbiParam {
    TronAbiParam { indexed: param.indexed, name: param.name, ty: param.r#type }
}

fn param_to_proto(param: TronAbiParam) -> ProtoAbiParam {
    ProtoAbiParam { indexed: param.indexed, name: param.name, r#type: param.ty }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protobuf_round_trip_preserves_unknown_values_and_bare_tuple() {
        let proto = ProtoAbi {
            entrys: vec![ProtoAbiEntry {
                name: "setPair".into(),
                inputs: vec![ProtoAbiParam {
                    indexed: false,
                    name: "pair".into(),
                    r#type: "tuple".into(),
                }],
                r#type: 99,
                state_mutability: 98,
                constant: true,
                ..Default::default()
            }],
        };

        let domain = from_proto(proto.clone());
        assert_eq!(domain.entries[0].entry_type, TronAbiEntryType::Unknown(99));
        assert_eq!(domain.entries[0].state_mutability, TronAbiStateMutability::Unknown(98));
        assert_eq!(domain.entries[0].inputs[0].ty, "tuple");
        assert_eq!(to_proto(domain), proto);
    }
}
