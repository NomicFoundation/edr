# Style Guide

This is a style guide for the EDR project.

## Procedural derive macros

When deriving multiple traits, use the following rules to order them:

1. Standard library traits before external crates
2. Supertraits before subtraits
3. Alphabetical order

For example:

```rust
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
```

Or, for super- and subtraits:

```rust
#[derive(PartialEq, Eq, PartialOrd, Ord)]
```

## Member ordering

When adding a variant to an `enum` or a field to a `struct` or enum variant, by default follow alphabetical order. If it makes more sense to follow custom ordering, feel free to do so.

### Member functions

For member functions, use the following default rules to order them:

1. Public members before private members
2. Alphabetical order

Again, if it makes more sense to follow custom ordering, feel free to do so.

### Example

```rust
struct Foo {
    bar: u32,
    baz: u32,
}

impl Foo {
    pub fn bar_mut(&mut self) -> &mut u32 {
        &mut self.bar
    }

    pub fn baz(&self) -> u32 {
        self.baz
    }

    fn bar(&self) -> u32 {
        self.bar
    }
}
```

## Use `#[napi(catch_unwind)]`

Annotate every function/method exposed from `edr_napi` to JS with the `catch_unwind` NAPI-RS macro attribute.

### Example

```rust
#[napi(catch_unwind)]
pub fn foo() {
  // The panic is turned into an error thrown on the JS side
  panic!("panic message")
}
```

### Rationale

Rust functions that panic on the Node.js main thread will not return a result and crash the Node.js process.

Annotating with the `catch_unwind` macro attribute will turn the panic into a `napi::Error`, which can then be returned as a `napi::Result`.

Note that this will not capture panics in background threads.

## Traits

### Defining traits

Keep traits composable: give a trait only the generic types, associated types and functions that a single use case needs.

Avoid overconstraining a trait. Require only:

- supertraits whose items the trait's own definition names
- type bounds that the definition needs, or that every usage site needs anyway

For supertraits, prefer:

```rust
/// Trait for specifying the contextual information type of a chain.
pub trait ContextChainSpec {
    type Context;
}

/// Trait for specifying the hardfork type of a chain.
pub trait HardforkChainSpec {
    type Hardfork: Clone + Default + Into<EvmSpecId>;
}

/// Trait for specifying the block environment type of a chain.
pub trait BlockEnvChainSpec: HardforkChainSpec {
    // `HardforkChainSpec` is required, because this bound names
    // `Self::Hardfork`.
    type BlockEnv: BlockEnvConstructor<Self::Hardfork>;
}

pub trait FullChainSpec: BlockEnvChainSpec + ContextChainSpec {}

// The blanket implementation makes the trait an alias for its supertraits.
impl<ChainSpecT: BlockEnvChainSpec + ContextChainSpec + ?Sized> FullChainSpec for ChainSpecT {}
```

over:

```rust
/// Trait for specifying EVM types of a chain.
pub trait EvmChainSpec: ContextChainSpec {
    // Nothing in this definition names `ContextChainSpec`, so implementers are
    // forced to implement it for nothing. Implementers that only need a
    // hardfork are forced to provide a block environment too.
    type Hardfork: Clone + Default + Into<EvmSpecId>;
    type BlockEnv: BlockEnvConstructor<Self::Hardfork>;
}
```

For type bounds, prefer:

```rust
pub trait HardforkChainSpec {
    // revm asks for `Clone + Into<EvmSpecId>` and every chain needs a default
    // hardfork. Every usage site requires these.
    type Hardfork: Clone + Default + Into<EvmSpecId>;
}
```

over:

```rust
pub trait HardforkChainSpec {
    // `Debug` and `Send + Sync` are only required by some usage sites, so they
    // belong on the bounds of those usage sites instead.
    type Hardfork: Clone + Debug + Default + Into<EvmSpecId> + Send + Sync;
}
```

When unsure whether a bound is always required, leave it out. A needless bound goes unnoticed, whereas a missing one surfaces as a compile error at the first usage site that needs it—which is also where it belongs. Likewise, when a bound exists only to satisfy a downstream consumer, require exactly what that consumer asks for and no more; e.g. strengthening `Clone` to `Copy` above would constrain every chain for our own convenience.

