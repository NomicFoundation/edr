//! Collects EIP-712 canonical type definitions from Solidity sources by
//! parsing them with Slang v2 and walking the resolved AST.
//!
//! This is a Rust port of Hardhat's TypeScript `collectEip712CanonicalTypes`,
//! which walks solc JSON ASTs, while here we parse `.sol` files directly with
//! Slang.
//! The canonicalization semantics (member-type encoding, struct dependency
//! ordering, encodability propagation, deduplication) mirror that
//! implementation and `forge bind-json`.

use std::collections::{HashMap, HashSet};

use slang_solidity_v2::{
    ast::{Definition, StructMember, Type, TypeName},
    compilation::CompilationUnit,
};

use crate::Eip712Type;

/// A struct survived encodability filtering while one of its dependencies did
/// not, which the fixed point in `reject_non_encodable` is meant to prevent.
#[derive(Debug, thiserror::Error)]
#[error(
    "struct `{name}` cannot be canonicalized because struct dependency `{dependency}` is missing"
)]
struct MissingStructDependency {
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
        /// Resolves a dependency name the root (transitively) references.
        fn dependency<'a>(
            encodables: &'a HashMap<String, EncodableStruct>,
            root: &EncodableStruct,
            name: &str,
        ) -> Result<&'a EncodableStruct, MissingStructDependency> {
            encodables.get(name).ok_or_else(|| MissingStructDependency {
                name: root.name.clone(),
                dependency: name.to_owned(),
            })
        }

        fn transitive_struct_deps(
            root: &EncodableStruct,
            encodables: &HashMap<String, EncodableStruct>,
        ) -> Result<Vec<String>, MissingStructDependency> {
            /// A canonical type inlines its dependencies by bare name, so the
            /// traversal is by name too. Every dependency was checked to
            /// resolve to the declaration collection selected for that name.
            fn dependency_names(struct_def: &EncodableStruct) -> Vec<String> {
                struct_def
                    .direct_struct_deps
                    .iter()
                    .map(|dependency| dependency.name.clone())
                    .collect()
            }

            let mut visited = HashSet::new();
            let mut stack = dependency_names(root);

            while let Some(next) = stack.pop() {
                if next == root.name || visited.contains(&next) {
                    continue;
                }

                stack.extend(dependency_names(dependency(encodables, root, &next)?));
                visited.insert(next);
            }

            Ok(visited.into_iter().collect())
        }

        // EIP-712 orders the referenced types by *name*, which is not the same
        // as ordering their rendered heads: `(` sorts above `$`, so `A` and
        // `A$` would render in the wrong order.
        let mut dependency_names = transitive_struct_deps(root, encodables)?;
        dependency_names.sort();

        let dependency_heads = dependency_names
            .iter()
            .map(|name| Ok(struct_head(dependency(encodables, root, name)?)))
            .collect::<Result<Vec<_>, MissingStructDependency>>()?;

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
/// Names that were seen but cannot be used (a same-name conflict the asking
/// source cannot settle, a reference to a name that resolves to more than one
/// struct, a non-EIP-712-encodable member, or a transitively non-encodable
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
#[error("EIP-712 type `{name}` cannot be used: {reason}")]
pub struct Eip712TypeRejected {
    /// The requested type name.
    pub name: String,
    /// Why the type was rejected.
    pub reason: RejectReason,
}

/// Why a [`Eip712TypeCollection::get`] lookup did not return a type.
#[derive(Clone, Debug, thiserror::Error)]
pub enum Eip712CollectionLookupError {
    /// No struct with this name exists in the compilation unit.
    #[error("EIP-712 type `{type_name}` was not found")]
    NotFound {
        /// The name that was looked up.
        type_name: String,
    },
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

    /// Whether the source declared no struct at all — neither usable nor
    /// rejected.
    pub fn is_empty(&self) -> bool {
        self.rejected.is_empty() && self.types.is_empty()
    }
}

/// The struct definitions of a unit, keyed by the file declaring them, with
/// every member already encoded.
///
/// A unit can span several unrelated roots, so a query is scoped per root.
#[derive(Debug)]
pub struct UnitStructs {
    /// Sorted by file id, so a report over several files reads the same on
    /// every run.
    by_file: Vec<(String, Vec<CollectedStruct>)>,
}

impl UnitStructs {
    /// Encodes every struct member in `unit`, keyed by the file declaring it.
    pub fn from_unit(unit: &CompilationUnit) -> Self {
        let mut by_file: HashMap<String, Vec<CollectedStruct>> = HashMap::new();
        for definition in unit.all_definitions() {
            let Definition::Struct(struct_def) = definition else {
                continue;
            };

            let members = struct_def
                .members()
                .iter()
                .map(|member| CollectedMember {
                    name: member.name().unparse().to_owned(),
                    encoded_type: encode_member(&member),
                })
                .collect();

            let file_id = struct_def.get_file_id().to_owned();
            by_file
                .entry(file_id.clone())
                .or_default()
                .push(CollectedStruct {
                    name: struct_def.name().unparse().to_owned(),
                    file_id,
                    members,
                });
        }

        let mut by_file: Vec<_> = by_file.into_iter().collect();
        by_file.sort_unstable_by(|(left, _), (right, _)| left.cmp(right));

        Self { by_file }
    }

