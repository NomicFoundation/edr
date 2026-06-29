// Public entry point for `@nomicfoundation/edr`.
//
// Re-exports the native addon bindings together with the addon-free hardfork
// metadata, so the hardfork enums/constants/conversions are importable from
// either `@nomicfoundation/edr` or `@nomicfoundation/edr/hardforks` and are the
// same types in both. `#native` resolves to the napi-generated loader via the
// package's `imports` map.

import {
  l1HardforkToString,
  opHardforkToString,
  type OpHardfork,
  type SpecId,
} from "./hardforks.js";
import {
  l1GenesisState as l1GenesisStateNative,
  opGenesisState as opGenesisStateNative,
  type AccountOverride,
} from "#native";

export * from "#native";
export * from "./hardforks.js";

// The two functions below re-export the native genesis builders with their
// original enum-typed signatures.
//
// The native bindings receive the hardfork as a name `string` (the consistent
// hardfork representation across the addon's API), so the numeric enum values
// never cross the FFI boundary. The `SpecId`/`OpHardfork` enums live in the
// addon-free `hardforks` module, so these wrappers convert the enum to its name
// before calling the native function. This preserves the original enum-typed
// public API for backwards compatibility while keeping the addon boundary
// string-based. A local export takes precedence over the same name re-exported
// by `export * from "#native"`, so consumers get these typed versions.

/**
 * Returns the genesis state for the given L1 hardfork. Re-typed to accept
 * `SpecId` (see the note above); converts it to a name for the native call.
 */
export function l1GenesisState(hardfork: SpecId): AccountOverride[] {
  return l1GenesisStateNative(l1HardforkToString(hardfork));
}

/**
 * Returns the genesis state for the given OP hardfork. Re-typed to accept
 * `OpHardfork` (see the note above); converts it to a name for the native call.
 */
export function opGenesisState(hardfork: OpHardfork): AccountOverride[] {
  return opGenesisStateNative(opHardforkToString(hardfork));
}
