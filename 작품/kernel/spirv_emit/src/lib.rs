#![no_std]
#![forbid(
    unsafe_code,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::arithmetic_side_effects,
    clippy::disallowed_types,
    clippy::disallowed_methods
)]

extern crate alloc;

use alloc::vec::Vec;
use math_q32::Q32;
use serde::{Deserialize, Serialize};
use verifier_abi::ContractViolation;

pub const SPIRV_MAGIC: u32 = 0x0723_0203;
pub const SPIRV_VERSION_1_6: u32 = 0x0001_0600;
pub const GENERATOR_AGY_FORGE: u32 = 0x4147_5934;
pub const DEFAULT_APERTURE_Q32: Q32 = Q32::ONE;
pub type SpirvContractViolation = ContractViolation;

// Selected SPIR-V 1.6 Core Opcodes
const OP_NOP: u16 = 0;
const OP_CAPABILITY: u16 = 17;
const OP_MEMORY_MODEL: u16 = 14;
const OP_ENTRY_POINT: u16 = 15;
const OP_EXECUTION_MODE: u16 = 16;
const OP_TYPE_VOID: u16 = 19;
const OP_TYPE_INT: u16 = 21;
const OP_TYPE_FUNCTION: u16 = 33;
const OP_CONSTANT: u16 = 43;
const OP_FUNCTION: u16 = 54;
const OP_FUNCTION_END: u16 = 56;
const OP_LABEL: u16 = 248;
const OP_RETURN: u16 = 253;

const CAPABILITY_SHADER: u32 = 1;
const ADDRESSING_LOGICAL: u32 = 0;
const MEMORY_MODEL_GLSL450: u32 = 1;
const EXEC_MODEL_GLCOMPUTE: u32 = 5;
const EXEC_MODE_LOCAL_SIZE: u32 = 17;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpirvError {
    HeaderTooShort,
    InvalidMagicNumber,
    UnsupportedVersion,
    InvalidBoundOrSchema,
    ZeroWordCountInstruction,
    TruncatedInstruction,
    MissingShaderCapability,
    MissingMemoryModel,
    UnbalancedFunctionBlock,
    MissingBlockTerminator,
    IdOutOfBounds,
}

/// Statically verified `SPIR-V 1.6` binary module (`§4.2 ①`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidatedSpirvModule {
    words: Vec<u32>,
}

impl ValidatedSpirvModule {
    #[inline]
    pub fn words(&self) -> &[u32] {
        &self.words
    }
}

#[inline]
const fn encode_inst_header(word_count: u16, opcode: u16) -> u32 {
    ((word_count as u32) << 16) | (opcode as u32)
}

fn push_spv_string(words: &mut Vec<u32>, text: &str) {
    let bytes = text.as_bytes();
    let mut idx: usize = 0;
    loop {
        let mut word: u32 = 0;
        let mut b_pos: u32 = 0;
        let mut hit_nul = false;
        while b_pos < 4 {
            if let Some(&b) = bytes.get(idx) {
                word |= (b as u32).wrapping_shl(b_pos.saturating_mul(8));
                idx = idx.saturating_add(1);
            } else {
                hit_nul = true;
                break;
            }
            b_pos = b_pos.saturating_add(1);
        }
        words.push(word);
        if hit_nul {
            break;
        }
    }
}

/// Builds a pure-Rust `SPIR-V 1.6` compute shader module for offscreen hybrid visibility/tonemapping passes (`§4.2 ①`)
/// and runs static CFG/capability/bound verification before returning.
pub fn emit_hybrid_compute_spirv(
    local_size_x: u32,
    local_size_y: u32,
    local_size_z: u32,
) -> Result<ValidatedSpirvModule, SpirvError> {
    let lx = local_size_x.max(1);
    let ly = local_size_y.max(1);
    let lz = local_size_z.max(1);

    // SSA IDs:
    // %1 = OpTypeVoid
    // %2 = OpTypeFunction %1
    // %3 = OpTypeInt 32 0
    // %4 = OpConstant %3 0
    // %5 = main function
    // %6 = entry label
    let bound: u32 = 7;
    let mut words = Vec::with_capacity(48);

    // 5-word SPIR-V 1.6 Header
    words.push(SPIRV_MAGIC);
    words.push(SPIRV_VERSION_1_6);
    words.push(GENERATOR_AGY_FORGE);
    words.push(bound);
    words.push(0); // schema = 0

    // OpCapability Shader
    words.push(encode_inst_header(2, OP_CAPABILITY));
    words.push(CAPABILITY_SHADER);

    // OpMemoryModel Logical GLSL450
    words.push(encode_inst_header(3, OP_MEMORY_MODEL));
    words.push(ADDRESSING_LOGICAL);
    words.push(MEMORY_MODEL_GLSL450);

    // OpEntryPoint GLCompute %5 "main"
    let mut name_words = Vec::new();
    push_spv_string(&mut name_words, "main");
    let ep_wc = (3_usize.saturating_add(name_words.len())) as u16;
    words.push(encode_inst_header(ep_wc, OP_ENTRY_POINT));
    words.push(EXEC_MODEL_GLCOMPUTE);
    words.push(5);
    words.extend_from_slice(&name_words);

    // OpExecutionMode %5 LocalSize lx ly lz
    words.push(encode_inst_header(6, OP_EXECUTION_MODE));
    words.push(5);
    words.push(EXEC_MODE_LOCAL_SIZE);
    words.push(lx);
    words.push(ly);
    words.push(lz);

    // %1 = OpTypeVoid
    words.push(encode_inst_header(2, OP_TYPE_VOID));
    words.push(1);

    // %2 = OpTypeFunction %1
    words.push(encode_inst_header(3, OP_TYPE_FUNCTION));
    words.push(2);
    words.push(1);

    // %3 = OpTypeInt 32 0
    words.push(encode_inst_header(4, OP_TYPE_INT));
    words.push(3);
    words.push(32);
    words.push(0);

    // %4 = OpConstant %3 0
    words.push(encode_inst_header(4, OP_CONSTANT));
    words.push(3);
    words.push(4);
    words.push(0);

    // %5 = OpFunction %1 None %2
    words.push(encode_inst_header(5, OP_FUNCTION));
    words.push(1);
    words.push(5);
    words.push(0);
    words.push(2);

    // %6 = OpLabel
    words.push(encode_inst_header(2, OP_LABEL));
    words.push(6);

    // OpReturn
    words.push(encode_inst_header(1, OP_RETURN));

    // OpFunctionEnd
    words.push(encode_inst_header(1, OP_FUNCTION_END));

    validate_spirv_words(words)
}