#### Associated types vs. generic type parameters

Before splitting a trait, decide which of its types belong in its signature at all:

- use an **associated type** when each implementer has exactly one choice; e.g. a chain has exactly one hardfork type
- use a **generic type parameter** when a single type may implement the trait for many choices; e.g. `Into<EvmSpecId>`

Using a generic type parameter where an associated type belongs forces every usage site to name—and therefore bound—a type it does not care about. This is a common source of the bound explosion these rules aim to avoid.

#### Umbrella traits

`FullChainSpec` above is an _umbrella trait_: it has no members of its own and exists only to name a combination of other traits.

The blanket implementation is what makes it an alias; without it, every implementer would have to write an empty `impl`. Note the `?Sized`, without which the implicit `Sized` bound excludes unsized implementers for no reason. Adding a required member to the trait breaks the blanket implementation—and with it every implementer—so only use blanket implementations for umbrella traits.

Another downside is worse diagnostics: a type that is missing only `HardforkChainSpec` is reported as not implementing `FullChainSpec`. Consider `#[diagnostic::on_unimplemented]` when the combination is widely used:

```rust
#[diagnostic::on_unimplemented(
    message = "The type `{Self}` does not implement `FullChainSpec`. It might be missing one of its supertraits: `BlockEnvChainSpec` or `ContextChainSpec`."
)]
trait FullChainSpec: BlockEnvChainSpec + ContextChainSpec {}

impl<ChainSpecT: BlockEnvChainSpec + ContextChainSpec + ?Sized> FullChainSpec for ChainSpecT {}
```

### Using trait bounds

Require only the bounds that the code actually uses. Prefer:

```rust
fn foo<HardforkT: Into<EvmSpecId>>(hardfork: HardforkT) {
    let _spec_id: EvmSpecId = hardfork.into();

    // More code that only requires the `Into<EvmSpecId>` bound
}
```

over:

```rust
pub trait Hardfork: Clone + Default + Into<EvmSpecId> {}

fn foo<HardforkT: Hardfork>(hardfork: HardforkT) {
    let _spec_id: EvmSpecId = hardfork.into();

    // More code that only requires the `Into<EvmSpecId>` bound
}
```

By default, parameterize a type over the individual types it uses, rather than over a trait that supplies them as associated types. Add a chain-specific type alias for convenience. Prefer:

```rust
pub trait ContextChainSpec {
    type Context;
}

pub trait HardforkChainSpec {
    type Hardfork: Clone + Default + Into<EvmSpecId>;
}

/// Type alias for a chain-specific [`Foo`].
pub type FooForChainSpec<ChainSpecT> = Foo<
    <ChainSpecT as ContextChainSpec>::Context,
    <ChainSpecT as HardforkChainSpec>::Hardfork,
>;

pub struct Foo<ContextT, HardforkT> {
    context: ContextT,
    hardfork: HardforkT,
    other_field: u32,
}
```

over:

```rust
pub trait FooChainSpec: ContextChainSpec + HardforkChainSpec {}

impl<ChainSpecT: ContextChainSpec + HardforkChainSpec + ?Sized> FooChainSpec for ChainSpecT {}

pub struct Foo<ChainSpecT: FooChainSpec> {
    context: ChainSpecT::Context,
    hardfork: ChainSpecT::Hardfork,
    other_field: u32,
}
```

Decomposition is not always the least constraining option, so this is a default rather than a rule. When a member needs the chain spec itself—to call one of its associated functions—the type ends up carrying both the decomposed parameters and the chain spec, which is worse than either alone. Keep the chain spec parameter in that case.

### Rationale

Every constraint has to be paid for somewhere. An implementer has to provide everything a trait requires, and a usage site has to satisfy every bound it names. Requiring less leaves that cost with the code that actually needs it. That is what keeps traits, functions and types reusable and composable.

A bound can also drag in requirements you never wrote, including requirements about lifetimes. We have seen an unrelated trait bound force one type to outlive another. The resulting error blamed a lifetime that was in fact valid. Layers of associated types make this more likely, and EDR's chain spec types are full of them.

The cost is maintainability. As use cases with fewer constraints arise, a trait may need to be split up further. Weigh that on a case-by-case basis.
