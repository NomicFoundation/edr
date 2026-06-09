//! DWARF debug-info parser for solx-built artifacts. Produces the same
//! [`Instruction`] vector as [`crate::source_map::decode_instructions`] does
//! for solc, so the rest of the stack-trace pipeline stays compiler-agnostic.

use std::{collections::HashMap, rc::Rc, sync::Arc};

use addr2line::Context as Addr2lineContext;
use edr_primitives::{bytecode::opcode::OpCode, hex};
use gimli::{EndianRcSlice, Reader, RunTimeEndian};
use object::{Endianness, Object, ObjectSection};

use crate::{
    build_model::{BuildModel, Instruction, JumpType, SourceLocation},
    debug_info::SolxBuildModelExt as _,
};

type DwarfReader = EndianRcSlice<RunTimeEndian>;

/// Failure modes when decoding a solx DWARF blob. Replaces previous
/// `anyhow::Error` returns so callers (and tests) can match on the specific
/// failure, as is the convention for reusable EDR crates.
#[derive(Debug, thiserror::Error)]
pub enum DwarfError {
    /// `evm.*.debugInfo` wasn't valid ASCII-hex.
    #[error("failed to hex-decode solx evm.*.debugInfo: {0}")]
    HexDecode(#[from] hex::FromHexError),

    /// The decoded bytes weren't a recognisable ELF container.
    #[error("solx debugInfo blob is not a valid ELF (expected ELF32-MSB): {0}")]
    ElfParse(#[from] object::Error),

    /// A DWARF section is compressed — solx doesn't emit them today,
    /// so we surface this as an explicit unsupported case rather than
    /// silently producing empty traces.
    #[error("compressed DWARF section {0:?} is not supported by EDR yet")]
    CompressedSection(&'static str),

    /// EDR currently assumes one compilation unit per contract's
    /// debugInfo blob (matching solx's output). Multi-CU blobs would
    /// share file-name / inlined-range tables across CUs, leading to
    /// wrong cross-CU resolution if processed naively.
    #[error(
        "solx debugInfo blob contains {0}+ DWARF compilation units; \
         EDR only supports single-CU blobs (one CU per contract)"
    )]
    MultiCu(u32),

    /// A DIE PC range escapes the bytecode it claims to cover —
    /// typically a sign of a mismatched / corrupt debugInfo blob.
    #[error("DWARF inlined range {bound} {value:#x} exceeds bytecode length {bytecode_len:#x}")]
    RangeEscapesBytecode {
        bound: &'static str,
        value: u64,
        bytecode_len: u64,
    },

    /// Catch-all for the gimli/addr2line traversal layer. The inner
    /// `gimli::Error` carries its own descriptive message.
    #[error("error walking DWARF structures: {0}")]
    DwarfParse(#[from] gimli::Error),
}

fn attr_file_index<R: Reader>(attr: &gimli::Attribute<R>) -> Option<u64> {
    match attr.value() {
        gimli::AttributeValue::FileIndex(u) | gimli::AttributeValue::Udata(u) => Some(u),
        _ => None,
    }
}

fn attr_udata<R: Reader>(attr: &gimli::Attribute<R>) -> Option<u64> {
    if let gimli::AttributeValue::Udata(u) = attr.value() {
        Some(u)
    } else {
        None
    }
}

fn attr_flag_true<R: Reader>(attr: &gimli::Attribute<R>) -> bool {
    matches!(
        attr.value(),
        gimli::AttributeValue::Flag(true) | gimli::AttributeValue::Udata(1)
    )
}

/// True for `SWAP1..SWAP16` and `DUP1..DUP16`. These reorder the EVM stack,
/// so the destination of a JUMP that follows one cannot be inferred from the
/// most recent PUSH alone.
fn is_stack_shuffle(opcode: OpCode) -> bool {
    matches!(
        opcode,
        OpCode::SWAP1
            | OpCode::SWAP2
            | OpCode::SWAP3
            | OpCode::SWAP4
            | OpCode::SWAP5
            | OpCode::SWAP6
            | OpCode::SWAP7
            | OpCode::SWAP8
            | OpCode::SWAP9
            | OpCode::SWAP10
            | OpCode::SWAP11
            | OpCode::SWAP12
            | OpCode::SWAP13
            | OpCode::SWAP14
            | OpCode::SWAP15
            | OpCode::SWAP16
            | OpCode::DUP1
            | OpCode::DUP2
            | OpCode::DUP3
            | OpCode::DUP4
            | OpCode::DUP5
            | OpCode::DUP6
            | OpCode::DUP7
            | OpCode::DUP8
            | OpCode::DUP9
            | OpCode::DUP10
            | OpCode::DUP11
            | OpCode::DUP12
            | OpCode::DUP13
            | OpCode::DUP14
            | OpCode::DUP15
            | OpCode::DUP16
    )
}

/// Decode a solx-emitted DWARF blob into the same
/// [`Instruction`] vector that [`crate::source_map::decode_instructions`]
/// produces for solc artifacts.
pub fn decode_instructions(
    bytecode: &[u8],
    debug_info_hex: &str,
    build_model: &Arc<BuildModel>,
    _is_deployment: bool,
) -> Result<Vec<Instruction>, DwarfError> {
    // 1. Hex → ELF → gimli::Dwarf.
    let raw = hex::decode(debug_info_hex)?;
    let parsed = ParsedDwarf::from_elf_bytes(&raw)?;

    // 2. Reject debugInfo whose PC ranges escape the bytecode (mismatched or
    //    corrupt blob).
    let bytecode_len = bytecode.len() as u64;
    for range in &parsed.inlined_ranges {
        if range.low_pc >= bytecode_len {
            return Err(DwarfError::RangeEscapesBytecode {
                bound: "low_pc",
                value: range.low_pc,
                bytecode_len,
            });
        }
        if range.high_pc > bytecode_len {
            return Err(DwarfError::RangeEscapesBytecode {
                bound: "high_pc",
                value: range.high_pc,
                bytecode_len,
            });
        }
    }

    // 3. Reuse BuildModel's lazy reverse index (built at most once per BuildModel).
    let name_to_file_id = build_model.name_to_file_id();

    // 4. Per-file line-start caches, populated on demand.
    let mut line_starts_by_file_id: HashMap<u32, Vec<usize>> = HashMap::new();

    // 5. Walk PCs, mirroring `source_map::decode_instructions` so PUSH operands are
    //    skipped consistently. PcOpcodes ends iteration as soon as it hits an
    //    invalid byte (CBOR metadata region), matching the previous `break`
    //    semantics.
    let mut instructions: Vec<Instruction> = PcOpcodes::new(bytecode)
        .map(|step| {
            let location = parsed.user_visible_location_for_pc(
                step.pc as u64,
                &parsed.file_names,
                name_to_file_id,
                build_model,
                &mut line_starts_by_file_id,
            );
            let inline_call_sites = parsed.inline_call_sites_for_pc(
                step.pc as u64,
                &parsed.file_names,
                name_to_file_id,
                build_model,
                &mut line_starts_by_file_id,
            );
            Instruction {
                pc: step.pc as u32,
                opcode: step.opcode,
                // Filled in by assign_jump_types once we have the full stream.
                jump_type: JumpType::NotJump,
                push_data: step.push_data,
                location,
                inline_call_sites,
            }
        })
        .collect();

    parsed.assign_jump_types(&mut instructions);

    Ok(instructions)
}

/// Iterator over `(pc, opcode, push_data)` decoded from raw bytecode,
/// skipping each PUSH's immediate operands so the next yielded PC is the
/// next instruction (not an operand byte). Terminates on the first byte
/// that isn't a valid opcode — solx's trailing CBOR metadata is delimited
/// by such a byte, so this also serves as a natural end-of-code sentinel.
struct PcOpcodes<'a> {
    bytecode: &'a [u8],
    pc: usize,
}

impl<'a> PcOpcodes<'a> {
    fn new(bytecode: &'a [u8]) -> Self {
        Self { bytecode, pc: 0 }
    }
}

struct PcStep {
    pc: usize,
    opcode: OpCode,
    push_data: Option<Vec<u8>>,
}

impl Iterator for PcOpcodes<'_> {
    type Item = PcStep;