    /// Collects the canonical types declared in `file_ids`, scoped to the root
    /// `root_file_id`.
    ///
    /// When a name resolves to definitions that disagree, the one declared in
    /// the root wins a direct lookup, so a test's own struct is never shadowed
    /// from deep in its import graph. Structs *referencing* such a name are
    /// rejected either way, because a canonical type identifies its
    /// dependencies by bare name alone.
    ///
    /// `root_file_id` need not be among `file_ids`, which is how a caller
    /// keeps a file out of the collection. A root left out contributes none of
    /// its own structs and settles no same-name clash, while still scoping the
    /// query. A struct that only `excluded_file_ids` declare is rejected along
    /// with every struct referencing it, so a lookup says why.
    pub fn types_for(
        &self,
        root_file_id: &str,
        file_ids: &HashSet<&str>,
        excluded_file_ids: &HashSet<&str>,
    ) -> Eip712TypeCollection {
        let collected: Vec<CollectedStruct> = self
            .by_file
            .iter()
            .filter(|(file_id, _)| file_ids.contains(file_id.as_str()))
            .flat_map(|(_, structs)| structs.iter().cloned())
            .collect();

        // A struct only excluded files declare is still a struct to the
        // members referencing it, which cannot be encoded without it.
        let collected_names: HashSet<&str> = collected
            .iter()
            .map(|struct_def| struct_def.name.as_str())
            .collect();
        let mut excluded_files_by_name: HashMap<String, Vec<String>> = HashMap::new();
        for (file_id, structs) in &self.by_file {
            if !excluded_file_ids.contains(file_id.as_str()) {
                continue;
            }
            for struct_def in structs {
                if collected_names.contains(struct_def.name.as_str()) {
                    continue;
                }

                excluded_files_by_name
                    .entry(struct_def.name.clone())
                    .or_default()
                    .push(file_id.clone());
            }
        }
        let excluded_only = excluded_files_by_name
            .into_iter()
            .map(|(name, mut file_ids)| {
                file_ids.dedup();
                (name, RejectReason::DeclaredInExcludedFiles { file_ids })
            })
            .collect();

        into_collection(collected, root_file_id, excluded_only, excluded_file_ids)
    }
}

/// Resolves same-name structs against `root_file_id`, then canonicalizes every
/// struct EIP-712 can encode. `already_rejected` names structs that cannot be
/// served before any encoding is attempted, so their dependents are rejected
/// too.
fn into_collection(
    collected: Vec<CollectedStruct>,
    root_file_id: &str,
    mut already_rejected: HashMap<String, RejectReason>,
    excluded_file_ids: &HashSet<&str>,
) -> Eip712TypeCollection {
    // Deduplication keeps a single declaration per name, so the dependency
    // check below must see them all: otherwise which one a member resolves to
    // would decide the outcome by file-id order alone.
    let mut dependencies_by_name: HashMap<String, Vec<StructDependency>> = HashMap::new();
    for struct_def in &collected {
        dependencies_by_name
            .entry(struct_def.name.clone())
            .or_default()
            .extend(struct_dependencies(struct_def));
    }
    for dependencies in dependencies_by_name.values_mut() {
        dependencies.sort();
        dependencies.dedup();
    }

    let DedupedCollection {
        unique,
        duplicates,
        ambiguous,
        selected_declarations,
    } = dedup_by_name(collected, root_file_id);
    already_rejected.extend(duplicates);

    let EncodableCollection {
        encodables,
        rejected,
    } = reject_non_encodable(
        unique,
        already_rejected,
        &ambiguous,
        &selected_declarations,
        &dependencies_by_name,
        excluded_file_ids,
    );

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

/// A struct definition collected from the AST, with each member's type already
/// encoded to its EIP-712 form (`None` if the member is not encodable).
#[derive(Clone, Debug)]
struct CollectedStruct {
    name: String,
    file_id: String,
    members: Vec<CollectedMember>,
}

/// A struct referenced by another struct's member, identified by the
/// declaration the member resolved to rather than by name alone.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct StructDependency {
    name: String,
    file_id: String,
}

struct EncodableStruct {
    pub name: String,
    pub members: Vec<EncodableMember>,
    /// The structs directly referenced by a struct's members (array suffixes
    /// stripped, self-references excluded).
    pub direct_struct_deps: Vec<StructDependency>,
}

/// The structs a declaration's members reference, each identified by the
/// declaration it resolves to, sorted and without repeats.
///
/// A member is a self-reference only when it resolves to this very
/// declaration. A same-named struct from another file is a dependency, and
/// inlining this body for it would hash the wrong type.
fn struct_dependencies(struct_def: &CollectedStruct) -> Vec<StructDependency> {
    let mut dependencies: Vec<StructDependency> = struct_def
        .members
        .iter()
        .filter_map(|member| {
            let EncodedMemberType {
                encoded,
                struct_file_id,
            } = member.encoded_type.as_ref().ok()?;
            let file_id = struct_file_id.as_ref()?;
            let name = base_type_name(encoded);

            (name != struct_def.name || *file_id != struct_def.file_id).then(|| StructDependency {
                name: name.to_owned(),
                file_id: file_id.clone(),
            })
        })
        .collect();

    // Two members of the same struct type are one dependency, and naming it
    // twice in a rejection would read as two.
    dependencies.sort();
    dependencies.dedup();

    dependencies
}

