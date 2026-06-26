//! Collects EIP-712 canonical type definitions from Solidity sources by
//! parsing them with Slang v2 and walking the resolved AST.
//!
//! This is a Rust port of Hardhat's TypeScript `collectEip712CanonicalTypes`,
//! which walks solc JSON ASTs; here we parse `.sol` files directly with Slang.
//! The canonicalization semantics (member-type encoding, struct dependency
//! ordering, encodability propagation, deduplication) mirror that
//! implementation and `forge bind-json`.

use std::{
    collections::{HashMap, HashSet},
    path::Path,
};

use semver::Version;
use slang_solidity_v2::{
    ast::{Definition, Type},
    compilation::{CompilationBuilder, CompilationUnit},
    utils::{FromSemverError, LanguageVersion},
};

use crate::{
    resolver::{ImportResolver, SourceProvider},
    Eip712Type,
};

#[derive(Debug, thiserror::Error)]
#[error(
    "struct `{name}` cannot be canonicalized because struct dependency `{dependency}` is missing."
)]
pub struct MissingStructDependency {
    name: String,
    dependency: String,
}

impl Eip712Type {
    /// Canonicalizes the provided encodable struct and its transitive
    /// dependencies.
    fn canonicalize(
        root: &EncodableStruct,
        encodables: &HashMap<String, EncodableStruct>,
    ) -> Result<Eip712Type, MissingStructDependency> {
        fn transitive_struct_deps(
            root: &EncodableStruct,
            encodables: &HashMap<String, EncodableStruct>,
        ) -> Result<Vec<String>, MissingStructDependency> {
            let mut visited = HashSet::new();
            let mut stack = root.direct_struct_deps.clone();

            while let Some(next) = stack.pop() {
                if next == root.name || visited.contains(&next) {
                    continue;
                }

                let dependency = encodables.get(&next).ok_or(MissingStructDependency {
                    name: root.name.clone(),
                    dependency: next.clone(),
                })?;

                stack.extend(dependency.direct_struct_deps.clone());
                visited.insert(next.clone());
            }

            Ok(visited.into_iter().collect())
        }

        let dependency_names = transitive_struct_deps(root, encodables)?;
        let mut dependency_heads = dependency_names
            .iter()
            .map(|dependency| {
                let dependency = encodables.get(dependency).ok_or(MissingStructDependency {
                    name: root.name.clone(),
                    dependency: dependency.clone(),
                })?;
                Ok(struct_head(dependency))
            })
            .collect::<Result<Vec<_>, MissingStructDependency>>()?;
        dependency_heads.sort();

        let name = root.name.clone();
        let mut canonical_definition = struct_head(root);
        canonical_definition.push_str(&dependency_heads.concat());

        Ok(Eip712Type {
            name,
            canonical_definition,
        })
    }
}

/// A set of EIP-712 canonical type definitions collected from a compilation
/// unit, keyed by primary type name.
///
/// Names that were seen but cannot be used (a same-name conflict between two
/// files, a non-EIP-712-encodable member, or a transitively non-encodable
/// dependency) are recorded separately so a lookup can explain *why* a type is
/// unavailable rather than reporting a bare "not found".
#[derive(Clone, Debug, Default)]
pub struct Eip712TypeCollection {
    types: HashMap<String, Eip712Type>,
    rejected: HashMap<String, RejectReason>,
}

/// An error type for a struct that exists but could not be converted to an
/// EIP-712 canonical type.
#[derive(Clone, Debug, thiserror::Error)]
#[error("EIP-712 type '{name}' cannot be used: {reason}")]
pub struct Eip712TypeRejected {
    /// The requested type name.
    pub name: String,
    /// Why the type was rejected.
    pub reason: RejectReason,
}