    fn next(&mut self) -> Option<Self::Item> {
        let pc = self.pc;
        let raw_op = *self.bytecode.get(pc)?;
        let opcode = OpCode::new(raw_op).or_else(|| {
            log::debug!("DWARF decoder: invalid opcode {raw_op} at pc={pc}, stopping");
            None
        })?;
        let push_size = opcode.info().immediate_size() as usize;
        let push_data = if opcode.is_push() {
            self.bytecode
                .get(pc..)
                .and_then(|s| s.get(..1 + push_size))
                .map(<[u8]>::to_vec)
        } else {
            None
        };
        self.pc = pc + 1 + push_size;
        Some(PcStep {
            pc,
            opcode,
            push_data,
        })
    }
}

/// Resolved row from the DWARF `.debug_line` program.
#[derive(Clone, Debug)]
struct LineRow {
    /// Combined `<dir>/<name>` from the DWARF file table.
    file: String,
    /// `0` is DWARF's "no line" sentinel.
    line: u64,
    column: u64,
}

/// PC range of a `DW_TAG_inlined_subroutine` (or concrete subprogram),
/// joined with its abstract-origin metadata.
#[derive(Clone, Copy, Debug)]
struct InlinedRange {
    low_pc: u64,
    high_pc: u64,
    call_file: Option<u64>,
    call_line: Option<u64>,
    call_column: Option<u64>,
    /// DFS depth — deeper = more innermost. Width can't be used because a
    /// parent's `DW_AT_ranges` can be wider than a child's contiguous range.
    depth: u32,
    /// `DW_AT_artificial=1`. Set on Yul helpers (`panic_error_*`,
    /// `abi_encode_*`, …); we skip these in stack traces and bottom-frame
    /// resolution because their PCs map to where the helper was declared,
    /// not to the user code that triggered it.
    is_artificial: bool,
    /// Abstract origin's `DW_AT_decl_file` / `decl_line` — the inlined
    /// function's own decl, NOT its caller. The anchor for "is this line
    /// inside the user fn's body?"; `call_*` above is the caller's
    /// position and would mis-anchor under user-into-user inlining.
    decl_file: Option<u64>,
    decl_line: Option<u64>,
}

/// First-pass metadata for an abstract subprogram, keyed by DIE offset.
#[derive(Clone, Copy, Default)]
struct AbstractOriginMeta {
    is_artificial: bool,
    decl_file: Option<u64>,
    decl_line: Option<u64>,
}

/// Per-CU view of a single solx DWARF blob.
struct ParsedDwarf {
    /// addr2line indexes the line program for `find_location(pc)`.
    context: Addr2lineContext<DwarfReader>,
    /// Mirrored file table for resolving `call_file` / `decl_file` indices.
    file_names: Vec<String>,
    /// Sorted by `low_pc`; each entry covers `[low_pc, high_pc)`.
    inlined_ranges: Vec<InlinedRange>,
}

impl ParsedDwarf {
    fn from_elf_bytes(raw: &[u8]) -> Result<Self, DwarfError> {
        let file = object::File::parse(raw)?;
        let endian = match file.endianness() {
            Endianness::Big => RunTimeEndian::Big,
            Endianness::Little => RunTimeEndian::Little,
        };

        let load_section = |id: gimli::SectionId| -> Result<DwarfReader, DwarfError> {
            let Some(section) = file.section_by_name(id.name()) else {
                return Ok(EndianRcSlice::new(Rc::from(Vec::new()), endian));
            };
            // Bail on compressed sections so a future solx that emits them
            // fails loudly instead of producing empty traces.
            let compressed = section
                .compressed_file_range()
                .map(|r| r.format != object::CompressionFormat::None)
                .unwrap_or(false);
            if compressed {
                return Err(DwarfError::CompressedSection(id.name()));
            }
            let data: Vec<u8> = section.data().ok().map(<[u8]>::to_vec).unwrap_or_default();
            Ok(EndianRcSlice::new(Rc::from(data), endian))
        };
        let dwarf = gimli::Dwarf::load(load_section)?;

        let mut file_names: Vec<String> = Vec::new();
        let mut inlined_ranges: Vec<InlinedRange> = Vec::new();

        // Bail on multi-CU blobs: file_names + inlined_ranges are shared
        // across CUs, so CU B's file index 0 would resolve via CU A's table.
        let mut cu_count = 0u32;
        let mut units = dwarf.units();
        while let Some(header) = units.next()? {
            cu_count += 1;
            if cu_count > 1 {
                return Err(DwarfError::MultiCu(cu_count));
            }
            let unit = dwarf.unit(header)?;
            let unit_ref = unit.unit_ref(&dwarf);

            if let Some(program) = unit.line_program.clone() {
                let header = program.header();
                if file_names.is_empty() {
                    // Combine `<dir>/<name>` so the key matches BuildModel's
                    // source-name keys; DWARF v5 splits them via
                    // `directory_index()` into `include_directories`.
                    //
                    // `.ok()` here intentionally swallows per-entry failures:
                    // a single malformed dir / file table entry should
                    // degrade locally (empty name → no source location for
                    // affected PCs) rather than sink the whole contract's
                    // tracing. Downstream `match_dwarf_to_build_model`
                    // returns `None` for empty names.
                    let directories: Vec<String> = header
                        .include_directories()
                        .iter()
                        .map(|d| {
                            unit_ref
                                .attr_string(d.clone())
                                .ok()
                                .and_then(|s| {
                                    s.to_string_lossy().ok().map(std::borrow::Cow::into_owned)
                                })
                                .unwrap_or_default()
                        })
                        .collect();
                    file_names = header
                        .file_names()
                        .iter()
                        .map(|fe| {
                            let name = unit_ref
                                .attr_string(fe.path_name())
                                .ok()
                                .and_then(|s| {
                                    s.to_string_lossy().ok().map(std::borrow::Cow::into_owned)
                                })
                                .unwrap_or_default();
                            let dir = directories
                                .get(fe.directory_index() as usize)
                                .map_or("", String::as_str);
                            if dir.is_empty() || name.is_empty() {
                                name
                            } else {
                                let mut s = String::with_capacity(dir.len() + 1 + name.len());
                                s.push_str(dir.trim_end_matches('/'));
                                s.push('/');
                                s.push_str(&name);
                                s
                            }
                        })
                        .collect();
                }
            }

            // Two passes over DIEs: pass 1 collects abstract-subprogram meta
            // (`DW_AT_artificial`, decl_file/line) keyed by DIE offset; pass 2
            // walks inlined-subroutine + concrete subprogram DIEs and joins on it.
            let mut abstract_meta: HashMap<gimli::UnitOffset, AbstractOriginMeta> = HashMap::new();
            {
                let mut entries = unit.entries();
                while let Some((_, die)) = entries.next_dfs()? {
                    if die.tag() != gimli::DW_TAG_subprogram {
                        continue;
                    }
                    let mut is_inline = false;
                    let mut meta = AbstractOriginMeta::default();
                    let mut attrs = die.attrs();
                    while let Some(attr) = attrs.next()? {
                        match attr.name() {
                            gimli::DW_AT_inline => {
                                if let gimli::AttributeValue::Inline(v) = attr.value() {
                                    // DW_INL_inlined or DW_INL_declared_inlined.
                                    is_inline = v.0 == 1 || v.0 == 3;
                                }
                            }
                            gimli::DW_AT_artificial => meta.is_artificial |= attr_flag_true(&attr),
                            gimli::DW_AT_decl_file => meta.decl_file = attr_file_index(&attr),
                            gimli::DW_AT_decl_line => meta.decl_line = attr_udata(&attr),
                            _ => {}
                        }
                    }
                    if is_inline {
                        abstract_meta.insert(die.offset(), meta);
                    }
                }
            }

            // Pass 2: walk inlined-subroutines and concrete subprograms.
            // `die_ranges` handles both the contiguous (`low_pc/high_pc`)
            // and the `DW_AT_ranges` list form.
            let mut entries = unit.entries();
            let mut depth: isize = 0;
            while let Some((delta, die)) = entries.next_dfs()? {
                depth += delta;

                // Two subprogram-like DIEs feed this: DW_TAG_inlined_subroutine
                // (with abstract origin) and concrete DW_TAG_subprogram (low_pc,
                // no DW_AT_inline) — typically self-recursive functions.
                let is_inlined = die.tag() == gimli::DW_TAG_inlined_subroutine;
                let is_subprogram_with_pc = die.tag() == gimli::DW_TAG_subprogram
                    && die.attr(gimli::DW_AT_low_pc)?.is_some()
                    && die.attr(gimli::DW_AT_inline)?.is_none();
                if !is_inlined && !is_subprogram_with_pc {
                    continue;
                }
                let mut call_file: Option<u64> = None;
                let mut call_line: Option<u64> = None;
                let mut call_column: Option<u64> = None;
                let mut abstract_origin: Option<gimli::UnitOffset> = None;
                // Concrete subprograms carry decl_* on the DIE itself (no
                // abstract origin to drill into).
                let mut self_decl_file: Option<u64> = None;
                let mut self_decl_line: Option<u64> = None;
                let mut self_artificial = false;
                let mut attrs = die.attrs();
                while let Some(attr) = attrs.next()? {
                    match attr.name() {
                        gimli::DW_AT_call_file => call_file = attr_file_index(&attr),
                        gimli::DW_AT_call_line => call_line = attr_udata(&attr),
                        gimli::DW_AT_call_column => call_column = attr_udata(&attr),
                        gimli::DW_AT_abstract_origin => {
                            if let gimli::AttributeValue::UnitRef(off) = attr.value() {
                                abstract_origin = Some(off);
                            }
                        }
                        gimli::DW_AT_decl_file => self_decl_file = attr_file_index(&attr),
                        gimli::DW_AT_decl_line => self_decl_line = attr_udata(&attr),
                        gimli::DW_AT_artificial => self_artificial |= attr_flag_true(&attr),
                        _ => {}
                    }
                }
                // Skip artificial concrete subprograms (solx's __entry
                // dispatcher covers the whole bytecode — would classify
                // every JUMP as IntoFunction).
                if is_subprogram_with_pc && self_artificial {
                    continue;
                }
                let abstract_meta = if is_inlined {
                    abstract_origin
                        .and_then(|off| abstract_meta.get(&off).copied())
                        .unwrap_or_default()
                } else {
                    AbstractOriginMeta {
                        is_artificial: self_artificial,
                        decl_file: self_decl_file,
                        decl_line: self_decl_line,
                    }
                };
                let depth_u32 = u32::try_from(depth.max(0)).unwrap_or(u32::MAX);
                let mut ranges = unit_ref.die_ranges(die)?;
                while let Some(range) = ranges.next()? {
                    if range.end > range.begin {
                        inlined_ranges.push(InlinedRange {
                            low_pc: range.begin,
                            high_pc: range.end,
                            call_file,
                            call_line,
                            call_column,
                            depth: depth_u32,
                            is_artificial: abstract_meta.is_artificial,
                            decl_file: abstract_meta.decl_file,
                            decl_line: abstract_meta.decl_line,
                        });
                    }
                }
            }
        }

        inlined_ranges.sort_by_key(|r| r.low_pc);

        // addr2line indexes the line program for find_location(pc).
        let context = Addr2lineContext::from_dwarf(dwarf)?;

        Ok(Self {
            context,
            file_names,
            inlined_ranges,
        })
    }