impl EncodableStruct {
    /// Constructs a new instance from a [`CollectedStruct`], rejecting it if
    /// any member is not encodable.
    fn new(struct_def: CollectedStruct) -> Result<Self, RejectReason> {
        let direct_struct_deps = struct_dependencies(&struct_def);

        let mut members = Vec::new();
        let mut unresolved_members = Vec::new();
        let mut non_encodable_members = Vec::new();

        for member in struct_def.members {
            match member.encoded_type {
                Ok(EncodedMemberType { encoded, .. }) => {
                    members.push(EncodableMember {
                        name: member.name,
                        encoded_type: encoded,
                    });
                }
                Err(MemberTypeProblem::Unresolved { declared }) => {
                    unresolved_members.push(match declared {
                        Some(declared) => format!("{declared} {}", member.name),
                        None => member.name,
                    });
                }
                Err(MemberTypeProblem::NotEncodable) => non_encodable_members.push(member.name),
            }
        }

        // An unresolved type is the more actionable of the two, and usually
        // the cause of the other members looking wrong too.
        if !unresolved_members.is_empty() {
            return Err(RejectReason::UnresolvedMemberTypes {
                members: unresolved_members,
            });
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

#[derive(Clone, Debug)]
struct CollectedMember {
    name: String,
    encoded_type: Result<EncodedMemberType, MemberTypeProblem>,
}

/// A member type in its EIP-712 encoding, together with the file declaring the
/// struct it is based on.
///
/// A canonical type inlines a dependency by bare name, so the declaring file is
/// the only thing that tells two same-named structs apart.
#[derive(Clone, Debug)]
struct EncodedMemberType {
    encoded: String,
    /// `Some` when the base type is a struct, naming the file that declares it.
    /// Array suffixes leave it unchanged.
    struct_file_id: Option<String>,
}

impl EncodedMemberType {
    /// Constructs an encoding whose base type is no struct, so it is never a
    /// dependency.
    fn elementary(encoded: String) -> Self {
        Self {
            encoded,
            struct_file_id: None,
        }
    }

    /// Wraps the encoding in an array suffix, keeping the base type's
    /// declaring file.
    fn in_array(self, suffix: &str) -> Self {
        let Self {
            encoded,
            struct_file_id,
        } = self;

        Self {
            encoded: format!("{encoded}{suffix}"),
            struct_file_id,
        }
    }
}

/// Why a struct member has no EIP-712 encoding.
#[derive(Clone, Debug)]
enum MemberTypeProblem {
    /// The member's type did not resolve, so what it encodes to is unknown.
    /// The file declaring it is usually one no import could reach.
    Unresolved {
        /// The type as written, when it can be recovered from the source.
        declared: Option<String>,
    },
    /// The type resolved, and EIP-712 has no encoding for it: a mapping, a
    /// function, a fixed-point number, or an array of unevaluable length.
    NotEncodable,
}

struct EncodableMember {
    name: String,
    encoded_type: String,
}

/// Encodes a member's type, distinguishing a type that did not resolve from
/// one that resolved to something EIP-712 cannot encode.
fn encode_member(member: &StructMember) -> Result<EncodedMemberType, MemberTypeProblem> {
    let Some(resolved) = member.get_type() else {
        return Err(MemberTypeProblem::Unresolved {
            declared: declared_type_name(&member.type_name()),
        });
    };

    encode_member_type(&resolved).ok_or(MemberTypeProblem::NotEncodable)
}

/// Recovers a member's type as written, so an unresolved type can be named in
/// an error. Only the forms an unresolved type can take are rendered.
fn declared_type_name(type_name: &TypeName) -> Option<String> {
    match type_name {
        TypeName::IdentifierPath(path) => Some(
            path.iter()
                .map(|segment| segment.unparse().to_owned())
                .collect::<Vec<_>>()
                .join("."),
        ),
        TypeName::ArrayTypeName(array) => {
            declared_type_name(&array.operand()).map(|base| format!("{base}[]"))
        }
        TypeName::ElementaryType(_) | TypeName::FunctionType(_) | TypeName::MappingType(_) => None,
    }
}

/// Encodes a resolved member type to its EIP-712 form, following the same
/// conventions as `forge bind-json`: enums become `uint8`, contracts/addresses
/// become `address`, user-defined value types resolve to their underlying
/// elementary type, structs become their bare name (a dependency), and
/// non-encodable types (mappings, functions, fixed-point) yield `None`.
fn encode_member_type(ty: &Type) -> Option<EncodedMemberType> {
    match ty {
        Type::Address(_) | Type::Contract(_) | Type::Interface(_) | Type::Library(_) => {
            Some(EncodedMemberType::elementary("address".to_owned()))
        }
        Type::Boolean(_) => Some(EncodedMemberType::elementary("bool".to_owned())),
        Type::Integer(integer) => {
            let prefix = if integer.signed() { "" } else { "u" };
            let bits = integer.bits();
            Some(EncodedMemberType::elementary(format!("{prefix}int{bits}")))
        }
        Type::ByteArray(byte_array) => {
            let width = byte_array.width();
            Some(EncodedMemberType::elementary(format!("bytes{width}")))
        }
        Type::Bytes(_) => Some(EncodedMemberType::elementary("bytes".to_owned())),
        Type::String(_) => Some(EncodedMemberType::elementary("string".to_owned())),
        Type::Enum(_) => Some(EncodedMemberType::elementary("uint8".to_owned())),
        Type::Struct(struct_type) => match struct_type.definition() {
            Definition::Struct(struct_def) => Some(EncodedMemberType {
                encoded: struct_def.name().unparse().to_owned(),
                struct_file_id: Some(struct_def.get_file_id().to_owned()),
            }),
            _ => None,
        },
        Type::UserDefinedValue(udvt) => udvt.target_type().as_ref().and_then(encode_member_type),
        Type::Array(array) => Some(encode_member_type(&array.element_type())?.in_array("[]")),
        Type::FixedSizeArray(array) => {
            let base = encode_member_type(&array.element_type())?;
            // Slang yields 0 for a length it cannot constant-fold, e.g.
            // `uint256[Lib.MAX]`. Solidity rejects a genuinely zero-length
            // fixed array, so 0 means "unknown length" and encoding it as
            // `[0]` would hash a type the source does not declare.
            match array.size() {
                0 => None,
                size => Some(base.in_array(&format!("[{size}]"))),
            }
        }
        Type::Mapping(_)
        | Type::Function(_)
        | Type::FixedPointNumber(_)
        | Type::Tuple(_)
        | Type::Literal(_)
        | Type::Void(_) => None,
    }
}

/// Renders `noun` for `count` items, so an error naming one member or type
/// does not read as if it named several.
fn pluralize(count: usize, noun: &str) -> String {
    pluralize_with(count, noun, &format!("{noun}s"))
}

/// Picks the form of a noun for `count`, for nouns whose forms are not a
/// matter of an `s`.
fn pluralize_with(count: usize, singular: &str, plural: &str) -> String {
    if count == 1 {
        singular.to_owned()
    } else {
        plural.to_owned()
    }
}

/// Why a struct that exists in the sources cannot serve as an EIP-712 type.
#[derive(Clone, Debug, thiserror::Error)]
pub enum RejectReason {
    /// Every file declaring the struct is excluded from collection, so neither
    /// it nor a struct referencing it can be encoded.
    #[error(
        "struct is declared only in {} excluded from EIP-712 type collection: {}. Declare it in a \
         collected file, or narrow the exclusion",
        pluralize_with(.file_ids.len(), "a file", "files"),
        .file_ids.join(", ")
    )]
    DeclaredInExcludedFiles {
        /// The excluded files declaring it, sorted.
        file_ids: Vec<String>,
    },
    /// Two files declare a struct of this name with different members, and
    /// neither is the source that asked, so the name is ambiguous.
    #[error(
        "conflicting definitions in: {}. Rename one of them, reference it from a source that \
         declares it, or exclude one of those files from EIP-712 type collection",
        .file_ids.join(", ")
    )]
    Duplicate {
        /// The distinct files declaring it, sorted.
        file_ids: Vec<String>,
    },
    /// The struct declares a member whose type did not resolve, so what it
    /// encodes to is unknown.
    #[error(
        "struct has {} with an unresolved type: `{}`. Check that the file declaring the type is \
         reachable, adding an import mapping if it is behind a package import",
        pluralize(.members.len(), "member"),
        .members.join("`, `")
    )]
    UnresolvedMemberTypes {
        /// The offending members, each as its declared type and name.
        members: Vec<String>,
    },
    /// The struct declares a member EIP-712 cannot encode — a mapping, a
    /// function, a fixed-point number, or an array of unevaluable length.
    #[error(
        "struct has non-encodable {}: `{}`",
        pluralize(.members.len(), "member"),
        .members.join("`, `")
    )]
    NonEncodableMembers {
        /// The offending member names.
        members: Vec<String>,
    },
    /// The struct references a name that resolves to different structs in
    /// different files. A canonical type inlines its dependencies by bare
    /// name, so there is no way to say which one was meant.
    #[error(
        "struct references ambiguously-named {}: `{}`. Rename one of the conflicting structs, \
         reference it from a source that declares it, or exclude the file declaring one of them \
         from EIP-712 type collection",
        pluralize(.dependencies.len(), "type"),
        .dependencies.join("`, `")
    )]
    AmbiguousDependencies {
        /// The referenced names that resolve to more than one struct.
        dependencies: Vec<String>,
    },
    /// The struct references a struct declared in an excluded file. An
    /// excluded file contributes no struct, so there is no body to inline
    /// under the dependency's name, whatever a collected file of that name
    /// declares.
    #[error(
        "struct references {} declared in {} excluded from EIP-712 type collection: {}. Reference \
         a definition from a collected file, or narrow the exclusion",
        pluralize_with(.dependencies.len(), "a type", "types"),
        pluralize_with(.dependencies.len(), "a file", "files"),
        .dependencies.join(", ")
    )]
    DependenciesInExcludedFiles {
        /// The referenced structs, sorted, each naming the struct and the
        /// file declaring it.
        dependencies: Vec<String>,
    },
    /// The struct is well-formed but references a struct that was itself
    /// rejected, so it cannot be encoded either.
    #[error(
        "struct references unusable {}: `{}`",
        pluralize(.dependencies.len(), "type"),
        .dependencies.join("`, `")
    )]
    UnusableDependencies {
        /// The referenced struct type names that are unusable.
        dependencies: Vec<String>,
    },
}

