// Static hardfork metadata for `@nomicfoundation/edr`.
//
// MANUALLY MAINTAINED. When adding or changing a hardfork, update this file and
// keep the display names in sync with the `define_hardforks!` converters in
// src/chains/l1.rs and src/chains/op.rs. (A future change may auto-generate this
// file from the Rust source of truth.)
//
// Self-contained: imports nothing, so reading these values does NOT load the
// native addon.

/* eslint-disable @typescript-eslint/naming-convention */

// Shared helpers, generic over a chain's hardfork enum (`Hardfork`), so the L1
// and OP sections below don't duplicate the conversion logic.

/** Builds the name→hardfork inverse of a hardfork→name map (derived once). */
function invertHardforkMap<Hardfork extends number>(
  toName: Record<Hardfork, string>
): Record<string, Hardfork> {
  return Object.fromEntries(
    Object.entries(toName).map(([hardfork, name]) => [
      name,
      Number(hardfork) as Hardfork,
    ])
  );
}

/** Looks up a hardfork by name, throwing if it is not supported. */
function hardforkFromName<Hardfork extends number>(
  byName: Record<string, Hardfork>,
  name: string
): Hardfork {
  const hardfork = byName[name];
  if (hardfork === undefined) {
    throw new Error(`The provided hardfork \`${name}\` is not supported.`);
  }
  return hardfork;
}

export enum SpecId {
  Frontier = 0,
  FrontierThawing = 1,
  Homestead = 2,
  DaoFork = 3,
  Tangerine = 4,
  SpuriousDragon = 5,
  Byzantium = 6,
  Constantinople = 7,
  Petersburg = 8,
  Istanbul = 9,
  MuirGlacier = 10,
  Berlin = 11,
  London = 12,
  ArrowGlacier = 13,
  GrayGlacier = 14,
  Merge = 15,
  Shanghai = 16,
  Cancun = 17,
  Prague = 18,
  Osaka = 19,
}

export const FRONTIER: string = "Frontier";
export const FRONTIER_THAWING: string = "Frontier Thawing";
export const HOMESTEAD: string = "Homestead";
export const DAO_FORK: string = "DAO Fork";
export const TANGERINE: string = "Tangerine";
export const SPURIOUS_DRAGON: string = "Spurious";
export const BYZANTIUM: string = "Byzantium";
export const CONSTANTINOPLE: string = "Constantinople";
export const PETERSBURG: string = "Petersburg";
export const ISTANBUL: string = "Istanbul";
export const MUIR_GLACIER: string = "MuirGlacier";
export const BERLIN: string = "Berlin";
export const LONDON: string = "London";
export const ARROW_GLACIER: string = "Arrow Glacier";
export const GRAY_GLACIER: string = "Gray Glacier";
export const MERGE: string = "Merge";
export const SHANGHAI: string = "Shanghai";
export const CANCUN: string = "Cancun";
export const PRAGUE: string = "Prague";
export const OSAKA: string = "Osaka";

export const L1_HARDFORK_LATEST: SpecId = SpecId.Osaka;

const L1_HARDFORK_TO_NAME: Record<SpecId, string> = {
  [SpecId.Frontier]: FRONTIER,
  [SpecId.FrontierThawing]: FRONTIER_THAWING,
  [SpecId.Homestead]: HOMESTEAD,
  [SpecId.DaoFork]: DAO_FORK,
  [SpecId.Tangerine]: TANGERINE,
  [SpecId.SpuriousDragon]: SPURIOUS_DRAGON,
  [SpecId.Byzantium]: BYZANTIUM,
  [SpecId.Constantinople]: CONSTANTINOPLE,
  [SpecId.Petersburg]: PETERSBURG,
  [SpecId.Istanbul]: ISTANBUL,
  [SpecId.MuirGlacier]: MUIR_GLACIER,
  [SpecId.Berlin]: BERLIN,
  [SpecId.London]: LONDON,
  [SpecId.ArrowGlacier]: ARROW_GLACIER,
  [SpecId.GrayGlacier]: GRAY_GLACIER,
  [SpecId.Merge]: MERGE,
  [SpecId.Shanghai]: SHANGHAI,
  [SpecId.Cancun]: CANCUN,
  [SpecId.Prague]: PRAGUE,
  [SpecId.Osaka]: OSAKA,
};

const L1_HARDFORK_BY_NAME = invertHardforkMap(L1_HARDFORK_TO_NAME);

export function l1HardforkToString(hardfork: SpecId): string {
  return L1_HARDFORK_TO_NAME[hardfork];
}

export function l1HardforkFromString(name: string): SpecId {
  return hardforkFromName(L1_HARDFORK_BY_NAME, name);
}

/**
 * @deprecated Use the {@link L1_HARDFORK_LATEST} constant instead.
 */
export function l1HardforkLatest(): SpecId {
  return L1_HARDFORK_LATEST;
}

export enum OpHardfork {
  Bedrock = 100,
  Regolith = 101,
  Canyon = 102,
  Ecotone = 103,
  Fjord = 104,
  Granite = 105,
  Holocene = 106,
  Isthmus = 107,
}

export const BEDROCK: string = "Bedrock";
export const REGOLITH: string = "Regolith";
export const CANYON: string = "Canyon";
export const ECOTONE: string = "Ecotone";
export const FJORD: string = "Fjord";
export const GRANITE: string = "Granite";
export const HOLOCENE: string = "Holocene";
export const ISTHMUS: string = "Isthmus";

export const OP_HARDFORK_LATEST: OpHardfork = OpHardfork.Isthmus;

const OP_HARDFORK_TO_NAME: Record<OpHardfork, string> = {
  [OpHardfork.Bedrock]: BEDROCK,
  [OpHardfork.Regolith]: REGOLITH,
  [OpHardfork.Canyon]: CANYON,
  [OpHardfork.Ecotone]: ECOTONE,
  [OpHardfork.Fjord]: FJORD,
  [OpHardfork.Granite]: GRANITE,
  [OpHardfork.Holocene]: HOLOCENE,
  [OpHardfork.Isthmus]: ISTHMUS,
};

const OP_HARDFORK_BY_NAME = invertHardforkMap(OP_HARDFORK_TO_NAME);

export function opHardforkToString(hardfork: OpHardfork): string {
  return OP_HARDFORK_TO_NAME[hardfork];
}

export function opHardforkFromString(name: string): OpHardfork {
  return hardforkFromName(OP_HARDFORK_BY_NAME, name);
}

/**
 * @deprecated Use the {@link OP_HARDFORK_LATEST} constant instead.
 */
export function opLatestHardfork(): OpHardfork {
  return OP_HARDFORK_LATEST;
}