    /// Line-program location covering `pc`.
    fn location_for_pc(&self, pc: u64) -> Option<LineRow> {
        let loc = self.context.find_location(pc).ok().flatten()?;
        Some(LineRow {
            file: loc.file?.to_string(),
            line: loc.line.map_or(0, u64::from),
            column: loc.column.map_or(0, u64::from),
        })
    }

    /// Inlined-subroutine ranges containing `pc`, sorted innermost-first by
    /// DIE depth. (Width is unreliable: a parent's `DW_AT_ranges` union can
    /// be wider than a child's contiguous range.)
    fn containing_ranges(&self, pc: u64) -> Vec<InlinedRange> {
        let mut out: Vec<InlinedRange> = self
            .inlined_ranges
            .iter()
            .filter(|r| r.low_pc <= pc && pc < r.high_pc)
            .copied()
            .collect();
        out.sort_by(|a, b| b.depth.cmp(&a.depth));
        out
    }

    /// Resolve an [`InlinedRange`]'s `call_*` attributes to a `SourceLocation`,
    /// or `None` if any required field is missing.
    fn range_call_site_to_location(
        r: &InlinedRange,
        dwarf_file_names: &[String],
        name_to_file_id: &HashMap<String, u32>,
        build_model: &Arc<BuildModel>,
        line_starts_cache: &mut HashMap<u32, Vec<usize>>,
    ) -> Option<Arc<SourceLocation>> {
        let (file_idx, line) = (r.call_file?, r.call_line?);
        let column = r.call_column.unwrap_or(0);
        let (file_id, offset) = resolve_location(
            file_idx,
            line,
            column,
            dwarf_file_names,
            name_to_file_id,
            build_model,
            line_starts_cache,
        )?;
        let length = build_model
            .smallest_enclosing_span(file_id, offset as u32)
            .map_or(0, |(_, len)| len);
        Some(source_location_at(
            build_model,
            file_id,
            offset as u32,
            length,
        ))
    }

    /// Best-effort bottom-frame source location for `pc`, in order:
    /// 1. innermost artificial helper's `call_site` (if inside the user fn);
    /// 2. line-program row at `pc`;
    /// 3. user function's own AST source location.
    fn user_visible_location_for_pc(
        &self,
        pc: u64,
        dwarf_file_names: &[String],
        name_to_file_id: &HashMap<String, u32>,
        build_model: &Arc<BuildModel>,
        line_starts_cache: &mut HashMap<u32, Vec<usize>>,
    ) -> Option<Arc<SourceLocation>> {
        let containing = self.containing_ranges(pc);

        // Innermost non-artificial range = the user fn body executing at this
        // PC. Innermost (not outermost) because under user-into-user inlining
        // (e.g. `set` inlining `_check`), a PC in `_check` should resolve to
        // `_check`, not `set`.
        let innermost_user_range = containing.iter().find(|r| !r.is_artificial).copied();

        // Trusted anchor: AST function whose decl_line matches the inlined
        // subroutine's abstract origin.
        let abstract_origin_func: Option<Arc<crate::build_model::ContractFunction>> =
            innermost_user_range.and_then(|r| {
                let (file_idx, decl_line) = (r.decl_file?, r.decl_line?);
                let dwarf_name = dwarf_file_names.get(file_idx as usize)?.as_str();
                let file_id = match_dwarf_to_build_model(dwarf_name, name_to_file_id)?;
                let file = build_model.file_id_to_source_file.get(&file_id)?;
                let file = file.read();
                file.get_function_by_decl_line(u32::try_from(decl_line).ok()?)
                    .cloned()
            });

        // Modifiers fold into the enclosing function. Accept the line-program
        // row only when it's in the same contract as the abstract-origin
        // function (filters dispatcher PCs mapped to unrelated files).
        let _ = dwarf_file_names; // unused on this path
        let line_program_func: Option<Arc<crate::build_model::ContractFunction>> = self
            .location_for_pc(pc)
            .filter(|row| row.line != 0)
            .and_then(|row| {
                let (file_id, offset) = resolve_location_by_name(
                    &row.file,
                    row.line,
                    row.column,
                    name_to_file_id,
                    build_model,
                    line_starts_cache,
                )?;
                let probe = SourceLocation::new(
                    Arc::clone(&build_model.file_id_to_source_file),
                    file_id,
                    offset as u32,
                    0,
                );
                probe.get_containing_function().ok().flatten()
            })
            .filter(|lp_func| match abstract_origin_func.as_ref() {
                None => true,
                Some(ao_func) => match (
                    lp_func.contract_name.as_deref(),
                    ao_func.contract_name.as_deref(),
                ) {
                    (Some(a), Some(b)) => a == b,
                    _ => true,
                },
            });

        let user_func: Option<(u32, u32, u32)> =
            line_program_func.or(abstract_origin_func).map(|f| {
                let file_id = f.location.file_id;
                (file_id, f.location.offset, f.location.length)
            });
        let user_func_range: Option<(u32, u32)> = user_func.map(|(_, off, len)| (off, off + len));

        let inside_user_func = |offset: u32| -> bool {
            user_func_range.is_none_or(|(lo, hi)| offset >= lo && offset < hi)
        };

        // Pass 1: innermost artificial entry whose call_site is inside the user fn.
        for r in &containing {
            if !r.is_artificial {
                continue;
            }
            let Some(loc) = Self::range_call_site_to_location(
                r,
                dwarf_file_names,
                name_to_file_id,
                build_model,
                line_starts_cache,
            ) else {
                continue;
            };
            if inside_user_func(loc.offset) {
                return Some(loc);
            }
        }

        // Pass 2: line-program row, only when it lands inside the user fn.
        if let Some(row) = self.location_for_pc(pc)
            && row.line != 0
            && let Some((file_id, offset)) = resolve_location_by_name(
                &row.file,
                row.line,
                row.column,
                name_to_file_id,
                build_model,
                line_starts_cache,
            )
            && inside_user_func(offset as u32)
        {
            let length = build_model
                .smallest_enclosing_span(file_id, offset as u32)
                .map_or(0, |(_, len)| len);
            return Some(source_location_at(
                build_model,
                file_id,
                offset as u32,
                length,
            ));
        }

        // Pass 3: fall back to the user fn's own AST location — coarser, but
        // at least `get_containing_function` will name the right function.
        user_func.map(|(file_id, offset, length)| {
            source_location_at(build_model, file_id, offset, length)
        })
    }

