# Style Guide

This is a style guide for the EDR project.

## Procedural derive macros

When deriving multiple traits, use the following rules to order them:

1. Standard library traits before external crates
2. Sub-traits before super-traits
3. Alphabetical order

For example:

```rust
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
```

Or, for sub- and super-traits:

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

## Defining traits

When defining traits, try to keep traits composable by only defining the generic types, associated types and/or functions needed for a specific use case.

Furthermore, avoid overconstraining traits by only requiring:

- sub-traits that the trait's definition requires
- type bounds that are required for the trait's definition OR are always required at usage sites

For sub-traits, e.g. prefer:

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

// Blanket implementation of `FullChainSpec` for any type that implements `BlockEnvChainSpec` and `ContextChainSpec`, making it an alias.
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

For type bounds, e.g. prefer:

```rust
pub trait HardforkChainSpec {
    // revm asks for `Clone + Into<EvmSpecId>` of a spec type and every chain
    // needs a default hardfork, so every usage site requires these.
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

When unsure whether a bound is always required, leave it out. An unnecessary bound is invisible, whereas a missing one surfaces as a compile error at the first usage site that needs it—which is also where it belongs. Likewise, when a bound exists only to satisfy a downstream consumer, require exactly what that consumer asks for and no more; e.g. strengthening `Clone` to `Copy` above would constrain every chain for our own convenience.

### Associated types vs. generic type parameters

Before deciding how to split a trait, decide which of its types belong in the trait's signature at all:

- use an **associated type** when each implementer has exactly one choice; e.g. a chain has exactly one hardfork type
- use a **generic type parameter** when a single type may implement the trait for many choices; e.g. `Into<EvmSpecId>`

Using a generic type parameter where an associated type belongs forces every usage site to name — and therefore bound — a type it does not care about. This is a common source of the bound explosion that the rest of this section aims to avoid.

### Umbrella traits

`FullChainSpec` above is an _umbrella trait_: it has no members of its own and exists only to name a combination of other traits.

The blanket implementation is what makes it an alias; without it, every implementer would have to write an empty `impl`. Note the `?Sized`, without which the implicit `Sized` bound excludes unsized implementers for no reason. Adding a required member to the trait breaks the blanket implementation—and with it every implementer—so only use blanket implementations for umbrella traits.

A downside of umbrella traits is that they can make diagnostics worse. E.g. a type that is missing only `HardforkChainSpec` is reported as not implementing `FullChainSpec`. Consider `#[diagnostic::on_unimplemented]` when the combination is widely used. E.g.:

```rust
#[diagnostic::on_unimplemented(
    message = "The type `{Self}` does not implement `FullChainSpec`. It might be missing one of its sub-traits: `BlockEnvChainSpec` or `ContextChainSpec`."
)]
trait FullChainSpec: BlockEnvChainSpec + ContextChainSpec {}

impl<ChainSpecT: BlockEnvChainSpec + ContextChainSpec + ?Sized> FullChainSpec for ChainSpecT {}
```

### Rationale

When a user is implementing a trait for their type or requiring a trait bound on a generic type, they will have to implement or satisfy all the constraints of the trait. By splitting traits up into smaller traits, you can reduce the number of constraints that need to be satisfied at usage sites. This guarantees that the trait is maximally reusable and composable.

Moreover, this limits the number of obligations the trait solver has to discharge and—because well-formedness obligations feed region inference—the number of lifetime constraints that come with them. Historically, we have run into problems where an unrelated trait bound resulted in an `outlives` requirement and the error blamed a lifetime that was in fact valid.

This does mean that over time a trait may need to be split up into smaller traits, as new use cases with less constraints arise. This is a trade-off between reusability and maintainability that needs to be considered on a case-by-case basis.

## Using trait bounds

When using trait bounds, use the least constraints possible to satisfy the requirements of the function. E.g. prefer:

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

Moreover, by default parameterize a type over the individual types that it uses, rather than over a trait that provides them as associated types. You can add a chain-specific type alias for convenience. E.g. prefer:

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
pub trait FullChainSpec: ContextChainSpec + HardforkChainSpec {}

impl<ChainSpecT: ContextChainSpec + HardforkChainSpec + ?Sized> FullChainSpec for ChainSpecT {}

pub struct Foo<ChainSpecT: FullChainSpec> {
    context: ChainSpecT::Context,
    hardfork: ChainSpecT::Hardfork,
    other_field: u32,
}
```

This is a default rather than a rule, as decomposition is not always the least constraining option. E.g. when a member needs the chain spec itself—to call one of its associated functions—the type ends up carrying both the decomposed parameters and the chain spec, which is worse than either alone. In cases like that, it is acceptable to keep the chain spec parameter instead.

### Rationale

When a user is calling a function or using a type with trait bounds, they will have to satisfy all the constraints of the trait bounds. By using the least constraining option, you can reduce the number of constraints that need to be satisfied at usage sites; thus making the function more reusable and composable.

Moreover, this limits the number of obligations the trait solver has to discharge and—because well-formedness obligations feed region inference—the number of lifetime constraints that come with them. Historically, we have run into problems where an unrelated trait bound resulted in an `outlives` requirement and the error blamed a lifetime that was in fact valid. This has especially been a problem when using trait bounds with layers of associated types; commonly used in chain spec types in EDR.