struct DedupedCollection {
    pub unique: HashMap<String, CollectedStruct>,
    pub duplicates: HashMap<String, RejectReason>,
    /// Names that resolve to different structs in different files. The root's
    /// definition wins a direct lookup by name, but a canonical type inlines
    /// its dependencies by bare name, so a struct *referencing* such a name
    /// would have to guess which body to inline and is rejected instead.
    pub ambiguous: HashSet<String>,
    /// For every name that is not `ambiguous`, the collected files whose
    /// declaration is the one `unique` holds. Several files can declare the
    /// very same body, and a member resolving to any of them encodes alike.
    pub selected_declarations: HashMap<String, HashSet<String>>,
}

/// Resolves structs sharing a name to a single definition.
///
/// Definitions that agree collapse to one. When they disagree, a definition in
/// `root_file_id` — the source whose test suite is asking — wins a direct
/// lookup, so a test's own struct is never shadowed by a collision deep in its
/// import graph; with no single root definition to prefer, the name is
/// rejected outright.
///
/// Either way the name is recorded as ambiguous, because a canonical type
/// identifies its dependencies by bare name alone: any struct referencing it
/// could be encoded with the wrong body and is rejected by
/// [`reject_non_encodable`].
fn dedup_by_name(collected: Vec<CollectedStruct>, root_file_id: &str) -> DedupedCollection {
    let mut by_name: HashMap<String, Vec<CollectedStruct>> = HashMap::new();
    for struct_def in collected {
        by_name
            .entry(struct_def.name.clone())
            .or_default()
            .push(struct_def);
    }

    let mut unique: HashMap<String, CollectedStruct> = HashMap::new();
    let mut duplicates: HashMap<String, RejectReason> = HashMap::new();
    let mut ambiguous = HashSet::new();
    let mut selected_declarations: HashMap<String, HashSet<String>> = HashMap::new();
    for (struct_name, struct_defs) in by_name {
        let (first, rest) = struct_defs
            .split_first()
            .expect("a name is only present because at least one struct declared it");

        let fingerprint = make_fingerprint(first);
        if rest.iter().all(|def| make_fingerprint(def) == fingerprint) {
            selected_declarations.insert(
                struct_name.clone(),
                struct_defs.iter().map(|def| def.file_id.clone()).collect(),
            );
            unique.insert(struct_name, first.clone());
            continue;
        }

        ambiguous.insert(struct_name.clone());

        let mut in_root = struct_defs.iter().filter(|def| def.file_id == root_file_id);
        // Exactly one definition in the source that asked: it wins. With none,
        // or with two of them, there is nothing to prefer.
        if let (Some(root_def), None) = (in_root.next(), in_root.next()) {
            unique.insert(struct_name, root_def.clone());
        } else {
            let mut file_ids: Vec<String> =
                struct_defs.iter().map(|def| def.file_id.clone()).collect();
            file_ids.sort();
            file_ids.dedup();
            duplicates.insert(struct_name.clone(), RejectReason::Duplicate { file_ids });
        }
    }

    DedupedCollection {
        unique,
        duplicates,
        ambiguous,
        selected_declarations,
    }
}