    /// Inlined call-site chain for `pc`, innermost-first. Non-artificial only;
    /// artificial entries (Yul helpers) fold into the bottom-frame location.
    /// Caller dedups consecutive same-function entries.
    fn inline_call_sites_for_pc(
        &self,
        pc: u64,
        dwarf_file_names: &[String],
        name_to_file_id: &HashMap<String, u32>,
        build_model: &Arc<BuildModel>,
        line_starts_cache: &mut HashMap<u32, Vec<usize>>,
    ) -> Box<[Arc<SourceLocation>]> {
        let containing = self.containing_ranges(pc);
        let mut out: Vec<Arc<SourceLocation>> = Vec::with_capacity(containing.len());
        for r in containing {
            if r.is_artificial {
                continue;
            }
            if let Some(loc) = Self::range_call_site_to_location(
                &r,
                dwarf_file_names,
                name_to_file_id,
                build_model,
                line_starts_cache,
            ) {
                out.push(loc);
            }
        }
        out.into_boxed_slice()
    }

    /// Smallest inlined-subroutine PC range that contains `pc`. Returns the
    /// range's `(low_pc, high_pc)` if any.
    fn smallest_containing_range(&self, pc: u64) -> Option<InlinedRange> {
        let mut best: Option<InlinedRange> = None;
        for r in &self.inlined_ranges {
            if r.low_pc > pc {
                break;
            }
            if pc < r.high_pc {
                let candidate = *r;
                let span = candidate.high_pc - candidate.low_pc;
                let best_span = best.map_or(u64::MAX, |b| b.high_pc - b.low_pc);
                if span <= best_span {
                    best = Some(candidate);
                }
            }
        }
        best
    }

    /// Classify JUMP/JUMPI by comparing the source PC's and destination's
    /// containing inlined-subroutine ranges; destination is the preceding PUSH.
    fn assign_jump_types(&self, instructions: &mut [Instruction]) {
        for i in 0..instructions.len() {
            let Some(inst) = instructions.get(i) else {
                continue;
            };
            if !matches!(inst.opcode, OpCode::JUMP | OpCode::JUMPI) {
                continue;
            }
            // Recent PUSH = JUMP destination (low 8 bytes; JUMPDESTs fit in u64).
            // SWAP/DUP between PUSH and JUMP → bail to InternalJump.
            let mut dest: Option<u64> = None;
            for j in (0..i).rev().take(8) {
                let Some(prev) = instructions.get(j) else {
                    break;
                };
                if is_stack_shuffle(prev.opcode) {
                    log::debug!(
                        "DWARF jump classifier: JUMP at pc={:#x} preceded by {:?}; \
                         falling back to InternalJump",
                        inst.pc,
                        prev.opcode,
                    );
                    break;
                }
                if let Some(data) = prev.push_data.as_deref()
                    && let Some(operand) = data.get(1..)
                {
                    let take_n = operand.len().min(8);
                    let tail = operand.get(operand.len() - take_n..).unwrap_or(operand);
                    let mut val: u64 = 0;
                    for &b in tail {
                        val = (val << 8) | u64::from(b);
                    }
                    dest = Some(val);
                    break;
                }
            }
            let here = self.smallest_containing_range(u64::from(inst.pc));
            let there = dest.and_then(|d| self.smallest_containing_range(d));
            let new_jump_type = match (here, there) {
                (None, Some(_)) => JumpType::IntoFunction,
                (Some(h), Some(t)) => {
                    if (h.low_pc, h.high_pc) == (t.low_pc, t.high_pc) {
                        // Jumping to a function's own entry PC is recursion.
                        if dest == Some(t.low_pc) {
                            JumpType::IntoFunction
                        } else {
                            JumpType::InternalJump
                        }
                    } else if t.low_pc < h.low_pc || t.high_pc > h.high_pc {
                        // `t` isn't nested in `h`: either parent-of-`h` (return)
                        // or sibling (cross-call between concrete subprograms).
                        // Jumping to `t.low_pc` is a call; anything else, a return.
                        if dest == Some(t.low_pc) {
                            JumpType::IntoFunction
                        } else {
                            JumpType::OutofFunction
                        }
                    } else {
                        JumpType::IntoFunction
                    }
                }
                (Some(_), None) => JumpType::OutofFunction,
                _ => JumpType::InternalJump,
            };
            if let Some(target) = instructions.get_mut(i) {
                target.jump_type = new_jump_type;
            }
        }
    }
}

/// Resolve a DWARF file index plus `(line, column)` to an EDR
/// `(file_id, byte_offset)` pair.
fn resolve_location(
    dwarf_file_index: u64,
    line: u64,
    column: u64,
    dwarf_file_names: &[String],
    name_to_file_id: &HashMap<String, u32>,
    build_model: &Arc<BuildModel>,
    line_starts_cache: &mut HashMap<u32, Vec<usize>>,
) -> Option<(u32, usize)> {
    let dwarf_name = dwarf_file_names.get(dwarf_file_index as usize)?.as_str();
    resolve_location_by_name(
        dwarf_name,
        line,
        column,
        name_to_file_id,
        build_model,
        line_starts_cache,
    )
}

/// Resolve a source path + `(line, column)` to `(file_id, byte_offset)`.
/// Used when the file path is already resolved (e.g. via `addr2line`).
fn resolve_location_by_name(
    dwarf_name: &str,
    line: u64,
    column: u64,
    name_to_file_id: &HashMap<String, u32>,
    build_model: &Arc<BuildModel>,
    line_starts_cache: &mut HashMap<u32, Vec<usize>>,
) -> Option<(u32, usize)> {
    let file_id = match_dwarf_to_build_model(dwarf_name, name_to_file_id)?;
    let starts = line_starts_cache
        .entry(file_id)
        .or_insert_with(|| compute_line_starts(build_model, file_id));
    let line_idx = line.saturating_sub(1) as usize;
    let line_start = starts.get(line_idx).copied()?;
    let column_offset = if column == 0 { 0 } else { column as usize - 1 };
    Some((file_id, line_start + column_offset))
}

/// Construct a `SourceLocation` against the `BuildModel`'s source map.
fn source_location_at(
    build_model: &Arc<BuildModel>,
    file_id: u32,
    offset: u32,
    length: u32,
) -> Arc<SourceLocation> {
    Arc::new(SourceLocation::new(
        Arc::clone(&build_model.file_id_to_source_file),
        file_id,
        offset,
        length,
    ))
}