/// Why a [`Eip712Collection::get`] lookup did not return a type.
#[derive(Clone, Debug, thiserror::Error)]
pub enum Eip712CollectionLookupError {
    /// No struct with this name exists in the compilation unit.
    #[error("EIP-712 type '{type_name}' was not found.")]
    NotFound { type_name: String },
    /// A struct with this name exists but cannot be used as an EIP-712 type.
    #[error(transparent)]
    Rejected(#[from] Eip712TypeRejected),
}

impl Eip712TypeCollection {
    /// Looks up a canonical type definition by its primary type name.
    pub fn get(&self, name: &str) -> Result<&Eip712Type, Eip712CollectionLookupError> {
        if let Some(def) = self.types.get(name) {
            Ok(def)
        } else if let Some(reason) = self.rejected.get(name) {
            Err(Eip712TypeRejected {
                name: name.to_owned(),
                reason: reason.clone(),
            }
            .into())
        } else {
            Err(Eip712CollectionLookupError::NotFound {
                type_name: name.to_owned(),
            })
        }
    }
}

/// Errors that prevent collection from running at all (as opposed to per-type
/// rejections, which are surfaced lazily via [`Eip712Collection::get`]).
#[derive(Debug, thiserror::Error)]
pub enum Eip712CollectError {
    /// The provided solc version is invalid.
    #[error(transparent)]
    InvalidSolcVersion(#[from] FromSemverError),

    /// The root source file could not be read.
    #[error("could not read EIP-712 root source {path}: {reason}")]
    RootFileNotFound {
        /// The root source path.
        path: String,
        /// Why it could not be read.
        reason: String,
    },
}

// TODO: `derive(Clone)` on CollectError once `FromSemverError` implements
// `Clone`.
impl Clone for Eip712CollectError {
    fn clone(&self) -> Self {
        match self {
            Self::InvalidSolcVersion(FromSemverError::UnexpectedMetadata) => {
                Self::InvalidSolcVersion(FromSemverError::UnexpectedMetadata)
            }
            Self::InvalidSolcVersion(FromSemverError::UnsupportedVersion) => {
                Self::InvalidSolcVersion(FromSemverError::UnsupportedVersion)
            }
            Self::RootFileNotFound { path, reason } => Self::RootFileNotFound {
                path: path.clone(),
                reason: reason.clone(),
            },
        }
    }
}

/// Collects EIP-712 canonical types reachable from `root_source`.
///
/// `import_map` maps non-relative import paths (as written in `import`
/// statements) to absolute disk paths; relative imports are resolved against
/// the importing file. Parse errors and unresolved imports degrade gracefully
/// — structs that still resolve are collected — but a missing root file is a
/// hard error.
pub fn collect_eip712_types_for_file(
    root_source: &Path,
    solc_version: Version,
    import_resolver: &ImportResolver,
) -> Result<Eip712TypeCollection, Eip712CollectError> {
    let language_version = to_language_version(solc_version)?;

    // Pre-check the root: a build over a missing root only yields a diagnostic
    // and an empty unit, which we would otherwise mistake for "no types".
    if let Err(error) = std::fs::metadata(root_source) {
        return Err(Eip712CollectError::RootFileNotFound {
            path: root_source.display().to_string(),
            reason: error.to_string(),
        });
    }

    let mut builder =
        CompilationBuilder::create(language_version, SourceProvider::new(import_resolver));

    builder.add_file(root_source.to_string_lossy().into_owned());
    let unit = builder.build();

    Ok(collect_eip712_types_from_compilation_unit(&unit))
}

/// Core collection logic, decoupled from disk so it can be unit-tested against
/// an in-memory compilation unit.
pub fn collect_eip712_types_from_compilation_unit(unit: &CompilationUnit) -> Eip712TypeCollection {
    let collected = collect_structs(unit);

    let DedupedCollection { unique, duplicates } = dedup_by_name(collected);

    let EncodableCollection {
        encodables,
        rejected,
    } = reject_non_encodable(unique, duplicates);

    let types = encodables
        .iter()
        .map(|(name, root)| {
            let canonical_type = Eip712Type::canonicalize(root, &encodables)
                .expect("all dependencies should be encodable");

            (name.clone(), canonical_type)
        })
        .collect();

    Eip712TypeCollection { types, rejected }
}

/// Maps a solc [`Version`] to a Slang [`LanguageVersion`]; clamping versions
/// newer than Slang supports down to its latest grammar.
fn to_language_version(solc_version: Version) -> Result<LanguageVersion, FromSemverError> {
    // Fall back to the latest Slang grammar for any solc version newer than what
    // Slang supports.
    let latest: Version = LanguageVersion::LATEST.into();
    if solc_version > latest {
        Ok(LanguageVersion::LATEST)
    } else {
        LanguageVersion::try_from(solc_version)
    }
}

/// A struct definition collected from the AST, with each member's type already
/// encoded to its EIP-712 form (`None` if the member is not encodable).
#[derive(Clone)]
struct CollectedStruct {
    name: String,
    file_id: String,
    members: Vec<CollectedMember>,
}

struct EncodableStruct {
    pub name: String,
    pub members: Vec<EncodableMember>,
    /// The names of structs directly referenced by a struct's members (array
    /// suffixes stripped, self-references excluded).
    pub direct_struct_deps: Vec<String>,
}

impl EncodableStruct {
    /// Constructs a new instance from a [`CollectedStruct`], rejecting it if
    /// any member is not encodable.
    fn new(
        struct_def: CollectedStruct,
        is_struct_fn: impl Fn(&str) -> bool,
    ) -> Result<Self, RejectReason> {
        let mut members = Vec::new();
        let mut non_encodable_members = Vec::new();
        let mut direct_struct_deps = Vec::new();

        for member in struct_def.members {
            if let Some(encoded_type) = member.encoded_type {
                let base = base_type_name(encoded_type.as_str());
                if base != struct_def.name && is_struct_fn(base) {
                    direct_struct_deps.push(base.to_owned());
                }

                members.push(EncodableMember {
                    name: member.name,
                    encoded_type,
                });
            } else {
                non_encodable_members.push(member.name);
            }
        }

        if !non_encodable_members.is_empty() {
            return Err(RejectReason::NonEncodableMembers {
                members: non_encodable_members,
            });
        }

        Ok(Self {
            name: struct_def.name,
            members,
            direct_struct_deps,
        })
    }
}

#[derive(Clone)]
struct CollectedMember {
    name: String,
    /// EIP-712 encoded member type, or `None` if not encodable (mapping,
    /// function, fixed-point, unresolved, …).
    encoded_type: Option<String>,
}

struct EncodableMember {
    name: String,
    encoded_type: String,
}

/// Walks every struct definition in the unit and encodes its members.
fn collect_structs(unit: &CompilationUnit) -> Vec<CollectedStruct> {
    let mut collected = Vec::new();
    for definition in unit.all_definitions() {
        let Definition::Struct(struct_def) = definition else {
            continue;
        };

        let members = struct_def
            .members()
            .iter()
            .map(|member| CollectedMember {
                name: member.name().unparse().to_owned(),
                encoded_type: member.get_type().and_then(|ty| encode_member_type(&ty)),
            })
            .collect();

        collected.push(CollectedStruct {
            name: struct_def.name().unparse().to_owned(),
            file_id: struct_def.get_file_id().to_owned(),
            members,
        });
    }
    collected
}

/// Encodes a resolved member type to its EIP-712 form, following the same
/// conventions as `forge bind-json`: enums become `uint8`, contracts/addresses
/// become `address`, user-defined value types resolve to their underlying
/// elementary type, structs become their bare name (a dependency), and
/// non-encodable types (mappings, functions, fixed-point) yield `None`.
fn encode_member_type(ty: &Type) -> Option<String> {
    match ty {
        Type::Address(_) | Type::Contract(_) | Type::Interface(_) | Type::Library(_) => {
            Some("address".to_owned())
        }
        Type::Boolean(_) => Some("bool".to_owned()),
        Type::Integer(integer) => {
            let prefix = if integer.signed() { "" } else { "u" };
            let bits = integer.bits();
            Some(format!("{prefix}int{bits}"))
        }
        Type::ByteArray(byte_array) => {
            let width = byte_array.width();
            Some(format!("bytes{width}"))
        }
        Type::Bytes(_) => Some("bytes".to_owned()),
        Type::String(_) => Some("string".to_owned()),
        Type::Enum(_) => Some("uint8".to_owned()),
        Type::Struct(struct_type) => match struct_type.definition() {
            Definition::Struct(struct_def) => Some(struct_def.name().unparse().to_owned()),
            _ => None,
        },
        Type::UserDefinedValue(udvt) => udvt.target_type().as_ref().and_then(encode_member_type),
        Type::Array(array) => {
            let base = encode_member_type(&array.element_type())?;
            Some(format!("{base}[]"))
        }
        Type::FixedSizeArray(array) => {
            let base = encode_member_type(&array.element_type())?;
            let size = array.size();
            Some(format!("{base}[{size}]"))
        }
        Type::Mapping(_)
        | Type::Function(_)
        | Type::FixedPointNumber(_)
        | Type::Tuple(_)
        | Type::Literal(_)
        | Type::Void(_) => None,
    }
}

#[derive(Clone, Debug, thiserror::Error)]
pub enum RejectReason {
    #[error("Conflicting definitions of struct '{name}' in: {}", .file_ids.join(", "))]
    Duplicate { name: String, file_ids: Vec<String> },
    #[error("Struct has non-encodable {}: '{}'", if .members.len() == 1 { "member" } else { "members" }, .members.join(", "))]
    NonEncodableMembers { members: Vec<String> },
}

struct DedupedCollection {
    pub unique: HashMap<String, CollectedStruct>,
    pub duplicates: HashMap<String, RejectReason>,
}

/// Deduplicates structs with the same name but different definitions; rejects
/// all of them as unusable.
fn dedup_by_name(collected: Vec<CollectedStruct>) -> DedupedCollection {
    let mut by_name: HashMap<String, Vec<CollectedStruct>> = HashMap::new();
    for struct_def in collected {
        by_name
            .entry(struct_def.name.clone())
            .and_modify(|defs| defs.push(struct_def.clone()))
            .or_insert(vec![struct_def]);
    }

    let mut unique: HashMap<String, CollectedStruct> = HashMap::new();
    let mut duplicates: HashMap<String, RejectReason> = HashMap::new();
    for (struct_name, struct_defs) in by_name {
        let mut iter = struct_defs.iter();

        let next = iter
            .next()
            .expect("at least one struct definition must exist for this name");
        let first_struct_def = next.clone();
        let fingerprint = make_fingerprint(next);

        // If fingerprints match, keep one definition and ignore the rest; otherwise,
        // reject all definitions for this name as unusable.
        if iter.all(|def| make_fingerprint(def) == fingerprint) {
            unique.insert(struct_name, first_struct_def);
        } else {
            let file_ids = struct_defs.into_iter().map(|def| def.file_id).collect();
            duplicates.insert(
                struct_name.clone(),
                RejectReason::Duplicate {
                    name: struct_name,
                    file_ids,
                },
            );
        }
    }

    DedupedCollection { unique, duplicates }
}

/// A deterministic fingerprint of a struct's name and members (including
/// non-encodable members as `<unsupported>`), used to tell identical
/// re-definitions apart from genuine conflicts.
fn make_fingerprint(struct_def: &CollectedStruct) -> String {
    let members: Vec<String> = struct_def
        .members
        .iter()
        .map(|member| {
            let ty = member.encoded_type.as_deref().unwrap_or("<unsupported>");
            let name = &member.name;
            format!("{ty} {name}")
        })
        .collect();

    let name = &struct_def.name;
    let body = members.join(",");
    format!("{name}({body})")
}

/// A set of encodable structs, keyed by name, along with the names of rejected
/// structs and why they were rejected.
struct EncodableCollection {
    pub encodables: HashMap<String, EncodableStruct>,
    pub rejected: HashMap<String, RejectReason>,
}

/// Rejects structs that (transitively) reference non-encodable structs.
fn reject_non_encodable(
    collected: HashMap<String, CollectedStruct>,
    previously_rejected: HashMap<String, RejectReason>,
) -> EncodableCollection {
    let mut encodables = HashMap::new();

    // Every struct name in the unit, including those already rejected (e.g. as
    // duplicates): a member referencing such a struct is still a struct
    // dependency, so the dependent must be rejected alongside it.
    let struct_names: HashSet<String> = collected
        .keys()
        .cloned()
        .chain(previously_rejected.keys().cloned())
        .collect();

    // Combine the previously and newly rejected structs so we can propagate
    // non-encodability to dependents.
    let mut newly_rejected = previously_rejected;

    for (name, struct_def) in collected {
        match EncodableStruct::new(struct_def, |struct_name| struct_names.contains(struct_name)) {
            Ok(encodable) => {
                encodables.insert(name, encodable);
            }
            Err(reason) => {
                newly_rejected.insert(name, reason);
            }
        }
    }

    // Reject structs that transitively reference non-encodable structs, until we
    // reach a fixed point where no encodable struct references a rejected one.
    let mut rejected = HashMap::new();
    while !newly_rejected.is_empty() {
        for name in newly_rejected.keys() {
            encodables.remove(name);
        }

        // Take `newly_rejected` so it's reset for the next iteration
        rejected.extend(std::mem::take(&mut newly_rejected));

        for (name, encodable) in &encodables {
            let non_encodable_members = encodable
                .direct_struct_deps
                .iter()
                .filter(|dependency| !encodables.contains_key(*dependency))
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>();

            if !non_encodable_members.is_empty() {
                newly_rejected.insert(
                    name.clone(),
                    RejectReason::NonEncodableMembers {
                        members: non_encodable_members,
                    },
                );
            }
        }
    }

    EncodableCollection {
        encodables,
        rejected,
    }
}

/// Strips array suffixes from an encoded type to get its base name
/// (`Person[3][2]` -> `Person`).
fn base_type_name(encoded_type: &str) -> &str {
    match encoded_type.split_once('[') {
        Some((base, _)) => base,
        None => encoded_type,
    }
}

/// The `Name(type member,…)` head of a single struct. Only called on encodable
/// structs, whose members all have an encoded type.
fn struct_head(struct_def: &EncodableStruct) -> String {
    let EncodableStruct {
        name,
        members,
        direct_struct_deps: _,
    } = struct_def;

    let members: Vec<String> = members
        .iter()
        .map(|member| {
            let EncodableMember { name, encoded_type } = member;
            format!("{encoded_type} {name}")
        })
        .collect();

    let body = members.join(",");
    format!("{name}({body})")
}

#[cfg(test)]
mod tests {
    use slang_solidity_v2::compilation::CompilationBuilderConfig;