/// A deterministic fingerprint of a struct's name and members (including
/// non-encodable members as `<unsupported>`), used to tell identical
/// re-definitions apart from genuine conflicts.
fn make_fingerprint(struct_def: &CollectedStruct) -> String {
    let members: Vec<String> = struct_def
        .members
        .iter()
        .map(|member| {
            let ty = match &member.encoded_type {
                Ok(encoded_type) => encoded_type.encoded.as_str(),
                Err(MemberTypeProblem::Unresolved { .. }) => "<unresolved>",
                Err(MemberTypeProblem::NotEncodable) => "<unsupported>",
            };
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

/// Rejects a name whose declarations reference a struct body that cannot be
/// inlined under the dependency's bare name.
///
/// A canonical type names its dependencies by bare name alone, so a member must
/// resolve to the declaration that collection selected for that name.
/// `dependencies` spans every collected declaration of the name, so the verdict
/// does not turn on which one deduplication kept. A name no collected file
/// declares is left to the fixed point in [`reject_non_encodable`], which
/// reports it through the rejection the name itself already carries.
fn unencodable_dependencies(
    dependencies: &[StructDependency],
    ambiguous: &HashSet<String>,
    selected_declarations: &HashMap<String, HashSet<String>>,
    excluded_file_ids: &HashSet<&str>,
) -> Option<RejectReason> {
    /// Names a dependency together with the file it resolved to, so a report
    /// distinguishes same-named structs.
    fn describe(dependency: &StructDependency) -> String {
        let StructDependency { name, file_id } = dependency;
        format!("`{name}` in {file_id}")
    }

    let mut ambiguous_dependencies = Vec::new();
    let mut excluded_dependencies = Vec::new();

    for dependency in dependencies {
        if ambiguous.contains(&dependency.name) {
            ambiguous_dependencies.push(dependency.name.clone());
            continue;
        }

        let Some(selected_files) = selected_declarations.get(&dependency.name) else {
            continue;
        };

        if selected_files.contains(&dependency.file_id) {
            continue;
        }

        debug_assert!(
            excluded_file_ids.contains(dependency.file_id.as_str()),
            "a dependency resolves to a collected or an excluded file, and a collected \
             declaration deduplication did not select differs from the selected one, which makes \
             the name ambiguous: {dependency:?}"
        );
        excluded_dependencies.push(describe(dependency));
    }

    // Only one reason is reported, most actionable first.
    if !ambiguous_dependencies.is_empty() {
        // Two members of the same ambiguous type are one dependency, and
        // naming it twice would read as two.
        ambiguous_dependencies.dedup();

        return Some(RejectReason::AmbiguousDependencies {
            dependencies: ambiguous_dependencies,
        });
    }

    if !excluded_dependencies.is_empty() {
        return Some(RejectReason::DependenciesInExcludedFiles {
            dependencies: excluded_dependencies,
        });
    }

    None
}

/// Rejects structs that (transitively) reference non-encodable structs.
fn reject_non_encodable(
    collected: HashMap<String, CollectedStruct>,
    previously_rejected: HashMap<String, RejectReason>,
    ambiguous: &HashSet<String>,
    selected_declarations: &HashMap<String, HashSet<String>>,
    dependencies_by_name: &HashMap<String, Vec<StructDependency>>,
    excluded_file_ids: &HashSet<&str>,
) -> EncodableCollection {
    let mut encodables = HashMap::new();

    // Seeded with the earlier rejections so the fixed point below propagates
    // those to their dependents too.
    let mut newly_rejected = previously_rejected;

    for (name, struct_def) in collected {
        match EncodableStruct::new(struct_def) {
            Ok(encodable) => {
                let dependencies = dependencies_by_name
                    .get(&name)
                    .expect("every collected name was scanned for its dependencies");

                match unencodable_dependencies(
                    dependencies,
                    ambiguous,
                    selected_declarations,
                    excluded_file_ids,
                ) {
                    Some(reason) => {
                        newly_rejected.insert(name, reason);
                    }
                    None => {
                        encodables.insert(name, encodable);
                    }
                }
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

        rejected.extend(std::mem::take(&mut newly_rejected));

        for (name, encodable) in &encodables {
            // These are struct *type* names, not member names, because
            // reporting them as members would name the wrong thing in the
            // error.
            let unusable_dependencies = encodable
                .direct_struct_deps
                .iter()
                .filter(|dependency| !encodables.contains_key(&dependency.name))
                .map(|dependency| dependency.name.clone())
                .collect::<Vec<_>>();

            if !unusable_dependencies.is_empty() {
                newly_rejected.insert(
                    name.clone(),
                    RejectReason::UnusableDependencies {
                        dependencies: unusable_dependencies,
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
    use slang_solidity_v2::{
        compilation::{CompilationBuilder, CompilationBuilderConfig},
        utils::LanguageVersion,
    };

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

    /// Builds a compilation unit from in-memory sources (the first entry is
    /// the root) and collects EIP-712 types from every file but `left_out`.
    fn collect_without(sources: &[(&str, &str)], left_out: &[&str]) -> Eip712TypeCollection {
        let (root, _) = sources.first().expect("at least one source");
        let sources = sources
            .iter()
            .map(|(id, src)| ((*id).to_string(), (*src).to_string()))
            .collect();

        let mut builder =
            CompilationBuilder::create(LanguageVersion::LATEST, InMemorySources { sources });

        builder.add_file((*root).to_string());

        let unit = builder.build();
        let file_ids = unit.file_ids();
        let collected_from: HashSet<&str> = file_ids
            .iter()
            .map(String::as_str)
            .filter(|file_id| !left_out.contains(file_id))
            .collect();
        let left_out: HashSet<&str> = left_out.iter().copied().collect();

        UnitStructs::from_unit(&unit).types_for(root, &collected_from, &left_out)
    }

    /// Builds a compilation unit from in-memory sources (the first entry is the
    /// root) and collects EIP-712 types from it.
    fn collect(sources: &[(&str, &str)]) -> Eip712TypeCollection {
        collect_without(sources, &[])
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

    /// Slang cannot constant-fold a length behind a member access, and
    /// reports it as 0. Encoding that as `uint256[0]` would return a hash for
    /// a type the source never declared, so the struct is rejected instead.
    #[test]
    fn fixed_size_array_with_unevaluable_length_is_not_encodable() {
        let collection = collect_one(
            "library Limits { uint256 internal constant MAX = 4; }
             struct Batch { uint256[Limits.MAX] amounts; }",
        );

        let error = collection
            .get("Batch")
            .expect_err("an unevaluable array length cannot be encoded");
        assert!(error.to_string().contains("amounts"), "{error}");
    }

    /// A type behind an import that did not resolve is not the same as a type
    /// EIP-712 cannot encode, and the error must name the declared type so the
    /// reader can find the missing import.
    #[test]
    fn unresolved_member_type_names_what_was_declared() {
        let collection = collect_one("struct Order { Coupon coupon; }");

        let error = collection
            .get("Order")
            .expect_err("an unresolved member type cannot be encoded");
        let message = error.to_string();
        assert!(message.contains("unresolved type"), "{message}");
        assert!(message.contains("Coupon coupon"), "{message}");
    }

    #[test]
    fn fixed_size_array_with_literal_length_is_encodable() {
        let collection = collect_one("struct Batch { uint256[3] amounts; }");

        assert_eq!(
            get_canonical_type(&collection, "Batch"),
            "Batch(uint256[3] amounts)"
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
        // `Outer`'s own members are all encodable. It falls only because
        // `Inner` does, so it is reported as an unusable dependency rather
        // than as a non-encodable member.
        let outer = collection.get("Outer").unwrap_err();
        assert!(
            matches!(
                &outer,
                Eip712CollectionLookupError::Rejected(
                    Eip712TypeRejected { reason: RejectReason::UnusableDependencies { dependencies }, .. } ,
                ) if dependencies.iter().any(|dependency| dependency == "Inner")
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
            Err(Eip712CollectionLookupError::Rejected(Eip712TypeRejected { name, reason: RejectReason::Duplicate { .. } })) if name == "S"
        ));
        // An unrelated struct is still usable.
        assert_eq!(get_canonical_type(&collection, "Ok"), "Ok(uint256 c)");
    }

    /// The suite's own source is the one asking, so its definition wins over a
    /// same-named struct anywhere in its import graph — otherwise a collision
    /// in a dependency (`Permit`, `Order`, `Call`) would make the test's own
    /// struct unusable.
    #[test]
    fn root_definition_wins_over_a_conflicting_import() {
        let collection = collect(&[
            (
                "root.sol",
                "import \"other.sol\";
                 struct S { uint256 a; }",
            ),
            ("other.sol", "struct S { address b; }"),
        ]);
        assert_eq!(get_canonical_type(&collection, "S"), "S(uint256 a)");
    }

    /// With no definition in the root source to prefer, conflicting imported
    /// definitions leave the name genuinely ambiguous.
    #[test]
    fn conflicting_imports_without_a_root_definition_are_rejected() {
        let collection = collect(&[
            (
                "root.sol",
                "import \"a.sol\";
                 import \"b.sol\";",
            ),
            ("a.sol", "struct S { uint256 a; }"),
            ("b.sol", "struct S { address b; }"),
        ]);
        assert!(matches!(
            collection.get("S"),
            Err(Eip712CollectionLookupError::Rejected(Eip712TypeRejected { name, reason: RejectReason::Duplicate { .. } })) if name == "S"
        ));
    }

    #[test]
    fn leaving_a_file_out_settles_a_clash_it_caused() {
        let collection = collect_without(
            &[
                (
                    "root.sol",
                    "import \"a.sol\";
                     import {S as Vendored} from \"b.sol\";
                     struct Uses { S s; }",
                ),
                ("a.sol", "struct S { uint256 a; }"),
                ("b.sol", "struct S { address b; }"),
            ],
            &["b.sol"],
        );

        assert_eq!(get_canonical_type(&collection, "S"), "S(uint256 a)");
        assert_eq!(
            get_canonical_type(&collection, "Uses"),
            "Uses(S s)S(uint256 a)"
        );
    }

    #[test]
    fn a_dependency_only_a_left_out_file_declares_makes_its_dependents_unusable() {
        let collection = collect_without(
            &[
                (
                    "root.sol",
                    "import \"dep.sol\";
                     struct Wrapper { PermitSingle p; }",
                ),
                (
                    "dep.sol",
                    "struct PermitSingle { address token; uint160 amount; }",
                ),
            ],
            &["dep.sol"],
        );

        assert!(matches!(
            collection.get("PermitSingle"),
            Err(Eip712CollectionLookupError::Rejected(Eip712TypeRejected {
                reason: RejectReason::DeclaredInExcludedFiles { file_ids },
                ..
            })) if file_ids == vec!["dep.sol".to_owned()]
        ));
        assert!(matches!(
            collection.get("Wrapper"),
            Err(Eip712CollectionLookupError::Rejected(Eip712TypeRejected {
                reason: RejectReason::UnusableDependencies { dependencies },
                ..
            })) if dependencies == vec!["PermitSingle".to_owned()]
        ));
    }

    /// The root still scopes the query when it is left out, so an imported
    /// name it also declares is resolved from the import rather than from it.
    #[test]
    fn leaving_the_root_out_drops_its_structs_and_its_say_in_a_clash() {
        let collection = collect_without(
            &[
                (
                    "root.sol",
                    "import {S as Vendored} from \"dep.sol\";
                     struct S { uint256 a; }
                     struct Own { uint256 b; }",
                ),
                ("dep.sol", "struct S { address c; }"),
            ],
            &["root.sol"],
        );

        assert_eq!(get_canonical_type(&collection, "S"), "S(address c)");
        assert!(matches!(
            collection.get("Own"),
            Err(Eip712CollectionLookupError::Rejected(Eip712TypeRejected {
                reason: RejectReason::DeclaredInExcludedFiles { file_ids },
                ..
            })) if file_ids == vec!["root.sol".to_owned()]
        ));
    }

    /// A dependency must resolve to the struct it actually references, not to
    /// whichever same-named struct wins the top-level lookup. Inlining the
    /// wrong one yields a `typeHash` that does not match the Solidity type.
    #[test]
    fn imported_dependency_keeps_its_own_definition() {
        let collection = collect(&[
            (
                "root.sol",
                "import {Order} from \"dep.sol\";
                 struct Point { uint256 x; }",
            ),
            (
                "dep.sol",
                "struct Point { address a; }
                 struct Order { Point p; }",
            ),
        ]);

        // `Order.p` is `dep.sol`'s `Point`, so it must not be encoded with
        // `root.sol`'s.
        let order = collection.get("Order");
        assert!(
            !matches!(&order, Ok(ty) if ty.canonical_definition().contains("Point(uint256 x)")),
            "Order inlined the wrong Point: {order:?}"
        );
    }

    /// Root-preference settles a direct lookup, but it must not leak into
    /// dependency resolution. A struct referencing the contested name could be
    /// encoded with either body.
    #[test]
    fn root_preference_does_not_extend_to_dependents() {
        let collection = collect(&[
            (
                "root.sol",
                "import {Order} from \"dep.sol\";
                 struct Point { uint256 x; }
                 struct RootUses { Point p; }",
            ),
            (
                "dep.sol",
                "struct Point { address a; }
                 struct Order { Point p; }",
            ),
        ]);

        // The direct lookup still resolves to the asking source's definition.
        assert_eq!(get_canonical_type(&collection, "Point"), "Point(uint256 x)");

        // Neither dependent is encodable because both would inline a `Point`
        // chosen by name alone.
        for name in ["Order", "RootUses"] {
            let error = collection.get(name).unwrap_err();
            assert!(
                matches!(&error, Eip712CollectionLookupError::Rejected(Eip712TypeRejected { reason: RejectReason::AmbiguousDependencies { dependencies }, .. }) if dependencies.iter().any(|dependency| dependency == "Point")),
                "{name}: {error}"
            );
        }
    }

    #[test]
    fn dependent_of_conflicting_struct_is_rejected() {
        let collection = collect_one(
            "struct S { uint256 a; }
             contract C { struct S { uint256 b; } }
             struct Uses { S s; }",
        );
        // `Uses` is well-formed in isolation. It is rejected because `S`
        // names two different structs, so there is no saying which body to
        // inline. The report names the struct type rather than the member
        // holding it.
        let uses = collection.get("Uses").unwrap_err();
        assert!(
            matches!(&uses, Eip712CollectionLookupError::Rejected(Eip712TypeRejected { reason: RejectReason::AmbiguousDependencies { dependencies }, .. }) if dependencies.iter().any(|dependency| dependency == "S")),
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

    /// A member resolving to an excluded file's struct must not be encoded
    /// with the body of a same-named struct that *is* collected: the two hash
    /// differently.
    #[test]
    fn excluded_dependency_is_not_replaced_by_a_collected_namesake() {
        let collection = collect_without(
            &[
                (
                    "root.sol",
                    "import \"a.sol\";
                     import \"c.sol\";",
                ),
                ("a.sol", "struct S { uint256 a; }"),
                ("b.sol", "struct S { address b; }"),
                (
                    "c.sol",
                    "import {S} from \"b.sol\";
                     struct Wrapper { S s; }",
                ),
            ],
            &["b.sol"],
        );

        assert_eq!(get_canonical_type(&collection, "S"), "S(uint256 a)");
        assert!(
            matches!(
                collection.get("Wrapper"),
                Err(Eip712CollectionLookupError::Rejected(Eip712TypeRejected {
                    reason: RejectReason::DependenciesInExcludedFiles { dependencies },
                    ..
                })) if dependencies == vec!["`S` in b.sol".to_owned()]
            ),
            "{:?}",
            collection.get("Wrapper")
        );
    }

    /// A member whose type merely shares the struct's name is a dependency,
    /// not a self-reference. Treating it as one would encode `T(T inner)` and
    /// drop the referenced body altogether.
    #[test]
    fn same_named_dependency_from_another_file_is_no_self_reference() {
        let collection = collect_without(
            &[
                ("root.sol", "import \"d.sol\";"),
                (
                    "d.sol",
                    "import {T as OtherT} from \"e.sol\";
                     struct T { OtherT inner; }",
                ),
                ("e.sol", "struct T { uint256 t; }"),
            ],
            &["e.sol"],
        );

        assert!(
            matches!(
                collection.get("T"),
                Err(Eip712CollectionLookupError::Rejected(Eip712TypeRejected {
                    reason: RejectReason::DependenciesInExcludedFiles { dependencies },
                    ..
                })) if dependencies == vec!["`T` in e.sol".to_owned()]
            ),
            "{:?}",
            collection.get("T")
        );
    }

    /// Deduplication keeps one declaration of a name, so an excluded
    /// dependency of any of them must reject the name. Otherwise the verdict
    /// would turn on which file id happens to sort first.
    #[test]
    fn an_excluded_dependency_of_any_declaration_rejects_the_name() {
        const COLLECTED_DEPENDENCY: &str = "import {S} from \"a.sol\";
                                            struct W { S s; }";
        const EXCLUDED_DEPENDENCY: &str = "import {S} from \"b.sol\";
                                           struct W { S s; }";

        for (c_source, d_source) in [
            (COLLECTED_DEPENDENCY, EXCLUDED_DEPENDENCY),
            (EXCLUDED_DEPENDENCY, COLLECTED_DEPENDENCY),
        ] {
            let collection = collect_without(
                &[
                    (
                        "root.sol",
                        "import \"c.sol\";
                         import \"d.sol\";",
                    ),
                    ("a.sol", "struct S { uint256 a; }"),
                    ("b.sol", "struct S { address b; }"),
                    ("c.sol", c_source),
                    ("d.sol", d_source),
                ],
                &["b.sol"],
            );

            assert_eq!(get_canonical_type(&collection, "S"), "S(uint256 a)");
            assert!(
                matches!(
                    collection.get("W"),
                    Err(Eip712CollectionLookupError::Rejected(Eip712TypeRejected {
                        reason: RejectReason::DependenciesInExcludedFiles { dependencies },
                        ..
                    })) if dependencies == vec!["`S` in b.sol".to_owned()]
                ),
                "{:?}",
                collection.get("W")
            );
        }
    }

    /// Two declarations of a name whose members resolve to declarations that
    /// are themselves identical still encode: every dependency identity is one
    /// the collection selected for that name.
    #[test]
    fn declarations_resolving_to_identical_dependencies_still_collapse() {
        let collection = collect(&[
            (
                "root.sol",
                "import \"c.sol\";
                 import \"d.sol\";",
            ),
            ("a.sol", "struct S { uint256 a; }"),
            ("b.sol", "struct S { uint256 a; }"),
            (
                "c.sol",
                "import {S} from \"a.sol\";
                 struct W { S s; }",
            ),
            (
                "d.sol",
                "import {S} from \"b.sol\";
                 struct W { S s; }",
            ),
        ]);

        assert_eq!(get_canonical_type(&collection, "S"), "S(uint256 a)");
        assert_eq!(get_canonical_type(&collection, "W"), "W(S s)S(uint256 a)");
    }

    #[test]
    fn unknown_type_is_not_found() {
        let collection = collect_one("struct S { uint256 a; }");
        assert!(matches!(
            collection.get("DoesNotExist"),
            Err(Eip712CollectionLookupError::NotFound { type_name }) if type_name == "DoesNotExist"
        ));
    }
}