/// Match a DWARF source path against `BuildModel` keys: exact → `/suffix` →
/// basename. Each step picks the longest match; ties resolve to `None`.
fn match_dwarf_to_build_model(
    dwarf_name: &str,
    name_to_file_id: &HashMap<String, u32>,
) -> Option<u32> {
    if let Some(id) = name_to_file_id.get(dwarf_name).copied() {
        return Some(id);
    }
    let suffix = format!("/{dwarf_name}");
    let suffix_candidates = find_candidates(name_to_file_id, |name| name.ends_with(&suffix));
    if let Some(id) = pick_unambiguous(&suffix_candidates, dwarf_name) {
        return Some(id);
    }

    let dwarf_basename = dwarf_name.rsplit('/').next().unwrap_or(dwarf_name);
    let basename_candidates = find_candidates(name_to_file_id, |name| {
        name.rsplit('/').next().unwrap_or(name) == dwarf_basename
    });
    pick_unambiguous(&basename_candidates, dwarf_name)
}

fn find_candidates(
    name_to_file_id: &HashMap<String, u32>,
    predicate: impl Fn(&str) -> bool,
) -> Vec<(&String, u32)> {
    name_to_file_id
        .iter()
        .filter(|(name, _)| predicate(name))
        .map(|(name, id)| (name, *id))
        .collect()
}

/// Longest candidate wins; length ties return None (wrong-file resolution
/// is harder to debug than a missing source reference).
fn pick_unambiguous(candidates: &[(&String, u32)], dwarf_name: &str) -> Option<u32> {
    if candidates.is_empty() {
        return None;
    }
    let max_len = candidates.iter().map(|(name, _)| name.len()).max()?;
    let longest: Vec<_> = candidates
        .iter()
        .filter(|(name, _)| name.len() == max_len)
        .collect();
    if longest.len() > 1 {
        let names: Vec<&str> = longest.iter().map(|(n, _)| n.as_str()).collect();
        log::warn!(
            "DWARF source path {dwarf_name:?} matches multiple BuildModel keys with \
             equal specificity: {names:?}; refusing to pick one"
        );
        return None;
    }
    longest.first().map(|(_, id)| *id)
}

fn compute_line_starts(build_model: &Arc<BuildModel>, file_id: u32) -> Vec<usize> {
    let Some(file) = build_model.file_id_to_source_file.get(&file_id) else {
        return Vec::new();
    };
    let content = file.read().content.clone();
    let mut starts = vec![0usize];
    for (i, b) in content.bytes().enumerate() {
        if b == b'\n' {
            starts.push(i + 1);
        }
    }
    starts
}

#[cfg(test)]
mod tests {
    use std::{rc::Rc, sync::Arc};

    use parking_lot::RwLock;

    use super::*;
    use crate::{
        artifacts::{CompilerOutput, SolxBytecode},
        build_model::SourceFile,
        debug_info::CompilerArtifact,
    };

    /// `ParsedDwarf` with only the given ranges and an empty Context — for
    /// range-logic tests that don't need a real line program.
    fn make_parsed_dwarf_with_ranges(inlined_ranges: Vec<InlinedRange>) -> ParsedDwarf {
        let load = |_: gimli::SectionId| -> Result<DwarfReader, gimli::Error> {
            Ok(EndianRcSlice::new(Rc::from(Vec::new()), RunTimeEndian::Big))
        };
        let dwarf = gimli::Dwarf::load(load).expect("empty DWARF should load");
        let context = Addr2lineContext::from_dwarf(dwarf).expect("empty Context should build");
        ParsedDwarf {
            context,
            file_names: Vec::new(),
            inlined_ranges,
        }
    }

    /// Minimal `BuildModel` (Counter.sol only) for `(file, line, column)`
    /// resolution tests.
    fn make_build_model_for_counter() -> Arc<BuildModel> {
        let counter_src = include_str!("../../fixtures/sources/Counter.sol");
        let file = SourceFile::new("Counter.sol".to_string(), counter_src.to_string());
        let mut map = std::collections::HashMap::new();
        map.insert(0u32, Arc::new(RwLock::new(file)));
        Arc::new(BuildModel {
            file_id_to_source_file: Arc::new(map),
            ..BuildModel::default()
        })
    }

    fn load_solx_output() -> CompilerOutput<SolxBytecode> {
        let s = include_str!("../../fixtures/solx_compiler_output.json");
        serde_json::from_str(s).unwrap()
    }

    fn load_scenarios_output() -> CompilerOutput<SolxBytecode> {
        let s = include_str!("../../fixtures/solx_compiler_output_scenarios.json");
        serde_json::from_str(s).unwrap()
    }

    /// `BuildModel` for `Scenarios.t.sol`, built via the same AST walk
    /// production uses — so regenerating the fixture JSON is the only
    /// step needed when scenarios are added or reordered.
    fn make_build_model_for_scenarios() -> Arc<BuildModel> {
        let mut input: crate::artifacts::CompilerInput = serde_json::from_str(include_str!(
            "../../fixtures/solx_compiler_input_scenarios.json"
        ))
        .expect("solx_compiler_input_scenarios.json must parse");
        input
            .sources
            .get_mut("project/contracts/Scenarios.t.sol")
            .unwrap()
            .content = include_str!("../../fixtures/sources/Scenarios.t.sol").to_string();
        let output = load_scenarios_output();

        let output =
            output.map_artifact(|artifact| -> Box<dyn CompilerArtifact> { Box::new(artifact) });

        let model = crate::compiler::create_sources_model_from_ast(&output, &input)
            .expect("AST processor must accept the scenarios fixture");
        Arc::new(model)
    }

    fn decode_deployed_for(
        output: &CompilerOutput<SolxBytecode>,
        contract: &str,
        model: &Arc<BuildModel>,
    ) -> Vec<Instruction> {
        let bc = &output
            .contracts
            .get("project/contracts/Scenarios.t.sol")
            .unwrap()
            .get(contract)
            .unwrap()
            .evm
            .deployed_bytecode;

        let raw = hex::decode(&bc.object).unwrap();
        decode_instructions(&raw, &bc.debug_info, model, false).unwrap()
    }

    /// First instruction whose resolved location starts at `expected` line.
    fn first_inst_at_line(insts: &[Instruction], expected: u32) -> Option<Instruction> {
        insts.iter().find_map(|i| {
            let line = i.location.as_ref()?.get_starting_line_number().ok()?;
            (line == expected).then(|| i.clone())
        })
    }

    mod bottom_frame_resolution {
        use super::*;

        /// Panic 0x12 (div by zero): pin the bottom location to the divide
        /// expression (line 34), not the panic helper's contract-scope
        /// `call_line`.
        #[test]
        fn divide_by_zero_filters_out_panic_helper_decl_line() {
            let output = load_scenarios_output();
            let model = make_build_model_for_scenarios();
            let instructions = decode_deployed_for(&output, "DivisionByZeroTest", &model);

            // PC 0xd2c is the panic-emitting REVERT for `0x12` (divide by zero).
            let inst = instructions
                .iter()
                .find(|i| i.pc == 0xd2c)
                .expect("PC 0xd2c should be present");
            let line = inst
                .location
                .as_ref()
                .and_then(|loc| loc.get_starting_line_number().ok())
                .expect("PC 0xd2c must have a resolved location");
            assert_eq!(
                line, 34,
                "expected line 34 (the divide expression `c = a / b`), got {line}. \
                 A regression to line 30 means the panic-helper bogus call_line \
                 is being picked again instead of the next-outer artificial entry."
            );
        }

        /// Cross-contract leak guard: PC 0xcb7's line-program row points at a
        /// different contract's setUp; the contract-match check must reject it
        /// and fall back to the abstract origin's `decl_line`.
        #[test]
        fn invalid_opcode_does_not_leak_unrelated_contract_setup_line() {
            let output = load_scenarios_output();
            let model = make_build_model_for_scenarios();
            let instructions = decode_deployed_for(&output, "InvalidOpcodeTest", &model);

            let inst = instructions
                .iter()
                .find(|i| i.pc == 0xcb7)
                .expect("PC 0xcb7 (the INVALID opcode) should be present");
            let line = inst
                .location
                .as_ref()
                .and_then(|loc| loc.get_starting_line_number().ok())
                .expect("PC 0xcb7 must have a resolved location");
            assert_ne!(
                line, 97,
                "regression: line 97 is in ModifierRevertTest.setUp, a *different* \
                 contract — the contract-match check in user_visible_location_for_pc \
                 must reject it."
            );
            assert_eq!(
                line, 182,
                "expected fall-back to `testInvalidOpcode`'s decl_line (182), got {line}."
            );
        }