    use super::*;

    /// A [`CompilationBuilderConfig`] that serves sources from memory. File ids
    /// and import paths are one and the same, so imports in test sources are
    /// written as the bare file id (e.g. `import "dep.sol";`).
    struct InMemorySources {
        sources: HashMap<String, String>,
    }

    impl CompilationBuilderConfig for InMemorySources {
        fn read_file(&mut self, file_id: &str) -> Result<String, String> {
            self.sources
                .get(file_id)
                .cloned()
                .ok_or_else(|| format!("no such file: {file_id}"))
        }

        fn resolve_import(&mut self, _source: &str, import_path: &str) -> Result<String, String> {
            if self.sources.contains_key(import_path) {
                Ok(import_path.to_string())
            } else {
                Err(format!("unresolved import: {import_path}"))
            }
        }
    }

    /// Builds a compilation unit from in-memory sources (the first entry is the
    /// root) and collects EIP-712 types from it.
    fn collect(sources: &[(&str, &str)]) -> Eip712TypeCollection {
        let (root, _) = sources.first().expect("at least one source");
        let sources = sources
            .iter()
            .map(|(id, src)| ((*id).to_string(), (*src).to_string()))
            .collect();

        let mut builder =
            CompilationBuilder::create(LanguageVersion::LATEST, InMemorySources { sources });

        builder.add_file((*root).to_string());

        let unit = builder.build();
        collect_eip712_types_from_compilation_unit(&unit)
    }