/// Static `SPIR-V 1.6` binary structural, capability, ID-bound, and basic-block terminator verifier.
pub fn validate_spirv_words(words: Vec<u32>) -> Result<ValidatedSpirvModule, SpirvError> {
    if words.len() < 5 {
        return Err(SpirvError::HeaderTooShort);
    }
    if words[0] != SPIRV_MAGIC {
        return Err(SpirvError::InvalidMagicNumber);
    }
    if words[1] != SPIRV_VERSION_1_6 {
        return Err(SpirvError::UnsupportedVersion);
    }
    let bound = words[3];
    if bound == 0 || words[4] != 0 {
        return Err(SpirvError::InvalidBoundOrSchema);
    }

    let mut pc: usize = 5;
    let mut has_shader_cap = false;
    let mut has_mem_model = false;
    let mut in_function = false;
    let mut in_block = false;
    let mut saw_function = false;

    while pc < words.len() {
        let inst_hdr = words[pc];
        let wc = (inst_hdr.wrapping_shr(16)) as usize;
        let opcode = (inst_hdr & 0xFFFF) as u16;
        if wc == 0 {
            return Err(SpirvError::ZeroWordCountInstruction);
        }
        let next_pc = pc.saturating_add(wc);
        let Some(operands) = words.get(pc.saturating_add(1)..next_pc) else {
            return Err(SpirvError::TruncatedInstruction);
        };

        match opcode {
            OP_NOP => {}
            OP_CAPABILITY => {
                if operands.first().copied() == Some(CAPABILITY_SHADER) {
                    has_shader_cap = true;
                }
            }
            OP_MEMORY_MODEL => {
                if operands.len() == 2 {
                    has_mem_model = true;
                }
            }
            OP_TYPE_VOID | OP_LABEL => {
                if let Some(&res_id) = operands.first()
                    && (res_id == 0 || res_id >= bound)
                {
                    return Err(SpirvError::IdOutOfBounds);
                }
                if opcode == OP_LABEL {
                    if !in_function || in_block {
                        return Err(SpirvError::UnbalancedFunctionBlock);
                    }
                    in_block = true;
                }
            }
            OP_FUNCTION => {
                if in_function || operands.len() != 4 {
                    return Err(SpirvError::UnbalancedFunctionBlock);
                }
                if operands[1] == 0 || operands[1] >= bound {
                    return Err(SpirvError::IdOutOfBounds);
                }
                in_function = true;
                saw_function = true;
            }
            OP_RETURN => {
                if !in_function || !in_block {
                    return Err(SpirvError::MissingBlockTerminator);
                }
                in_block = false;
            }
            OP_FUNCTION_END => {
                if !in_function || in_block {
                    return Err(SpirvError::MissingBlockTerminator);
                }
                in_function = false;
            }
            _ => {}
        }
        pc = next_pc;
    }

    if !has_shader_cap {
        return Err(SpirvError::MissingShaderCapability);
    }
    if !has_mem_model {
        return Err(SpirvError::MissingMemoryModel);
    }
    if !saw_function || in_function || in_block {
        return Err(SpirvError::UnbalancedFunctionBlock);
    }

    Ok(ValidatedSpirvModule { words })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_emit_and_validate_spirv_1_6() {
        let spv = emit_hybrid_compute_spirv(8, 8, 1);
        assert!(spv.is_ok());
        if let Ok(m) = spv {
            assert_eq!(m.words()[0], SPIRV_MAGIC);
            assert_eq!(m.words()[1], SPIRV_VERSION_1_6);
        }

        let bad = validate_spirv_words(alloc::vec![0, SPIRV_VERSION_1_6, 0, 2, 0]);
        assert_eq!(bad, Err(SpirvError::InvalidMagicNumber));
    }
}