        /// Assembly reverts: solx emits no `.debug_line` rows for assembly
        /// opcodes, so we fall back to the function decl line. Update
        /// if solx changes.
        #[test]
        fn inline_assembly_revert_falls_back_to_function_decl_line() {
            let output = load_scenarios_output();
            let model = make_build_model_for_scenarios();
            let instructions = decode_deployed_for(&output, "InlineAssemblyRevertTest", &model);

            let inst = instructions
                .iter()
                .find(|i| i.pc == 0x445)
                .expect("PC 0x445 (the assembly REVERT) should be present");
            let line = inst
                .location
                .as_ref()
                .and_then(|loc| loc.get_starting_line_number().ok())
                .expect("PC 0x445 must have a resolved location");
            assert_eq!(
                line, 129,
                "expected fall-back to testInlineAssemblyRevert's decl line (129), got {line}. \
                 Solc would emit line 135 (the literal `revert(...)` statement); update this \
                 assertion when solx emits `.debug_line` rows for assembly opcodes."
            );
        }

        #[test]
        fn deployed_dwarf_maps_some_pc_to_require_line() {
            let output = load_solx_output();
            let bc = &output
                .contracts
                .get("Counter.sol")
                .expect("Counter.sol")
                .get("Counter")
                .expect("Counter")
                .evm
                .deployed_bytecode;
            let raw = hex::decode(&bc.object).expect("hex object");

            let model = make_build_model_for_counter();
            let instructions = decode_instructions(&raw, &bc.debug_info, &model, false)
                .expect("DWARF decode should succeed for the Counter fixture");
            assert!(!instructions.is_empty());

            // pc=0x002e maps to Counter.sol line 13 (the `require(v > 0, ...)`).
            let has_line_13 = instructions
                .iter()
                .any(|inst| match inst.location.as_ref() {
                    Some(loc) => loc.get_starting_line_number().unwrap_or(0) == 13,
                    None => false,
                });
            assert!(
                has_line_13,
                "expected at least one instruction at Counter.sol line 13 (the require)"
            );
        }

        /// `require(false, "boom")` — bottom at the require statement.
        #[test]
        fn pin_direct_require() {
            let output = load_scenarios_output();
            let model = make_build_model_for_scenarios();
            let insts = decode_deployed_for(&output, "DirectRequireTest", &model);
            assert!(first_inst_at_line(&insts, 12).is_some());
        }

        /// `assert(false)` panic 0x01 — bottom at the assert statement.
        #[test]
        fn pin_assertion_failure() {
            let output = load_scenarios_output();
            let model = make_build_model_for_scenarios();
            let insts = decode_deployed_for(&output, "AssertionFailureTest", &model);
            assert!(first_inst_at_line(&insts, 18).is_some());
        }

        /// Arithmetic overflow panic 0x11 — bottom at the `+` expression.
        #[test]
        fn pin_overflow() {
            let output = load_scenarios_output();
            let model = make_build_model_for_scenarios();
            let insts = decode_deployed_for(&output, "OverflowTest", &model);
            assert!(first_inst_at_line(&insts, 26).is_some());
        }

        /// Array OOB panic 0x32 — bottom at the indexing expression.
        #[test]
        fn pin_array_oob() {
            let output = load_scenarios_output();
            let model = make_build_model_for_scenarios();
            let insts = decode_deployed_for(&output, "ArrayOutOfBoundsTest", &model);
            assert!(first_inst_at_line(&insts, 42).is_some());
        }

        /// `revert MyError(...)` — bottom at the revert statement.
        #[test]
        fn pin_custom_error() {
            let output = load_scenarios_output();
            let model = make_build_model_for_scenarios();
            let insts = decode_deployed_for(&output, "CustomErrorTest", &model);
            assert!(first_inst_at_line(&insts, 51).is_some());
        }

        /// Constructor `require(false, ...)` — runs in CREATE bytecode.
        #[test]
        fn pin_constructor_revert() {
            let output = load_scenarios_output();
            let model = make_build_model_for_scenarios();
            let bc = &output
                .contracts
                .get("project/contracts/Scenarios.t.sol")
                .unwrap()
                .get("ConstructorRevertContract")
                .unwrap()
                .evm
                .bytecode;
            let raw = hex::decode(&bc.object).unwrap();
            let insts = decode_instructions(&raw, &bc.debug_info, &model, true).unwrap();
            assert!(first_inst_at_line(&insts, 57).is_some());
        }

        /// Cross-contract recursion — pin the innermost `recurse(0)`. The
        /// multi-level chain is reconstructed by the trace renderer.
        #[test]
        fn pin_deep_recursion_bottom() {
            let output = load_scenarios_output();
            let model = make_build_model_for_scenarios();
            let insts = decode_deployed_for(&output, "DeepRecursionTarget", &model);
            assert!(first_inst_at_line(&insts, 109).is_some());
        }

        /// Library-internal function reverts. solx emits the helper as a
        /// concrete subprogram; the require resolves directly.
        #[test]
        fn pin_library_revert() {
            let output = load_scenarios_output();
            let model = make_build_model_for_scenarios();
            let insts = decode_deployed_for(&output, "LibraryRevertTest", &model);
            assert!(first_inst_at_line(&insts, 229).is_some());
        }

        /// `fallback() { revert(...) }` on the target contract.
        #[test]
        fn pin_fallback_revert() {
            let output = load_scenarios_output();
            let model = make_build_model_for_scenarios();
            let insts = decode_deployed_for(&output, "FallbackRevertTarget", &model);
            assert!(first_inst_at_line(&insts, 259).is_some());
        }

        /// `receive() { revert(...) }` on the target contract.
        #[test]
        fn pin_receive_revert() {
            let output = load_scenarios_output();
            let model = make_build_model_for_scenarios();
            let insts = decode_deployed_for(&output, "ReceiveRevertTarget", &model);
            assert!(first_inst_at_line(&insts, 276).is_some());
        }

        /// Two consecutive `require`s; the second fails.
        #[test]
        fn pin_multiple_requires() {
            let output = load_scenarios_output();
            let model = make_build_model_for_scenarios();
            let insts = decode_deployed_for(&output, "MultipleRequiresTest", &model);
            assert!(first_inst_at_line(&insts, 340).is_some());
        }

        /// Invalid enum cast panic 0x21 — anywhere inside the test body.
        #[test]
        fn pin_invalid_enum_cast() {
            let output = load_scenarios_output();
            let model = make_build_model_for_scenarios();
            let insts = decode_deployed_for(&output, "InvalidEnumCastTest", &model);
            assert!(first_inst_at_line(&insts, 169).is_some());
        }

        /// `arr.pop()` on empty array panics 0x31.
        #[test]
        fn pin_pop_empty_array() {
            let output = load_scenarios_output();
            let model = make_build_model_for_scenarios();
            let insts = decode_deployed_for(&output, "PopEmptyArrayTest", &model);
            assert!(first_inst_at_line(&insts, 177).is_some());
        }

        /// Invariant test that always reverts — pin the require's line.
        #[test]
        fn pin_invariant_failure() {
            let output = load_scenarios_output();
            let model = make_build_model_for_scenarios();
            let insts = decode_deployed_for(&output, "InvariantFailureTest", &model);
            assert!(first_inst_at_line(&insts, 361).is_some());
        }
    }

    mod inline_call_sites {
        use super::*;

        /// solx flattens a modifier into its enclosing function as a single
        /// inlined-subroutine — pin one frame (vs. solc's duplicate
        /// setIfPositive).
        #[test]
        fn modifier_revert_has_single_set_if_positive_inline_call_site() {
            let output = load_scenarios_output();
            let model = make_build_model_for_scenarios();
            let instructions = decode_deployed_for(&output, "ModifierTarget", &model);

            // PC 0x73 is the modifier's `require(v > 0, ...)` revert.
            let inst = instructions
                .iter()
                .find(|i| i.pc == 0x73)
                .expect("PC 0x73 should be present");
            let call_site_lines: Vec<u32> = inst
                .inline_call_sites
                .iter()
                .filter_map(|cs| cs.get_starting_line_number().ok())
                .collect();
            assert_eq!(
                call_site_lines,
                vec![91],
                "expected exactly one non-artificial inline call site at line 91 \
                 (setIfPositive), got {call_site_lines:?}. If this asserts \
                 [91, 91] a duplicate function-entry frame has reappeared."
            );
        }