    /// Convenience function to collect from a single root source.
    fn collect_one(source: &str) -> Eip712TypeCollection {
        collect(&[("root.sol", source)])
    }

    fn get_canonical_type<'a>(collection: &'a Eip712TypeCollection, name: &str) -> &'a str {
        collection
            .get(name)
            .unwrap_or_else(|error| panic!("expected '{name}': {error}"))
            .canonical_definition()
    }

    #[test]
    fn eip712_spec_mail_person() {
        // The canonical example from https://eips.ethereum.org/EIPS/eip-712.
        let collection = collect_one(
            "struct Person { address wallet; string name; }
             struct Mail { Person from; Person to; string contents; }",
        );
        assert_eq!(
            get_canonical_type(&collection, "Mail"),
            "Mail(Person from,Person to,string contents)Person(address wallet,string name)"
        );
        assert_eq!(
            get_canonical_type(&collection, "Person"),
            "Person(address wallet,string name)"
        );
    }

    #[test]
    fn dependencies_sorted_alphabetically() {
        let collection = collect_one(
            "struct Person { address wallet; string name; }
             struct Asset { address token; uint256 amount; }
             struct Transaction { Person from; Asset payload; }",
        );
        // Asset sorts before Person regardless of member order.
        assert_eq!(
            get_canonical_type(&collection, "Transaction"),
            "Transaction(Person from,Asset payload)\
             Asset(address token,uint256 amount)\
             Person(address wallet,string name)"
        );
    }

    #[test]
    fn transitive_dependencies_included_once() {
        let collection = collect_one(
            "struct C { uint256 v; }
             struct B { C c; }
             struct A { B b; C c; }",
        );
        assert_eq!(
            get_canonical_type(&collection, "A"),
            "A(B b,C c)B(C c)C(uint256 v)"
        );
    }

    #[test]
    fn self_recursive_struct_is_supported() {
        // EIP-712 explicitly supports recursive struct types. The primary type
        // is excluded from its own referenced-type set, so a self-reference
        // adds no extra dependency head and the canonical form is just the
        // struct head.
        let collection = collect_one("struct Node { uint256 value; Node[] children; }");
        assert_eq!(
            get_canonical_type(&collection, "Node"),
            "Node(uint256 value,Node[] children)"
        );
    }

    #[test]
    fn mutually_recursive_structs_are_supported() {
        // A cycle (A -> B -> A) is valid under EIP-712. Each type's canonical
        // form lists the other as its sole dependency head.
        let collection = collect_one(
            "struct A { B b; }
             struct B { A a; }",
        );
        assert_eq!(get_canonical_type(&collection, "A"), "A(B b)B(A a)");
        assert_eq!(get_canonical_type(&collection, "B"), "B(A a)A(B b)");
    }

    #[test]
    fn enum_member_is_uint8() {
        let collection = collect_one(
            "enum Color { Red, Green, Blue }
             struct S { Color color; }",
        );
        assert_eq!(get_canonical_type(&collection, "S"), "S(uint8 color)");
    }