        #[test]
        fn inline_call_sites_recover_caller_line_for_inlined_helpers() {
            // PCs inside _checkPositive carry the caller line (Counter.sol:8)
            // via inline_call_sites — that's what gives EDR the middle frame.
            let output = load_solx_output();
            let bc = &output
                .contracts
                .get("Counter.sol")
                .unwrap()
                .get("Counter")
                .unwrap()
                .evm
                .deployed_bytecode;
            let raw = hex::decode(&bc.object).unwrap();
            let model = make_build_model_for_counter();
            let instructions = decode_instructions(&raw, &bc.debug_info, &model, false).unwrap();

            // Pin: some PC at Counter.sol:13 (the require) has inline_call_sites
            // pointing at Counter.sol:8 (the _checkPositive call site).
            let any_call_site_at_line_8 = instructions.iter().any(|inst| {
                inst.location
                    .as_ref()
                    .is_some_and(|l| l.get_starting_line_number().unwrap_or(0) == 13)
                    && inst
                        .inline_call_sites
                        .iter()
                        .any(|cs| cs.get_starting_line_number().unwrap_or(0) == 8)
            });
            assert!(
                any_call_site_at_line_8,
                "expected at least one instruction at Counter.sol:13 (the require) \
                 to carry an inline_call_sites entry pointing at Counter.sol:8 \
                 (the call site of _checkPositive inside set)"
            );
        }

        /// Internal helper chain: `set` → `_checkPositive` reverts.
        /// `inline_call_sites` at the require PC carries the caller line.
        #[test]
        fn pin_internal_helper_chain_carries_caller_line() {
            let output = load_scenarios_output();
            let model = make_build_model_for_scenarios();
            let insts = decode_deployed_for(&output, "InternalHelperChainContract", &model);
            let inst = first_inst_at_line(&insts, 149)
                .expect("expected `_checkPositive`'s require at line 149");
            let lines: Vec<u32> = inst
                .inline_call_sites
                .iter()
                .filter_map(|cs| cs.get_starting_line_number().ok())
                .collect();
            assert!(lines.contains(&144), "got {lines:?}");
        }

        /// Constructor → internal `_check` revert (CREATE). `inline_call_sites`
        /// at the require PC carries the constructor's call site.
        #[test]
        fn pin_helper_reverting_constructor_carries_caller_line() {
            let output = load_scenarios_output();
            let model = make_build_model_for_scenarios();
            let bc = &output
                .contracts
                .get("project/contracts/Scenarios.t.sol")
                .unwrap()
                .get("HelperRevertingConstructorContract")
                .unwrap()
                .evm
                .bytecode;
            let raw = hex::decode(&bc.object).unwrap();
            let insts = decode_instructions(&raw, &bc.debug_info, &model, true).unwrap();
            let inst =
                first_inst_at_line(&insts, 295).expect("expected `_check`'s require at line 295");
            let lines: Vec<u32> = inst
                .inline_call_sites
                .iter()
                .filter_map(|cs| cs.get_starting_line_number().ok())
                .collect();
            assert!(lines.contains(&298), "got {lines:?}");
        }
    }

    mod jump_type_assignment {
        use super::*;

        #[test]
        fn every_jump_gets_a_non_default_jump_type() {
            // solx inlines helpers, so most JUMPs are dispatcher/abi-related and
            // stay inside a single range — we only require every JUMP/JUMPI to
            // get *some* non-default jump_type (not the NotJump placeholder).
            let output = load_solx_output();
            let bc = &output
                .contracts
                .get("Counter.sol")
                .unwrap()
                .get("Counter")
                .unwrap()
                .evm
                .deployed_bytecode;
            let raw = hex::decode(&bc.object).unwrap();
            let model = make_build_model_for_counter();
            let instructions = decode_instructions(&raw, &bc.debug_info, &model, false).unwrap();

            for inst in &instructions {
                if matches!(inst.opcode, OpCode::JUMP | OpCode::JUMPI) {
                    assert!(
                        inst.jump_type != JumpType::NotJump,
                        "JUMP/JUMPI at pc=0x{:04x} should have a jump_type \
                         assigned by the post-decode pass, got NotJump",
                        inst.pc
                    );
                }
            }
        }

        #[test]
        fn jump_after_swap_falls_back_to_internal_jump() {
            // Synthetic instruction stream:
            //   PC=0    PUSH2 0x0100   ; real destination
            //   PC=3    SWAP1          ; obscures the destination on the stack
            //   PC=4    JUMP
            //   PC=0x100 JUMPDEST       ; inlined-subroutine entry (per ParsedDwarf)
            let make_inst = |pc: u32, opcode: OpCode, push_data: Option<Vec<u8>>| Instruction {
                pc,
                opcode,
                jump_type: JumpType::NotJump,
                push_data,
                location: None,
                inline_call_sites: Box::default(),
            };
            let mut instructions = vec![
                make_inst(0, OpCode::PUSH2, Some(vec![0x61, 0x01, 0x00])),
                make_inst(3, OpCode::SWAP1, None),
                make_inst(4, OpCode::JUMP, None),
                make_inst(0x100, OpCode::JUMPDEST, None),
            ];

            // Range at [0x100, 0x200). Without the SWAP guard the classifier
            // would read 0x100 from the prior PUSH and call this IntoFunction.
            let parsed = make_parsed_dwarf_with_ranges(vec![InlinedRange {
                low_pc: 0x100,
                high_pc: 0x200,
                call_file: None,
                call_line: None,
                call_column: None,
                depth: 1,
                is_artificial: false,
                decl_file: None,
                decl_line: None,
            }]);

            parsed.assign_jump_types(&mut instructions);

            let jump = &instructions[2];
            assert_eq!(jump.opcode, OpCode::JUMP);
            assert_eq!(
                jump.jump_type,
                JumpType::InternalJump,
                "JUMP separated from the prior PUSH by a SWAP must NOT derive its \
                 destination from that PUSH; got jump_type={:?}",
                jump.jump_type,
            );
        }

        /// Happy path companion to the SWAP-guard test: clean `PUSH; JUMP`
        /// must still classify as `IntoFunction`.
        #[test]
        fn jump_directly_after_push_classifies_into_function() {
            let make_inst = |pc: u32, opcode: OpCode, push_data: Option<Vec<u8>>| Instruction {
                pc,
                opcode,
                jump_type: JumpType::NotJump,
                push_data,
                location: None,
                inline_call_sites: Box::default(),
            };
            let mut instructions = vec![
                make_inst(0, OpCode::PUSH2, Some(vec![0x61, 0x01, 0x00])),
                make_inst(3, OpCode::JUMP, None),
                make_inst(0x100, OpCode::JUMPDEST, None),
            ];
            let parsed = make_parsed_dwarf_with_ranges(vec![InlinedRange {
                low_pc: 0x100,
                high_pc: 0x200,
                call_file: None,
                call_line: None,
                call_column: None,
                depth: 1,
                is_artificial: false,
                decl_file: None,
                decl_line: None,
            }]);
            parsed.assign_jump_types(&mut instructions);
            assert_eq!(instructions[1].jump_type, JumpType::IntoFunction);
        }
    }

    mod dwarf_file_matching {
        use super::*;