    #[test]
    fn contract_interface_library_members_are_address() {
        let collection = collect_one(
            "contract C {}
             interface I {}
             library L {}
             struct S { C c; I i; }",
        );
        assert_eq!(
            get_canonical_type(&collection, "S"),
            "S(address c,address i)"
        );
    }

    #[test]
    fn user_defined_value_type_resolves_to_underlying() {
        let collection = collect_one(
            "type USD is uint256;
             struct S { USD amount; }",
        );
        assert_eq!(get_canonical_type(&collection, "S"), "S(uint256 amount)");
    }

    #[test]
    fn user_defined_value_type_resolves_across_files() {
        let collection = collect(&[
            (
                "root.sol",
                "import \"udvt.sol\";
                 struct S { USD amount; }",
            ),
            ("udvt.sol", "type USD is uint128;"),
        ]);
        assert_eq!(get_canonical_type(&collection, "S"), "S(uint128 amount)");
    }

    #[test]
    fn address_payable_is_address() {
        let collection = collect_one("struct S { address payable recipient; }");
        assert_eq!(get_canonical_type(&collection, "S"), "S(address recipient)");
    }

    #[test]
    fn integer_aliases_are_normalized() {
        let collection = collect_one("struct S { uint a; int b; }");
        assert_eq!(
            get_canonical_type(&collection, "S"),
            "S(uint256 a,int256 b)"
        );
    }

    #[test]
    fn byte_and_string_types() {
        let collection = collect_one("struct S { bytes data; string text; bytes17 fixed_bytes; }");
        assert_eq!(
            get_canonical_type(&collection, "S"),
            "S(bytes data,string text,bytes17 fixed_bytes)"
        );
    }

    #[test]
    fn arrays_dynamic_fixed_and_nested() {
        let collection = collect_one(
            "struct S { uint256[] dynamic; uint256[3] fixed_size; uint256[3][2] nested; }",
        );
        assert_eq!(
            get_canonical_type(&collection, "S"),
            "S(uint256[] dynamic,uint256[3] fixed_size,uint256[3][2] nested)"
        );
    }

    #[test]
    fn array_of_structs() {
        let collection = collect_one(
            "struct Person { address wallet; string name; }
             struct Group { Person[] members; }",
        );
        assert_eq!(
            get_canonical_type(&collection, "Group"),
            "Group(Person[] members)Person(address wallet,string name)"
        );
    }

    #[test]
    fn mapping_member_makes_struct_non_encodable() {
        let collection = collect_one("struct S { mapping(uint256 => uint256) balances; }");
        assert!(matches!(
            collection.get("S"),
            Err(Eip712CollectionLookupError::Rejected(Eip712TypeRejected {
                reason: RejectReason::NonEncodableMembers { members },
                ..
            })
        ) if members.iter().any(|member| member.contains("balances"))));
    }

    #[test]
    fn non_encodability_propagates_to_dependents() {
        let collection = collect_one(
            "struct Inner { mapping(uint256 => uint256) m; }
             struct Outer { Inner inner; }",
        );
        assert!(matches!(
            collection.get("Inner"),
            Err(Eip712CollectionLookupError::Rejected( Eip712TypeRejected { reason: RejectReason::NonEncodableMembers { members } ,.. })) if members.iter().any(|member| member.contains("m"))
        ));
        let outer = collection.get("Outer").unwrap_err();
        assert!(
            matches!(
                &outer,
                Eip712CollectionLookupError::Rejected(
                    Eip712TypeRejected { reason: RejectReason::NonEncodableMembers { members }, .. } ,
                ) if members.iter().any(|member| member.contains("Inner"))
            ),
            "unexpected: {outer}"
        );
    }

    #[test]
    fn function_typed_member_is_non_encodable() {
        let collection = collect_one("struct S { function() external fn; }");
        assert!(matches!(
            collection.get("S"),
            Err(Eip712CollectionLookupError::Rejected(Eip712TypeRejected {
                reason: RejectReason::NonEncodableMembers { members },
                ..
            })) if members.iter().any(|member| member.contains("fn"))
        ));
    }