        /// G5 — exact > suffix > basename; ambiguous matches return None
        /// (a wrong-file frame is harder to spot than a missing reference).
        #[test]
        fn match_dwarf_to_build_model_disambiguates_basenames() {
            let mut map = std::collections::HashMap::new();
            map.insert("contracts/A/Counter.sol".to_string(), 1u32);
            map.insert("contracts/B/Counter.sol".to_string(), 2u32);

            // Exact match wins.
            assert_eq!(
                match_dwarf_to_build_model("contracts/A/Counter.sol", &map),
                Some(1)
            );
            assert_eq!(
                match_dwarf_to_build_model("contracts/B/Counter.sol", &map),
                Some(2)
            );

            // Suffix match disambiguates by parent directory.
            assert_eq!(match_dwarf_to_build_model("A/Counter.sol", &map), Some(1));
            assert_eq!(match_dwarf_to_build_model("B/Counter.sol", &map), Some(2));

            // Basename-only is ambiguous (two equal-specificity candidates):
            // refuse to guess. The renderer falls back to "no source location"
            // rather than emitting a wrong-file frame.
            assert_eq!(
                match_dwarf_to_build_model("Counter.sol", &map),
                None,
                "ambiguous basename collisions must not resolve to either candidate"
            );

            // When one candidate IS more specific than the other, pick it.
            let mut deeper = std::collections::HashMap::new();
            deeper.insert("contracts/A/Counter.sol".to_string(), 1u32);
            deeper.insert("contracts/A/utils/Counter.sol".to_string(), 2u32);
            assert_eq!(
                match_dwarf_to_build_model("utils/Counter.sol", &deeper),
                Some(2),
                "longest matching suffix wins"
            );
        }

        /// G7 — absolute DWARF paths resolve via suffix/basename match against
        /// project-relative `BuildModel` keys without double-prefixing.
        #[test]
        fn match_dwarf_to_build_model_handles_absolute_paths() {
            let mut map = std::collections::HashMap::new();
            map.insert("project/contracts/Counter.sol".to_string(), 7u32);

            // DWARF-side absolute path, BuildModel uses relative key:
            // suffix match `/project/contracts/Counter.sol` doesn't fit, but
            // basename match `Counter.sol` should still find file_id 7.
            assert_eq!(
                match_dwarf_to_build_model("/mnt/host/project/contracts/Counter.sol", &map),
                Some(7),
                "absolute DWARF path with matching basename must resolve to the BuildModel id"
            );

            // Same file via the suffix path — also expected to work.
            assert_eq!(
                match_dwarf_to_build_model("project/contracts/Counter.sol", &map),
                Some(7)
            );
        }

        /// G7' — absolute DWARF path with a basename collision must return
        /// None. Known limitation: a long DWARF path can't disambiguate
        /// when no part of it appears in any `BuildModel` key.
        #[test]
        fn match_dwarf_to_build_model_absolute_path_with_basename_collision() {
            let mut map = std::collections::HashMap::new();
            map.insert("contracts/A/Counter.sol".to_string(), 1u32);
            map.insert("contracts/B/Counter.sol".to_string(), 2u32);

            // Absolute DWARF path with a basename-collision in the BuildModel:
            // basename match is ambiguous; the resolver must report None
            // rather than guess based on iteration order.
            assert_eq!(
                match_dwarf_to_build_model("/mnt/host/build/Counter.sol", &map),
                None,
                "absolute path with ambiguous basename must NOT resolve to either candidate"
            );
        }
    }

    mod source_length_resolution {
        use super::*;

        /// Non-zero `SourceLocation.length` via `BuildModel.ast_spans` — pin
        /// that at least one resolved instruction gets a non-zero span.
        #[test]
        fn solx_instructions_carry_nonzero_source_length() {
            let output = load_scenarios_output();
            let model = make_build_model_for_scenarios();
            let insts = decode_deployed_for(&output, "DirectRequireTest", &model);

            let any_with_length = insts
                .iter()
                .filter_map(|i| i.location.as_deref())
                .any(|loc| loc.length > 0);
            assert!(
                any_with_length,
                "expected at least one decoded instruction's SourceLocation to have \
                 length > 0 once BuildModel.ast_spans is populated"
            );
        }
    }

    mod edge_cases {
        use super::*;

        /// G1 — innermost-first by DIE depth, not range width.
        /// A parent's `DW_AT_ranges` union can be wider than a child's
        /// contiguous range; width-based sorting would invert or tie
        /// the chain.
        #[test]
        fn containing_ranges_orders_by_die_depth_not_width() {
            fn r(depth: u32, low_pc: u64, high_pc: u64) -> InlinedRange {
                InlinedRange {
                    low_pc,
                    high_pc,
                    call_file: None,
                    call_line: None,
                    call_column: None,
                    depth,
                    is_artificial: false,
                    decl_file: None,
                    decl_line: None,
                }
            }

            // Two ranges with **identical** PC span [10, 50). Width-based
            // sorting is unstable here. Depth-based must put depth=3 first.
            let mut inlined = vec![r(2, 10, 50), r(3, 10, 50)];
            inlined.sort_by_key(|r| r.low_pc);
            let parsed = make_parsed_dwarf_with_ranges(inlined);
            let chain = parsed.containing_ranges(20);
            assert_eq!(chain.len(), 2);
            assert_eq!(
                chain[0].depth,
                3,
                "innermost (depth 3) must be first; got depth ordering {:?}",
                chain.iter().map(|r| r.depth).collect::<Vec<_>>()
            );
            assert_eq!(chain[1].depth, 2);

            // Now test the case the audit doc explicitly called out: a child
            // contiguous range wider than the parent's own ranges (can happen
            // if `DW_AT_ranges` for the parent has multiple narrow segments).
            // We model it as two parent segments each narrower than the child.
            let mut inlined = vec![
                r(2, 10, 20), // parent segment 1 (width 10)
                r(2, 30, 40), // parent segment 2 (width 10)
                r(3, 0, 100), // child (width 100, broader than either parent segment)
            ];
            inlined.sort_by_key(|r| r.low_pc);
            let parsed = make_parsed_dwarf_with_ranges(inlined);
            // PC 15 sits in parent's first segment AND inside the child's
            // contiguous range. Depth-3 (child) must still come first.
            let chain = parsed.containing_ranges(15);
            assert!(chain
                .iter()
                .any(|r| r.depth == 3 && r.low_pc == 0 && r.high_pc == 100));
            assert_eq!(
                chain[0].depth, 3,
                "depth-based ordering must pick the inner DIE even when the parent's contiguous segment is narrower"
            );
        }
    }

    /// Each [`DwarfError`] variant gets a round-trip test that drives
    /// `decode_instructions` to the failure mode. Variants that need a
    /// crafted ELF blob to trigger (`CompressedSection`, `MultiCu`) are
    /// not covered here — they're guarded for forward-compat with future
    /// solx output, not regression-prone today.
    mod parse_errors {
        use super::*;

        #[test]
        fn hex_decode_error_on_invalid_hex() {
            let model = make_build_model_for_counter();
            let err = decode_instructions(&[], "not-valid-hex", &model, false)
                .expect_err("non-hex input must fail at hex::decode");
            assert!(
                matches!(err, DwarfError::HexDecode(_)),
                "expected HexDecode, got {err:?}"
            );
        }

        #[test]
        fn elf_parse_error_on_random_bytes() {
            // Valid hex of obviously-not-ELF bytes (random payload that
            // happens to be the right length for object::File::parse to
            // try to interpret).
            let bogus = hex::encode([0xDE, 0xAD, 0xBE, 0xEFu8].repeat(64));
            let model = make_build_model_for_counter();
            let err = decode_instructions(&[], &bogus, &model, false)
                .expect_err("non-ELF bytes must fail at object::File::parse");
            assert!(
                matches!(err, DwarfError::ElfParse(_)),
                "expected ElfParse, got {err:?}"
            );
        }

        /// Real DWARF blob + a bytecode shorter than any DIE PC range →
        /// the bounds check should reject before walking PCs. Stronger
        /// than the old test, which only checked the predicate by hand.
        #[test]
        fn range_escapes_bytecode_on_truncated_bytecode() {
            let output = load_solx_output();
            let bc = &output
                .contracts
                .get("Counter.sol")
                .unwrap()
                .get("Counter")
                .unwrap()
                .evm
                .deployed_bytecode;
            // 4 bytes — well short of any subprogram's high_pc.
            let truncated = [0u8; 4];
            let model = make_build_model_for_counter();
            let err = decode_instructions(&truncated, &bc.debug_info, &model, false)
                .expect_err("truncated bytecode must trigger the bounds check");
            assert!(
                matches!(err, DwarfError::RangeEscapesBytecode { .. }),
                "expected RangeEscapesBytecode, got {err:?}"
            );
        }
    }
}