    #[test]
    fn file_level_and_contract_nested_structs_both_collected() {
        let collection = collect_one(
            "struct TopLevel { uint256 a; }
             contract C { struct Nested { uint256 b; } }",
        );
        assert_eq!(
            get_canonical_type(&collection, "TopLevel"),
            "TopLevel(uint256 a)"
        );
        assert_eq!(
            get_canonical_type(&collection, "Nested"),
            "Nested(uint256 b)"
        );
    }

    #[test]
    fn identical_duplicate_definitions_dedupe() {
        let collection = collect(&[
            (
                "root.sol",
                "import \"other.sol\";
                 struct S { uint256 a; }",
            ),
            ("other.sol", "struct S { uint256 a; }"),
        ]);
        assert_eq!(get_canonical_type(&collection, "S"), "S(uint256 a)");
    }

    #[test]
    fn conflicting_definitions_are_rejected_others_unaffected() {
        let collection = collect_one(
            "struct S { uint256 a; }
             contract C { struct S { uint256 b; } }
             struct Ok { uint256 c; }",
        );
        assert!(matches!(
            collection.get("S"),
            Err(Eip712CollectionLookupError::Rejected(Eip712TypeRejected { reason: RejectReason::Duplicate { name, .. }, .. })) if name == "S"
        ));
        // An unrelated struct is still usable.
        assert_eq!(get_canonical_type(&collection, "Ok"), "Ok(uint256 c)");
    }

    #[test]
    fn dependent_of_conflicting_struct_is_rejected() {
        let collection = collect_one(
            "struct S { uint256 a; }
             contract C { struct S { uint256 b; } }
             struct Uses { S s; }",
        );
        // `Uses` is well-formed in isolation; it is rejected because its member
        // references the unusable `S`, which surfaces as a non-encodable member.
        let uses = collection.get("Uses").unwrap_err();
        assert!(
            matches!(&uses, Eip712CollectionLookupError::Rejected(Eip712TypeRejected { reason: RejectReason::NonEncodableMembers { members }, .. }) if members.iter().any(|member| member == "S")),
            "unexpected: {uses}"
        );
    }

    #[test]
    fn import_aliasing_uses_definition_name() {
        let collection = collect(&[
            (
                "root.sol",
                "import { Person as Account } from \"person.sol\";
                 struct Wallet { Account owner; }",
            ),
            (
                "person.sol",
                "struct Person { address addr; string handle; }",
            ),
        ]);
        // The dependency is encoded under its definition name, not the alias.
        assert_eq!(
            get_canonical_type(&collection, "Wallet"),
            "Wallet(Person owner)Person(address addr,string handle)"
        );
    }

    #[test]
    fn unknown_type_is_not_found() {
        let collection = collect_one("struct S { uint256 a; }");
        assert!(matches!(
            collection.get("DoesNotExist"),
            Err(Eip712CollectionLookupError::NotFound { type_name }) if type_name == "DoesNotExist"
        ));
    }

    mod version_mapping {
        use super::*;

        #[test]
        fn exact_supported_version() {
            assert_eq!(
                to_language_version(Version::new(0, 8, 24)).unwrap(),
                LanguageVersion::V0_8_24
            );
        }

        #[test]
        fn clamps_newer_versions_to_latest() {
            assert_eq!(
                to_language_version(Version::new(0, 9, 0)).unwrap(),
                LanguageVersion::LATEST
            );
        }

        #[test]
        fn rejects_versions_older_than_0_8_0() {
            assert!(matches!(
                to_language_version(Version::new(0, 7, 6)),
                Err(FromSemverError::UnsupportedVersion)
            ));
        }

        #[test]
        fn rejects_versions_with_build_and_prerelease_metadata() {
            let version = Version::parse("0.8.24+commit.abcdef").unwrap();
            assert!(matches!(
                to_language_version(version),
                Err(FromSemverError::UnexpectedMetadata)
            ));
        }
    }
}
