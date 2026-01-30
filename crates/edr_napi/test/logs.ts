import { toBytes } from "@nomicfoundation/ethereumjs-util";
import { assert } from "chai";
import chalk, { Chalk } from "chalk";
import {
  AccountOverride,
  ContractDecoder,
  // Ignore this on testNoBuild
  // @ts-ignore
  createProviderWithMockTimer,
  l1GenesisState,
  l1HardforkFromString,
  MineOrdering,
  // Ignore this on testNoBuild
  // @ts-ignore
  MockTime,
  Provider,
  SHANGHAI,
  SubscriptionEvent,
} from "..";
import {
  deployContract,
  getBlockNumber,
  getGasPrice,
  sendTransaction,
  sleep,
} from "./helpers";
import { ConsoleLogger } from "./helper/console-logger";

export const EXAMPLE_READ_CONTRACT = {
  sourceCode: `
pragma solidity ^0.8.0;

contract Example {
    event ReturnValue(uint value);

    function blockNumber() public view returns (uint) {
        return block.number;
    }
    function blockTimestamp() public view returns (uint) {
        return block.timestamp;
    }
    function blockGasLimit() public returns (uint) {
        emit ReturnValue(block.gaslimit);
        return block.gaslimit;
    }
    function gasLeft() public returns (uint) {
        uint gas = gasleft();
        emit ReturnValue(gas);
        return gas;
    }
    function senderBalance() public returns (uint balance) {
      balance = msg.sender.balance;
    }
}
`,
  bytecode: {
    linkReferences: {},
    object:
      "608060405234801561001057600080fd5b5061020e806100206000396000f3fe608060405234801561001057600080fd5b50600436106100575760003560e01c80632ddb301b1461005c57806357e871e71461007a5780637877a797146100985780638ce671ec146100b6578063adb61832146100d4575b600080fd5b6100646100f2565b60405161007191906101bd565b60405180910390f35b610082610136565b60405161008f91906101bd565b60405180910390f35b6100a061013e565b6040516100ad91906101bd565b60405180910390f35b6100be61017d565b6040516100cb91906101bd565b60405180910390f35b6100dc61019c565b6040516100e991906101bd565b60405180910390f35b6000805a90507f3a1575e395fa8386a814e103dd43d4a6a43479ce4e36cb661466fa47fe2e79968160405161012791906101bd565b60405180910390a18091505090565b600043905090565b60007f3a1575e395fa8386a814e103dd43d4a6a43479ce4e36cb661466fa47fe2e79964560405161016f91906101bd565b60405180910390a145905090565b60003373ffffffffffffffffffffffffffffffffffffffff1631905090565b600042905090565b6000819050919050565b6101b7816101a4565b82525050565b60006020820190506101d260008301846101ae565b9291505056fea2646970667358221220549245fc80d513d8f8cd835450c5373c29ddd7313034119e4a6d899a8e97444b64736f6c63430008090033",
    opcodes:
      "PUSH1 0x80 PUSH1 0x40 MSTORE CALLVALUE DUP1 ISZERO PUSH2 0x10 JUMPI PUSH1 0x0 DUP1 REVERT JUMPDEST POP PUSH2 0x20E DUP1 PUSH2 0x20 PUSH1 0x0 CODECOPY PUSH1 0x0 RETURN INVALID PUSH1 0x80 PUSH1 0x40 MSTORE CALLVALUE DUP1 ISZERO PUSH2 0x10 JUMPI PUSH1 0x0 DUP1 REVERT JUMPDEST POP PUSH1 0x4 CALLDATASIZE LT PUSH2 0x57 JUMPI PUSH1 0x0 CALLDATALOAD PUSH1 0xE0 SHR DUP1 PUSH4 0x2DDB301B EQ PUSH2 0x5C JUMPI DUP1 PUSH4 0x57E871E7 EQ PUSH2 0x7A JUMPI DUP1 PUSH4 0x7877A797 EQ PUSH2 0x98 JUMPI DUP1 PUSH4 0x8CE671EC EQ PUSH2 0xB6 JUMPI DUP1 PUSH4 0xADB61832 EQ PUSH2 0xD4 JUMPI JUMPDEST PUSH1 0x0 DUP1 REVERT JUMPDEST PUSH2 0x64 PUSH2 0xF2 JUMP JUMPDEST PUSH1 0x40 MLOAD PUSH2 0x71 SWAP2 SWAP1 PUSH2 0x1BD JUMP JUMPDEST PUSH1 0x40 MLOAD DUP1 SWAP2 SUB SWAP1 RETURN JUMPDEST PUSH2 0x82 PUSH2 0x136 JUMP JUMPDEST PUSH1 0x40 MLOAD PUSH2 0x8F SWAP2 SWAP1 PUSH2 0x1BD JUMP JUMPDEST PUSH1 0x40 MLOAD DUP1 SWAP2 SUB SWAP1 RETURN JUMPDEST PUSH2 0xA0 PUSH2 0x13E JUMP JUMPDEST PUSH1 0x40 MLOAD PUSH2 0xAD SWAP2 SWAP1 PUSH2 0x1BD JUMP JUMPDEST PUSH1 0x40 MLOAD DUP1 SWAP2 SUB SWAP1 RETURN JUMPDEST PUSH2 0xBE PUSH2 0x17D JUMP JUMPDEST PUSH1 0x40 MLOAD PUSH2 0xCB SWAP2 SWAP1 PUSH2 0x1BD JUMP JUMPDEST PUSH1 0x40 MLOAD DUP1 SWAP2 SUB SWAP1 RETURN JUMPDEST PUSH2 0xDC PUSH2 0x19C JUMP JUMPDEST PUSH1 0x40 MLOAD PUSH2 0xE9 SWAP2 SWAP1 PUSH2 0x1BD JUMP JUMPDEST PUSH1 0x40 MLOAD DUP1 SWAP2 SUB SWAP1 RETURN JUMPDEST PUSH1 0x0 DUP1 GAS SWAP1 POP PUSH32 0x3A1575E395FA8386A814E103DD43D4A6A43479CE4E36CB661466FA47FE2E7996 DUP2 PUSH1 0x40 MLOAD PUSH2 0x127 SWAP2 SWAP1 PUSH2 0x1BD JUMP JUMPDEST PUSH1 0x40 MLOAD DUP1 SWAP2 SUB SWAP1 LOG1 DUP1 SWAP2 POP POP SWAP1 JUMP JUMPDEST PUSH1 0x0 NUMBER SWAP1 POP SWAP1 JUMP JUMPDEST PUSH1 0x0 PUSH32 0x3A1575E395FA8386A814E103DD43D4A6A43479CE4E36CB661466FA47FE2E7996 GASLIMIT PUSH1 0x40 MLOAD PUSH2 0x16F SWAP2 SWAP1 PUSH2 0x1BD JUMP JUMPDEST PUSH1 0x40 MLOAD DUP1 SWAP2 SUB SWAP1 LOG1 GASLIMIT SWAP1 POP SWAP1 JUMP JUMPDEST PUSH1 0x0 CALLER PUSH20 0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF AND BALANCE SWAP1 POP SWAP1 JUMP JUMPDEST PUSH1 0x0 TIMESTAMP SWAP1 POP SWAP1 JUMP JUMPDEST PUSH1 0x0 DUP2 SWAP1 POP SWAP2 SWAP1 POP JUMP JUMPDEST PUSH2 0x1B7 DUP2 PUSH2 0x1A4 JUMP JUMPDEST DUP3 MSTORE POP POP JUMP JUMPDEST PUSH1 0x0 PUSH1 0x20 DUP3 ADD SWAP1 POP PUSH2 0x1D2 PUSH1 0x0 DUP4 ADD DUP5 PUSH2 0x1AE JUMP JUMPDEST SWAP3 SWAP2 POP POP JUMP INVALID LOG2 PUSH5 0x6970667358 0x22 SLT KECCAK256 SLOAD SWAP3 GASLIMIT 0xFC DUP1 0xD5 SGT 0xD8 0xF8 0xCD DUP4 SLOAD POP 0xC5 CALLDATACOPY EXTCODECOPY 0x29 0xDD 0xD7 BALANCE ADDRESS CALLVALUE GT SWAP15 0x4A PUSH14 0x899A8E97444B64736F6C63430008 MULMOD STOP CALLER ",
    sourceMap: "57:613:0:-:0;;;;;;;;;;;;;;;;;;;",
  },
  abi: [
    {
      anonymous: false,
      inputs: [
        {
          indexed: false,
          internalType: "uint256",
          name: "value",
          type: "uint256",
        },
      ],
      name: "ReturnValue",
      type: "event",
    },
    {
      inputs: [],
      name: "blockGasLimit",
      outputs: [{ internalType: "uint256", name: "", type: "uint256" }],
      stateMutability: "nonpayable",
      type: "function",
    },
    {
      inputs: [],
      name: "blockNumber",
      outputs: [{ internalType: "uint256", name: "", type: "uint256" }],
      stateMutability: "view",
      type: "function",
    },
    {
      inputs: [],
      name: "blockTimestamp",
      outputs: [{ internalType: "uint256", name: "", type: "uint256" }],
      stateMutability: "view",
      type: "function",
    },
    {
      inputs: [],
      name: "gasLeft",
      outputs: [{ internalType: "uint256", name: "", type: "uint256" }],
      stateMutability: "nonpayable",
      type: "function",
    },
    {
      inputs: [],
      name: "senderBalance",
      outputs: [
        {
          internalType: "uint256",
          name: "balance",
          type: "uint256",
        },
      ],
      stateMutability: "nonpayable",
      type: "function",
    },
  ],
  selectors: {
    blockGasLimit: "0x7877a797",
    blockNumber: "0x57e871e7",
    blockTimestamp: "0xadb61832",
    gasLeft: "0x2ddb301b",
    senderBalance: "0x8ce671ec",
  },
  topics: {},
};

class FakeModulesLogger {
  public lines: string[] = [];

  private _hasChanged = false;

  /// Checks if the logger has changed since the last call to this method.
  public hasChanged(): boolean {
    const hasChanged = this._hasChanged;
    this._hasChanged = false;
    return hasChanged;
  }

  public printLineFn(): (line: string) => void {
    return (line) => {
      this.lines.push(line);
      this._hasChanged = true;
    };
  }

  public replaceLastLineFn(): (line: string) => void {
    return (line) => {
      this.lines[this.lines.length - 1] = line;
      this._hasChanged = true;
    };
  }

  public getOutput(): string {
    return this.lines.join("\n");
  }

  public reset() {
    this._hasChanged = false;
    this.lines = [];
  }
}

function ansiColor(text: string, color: Chalk): string {
  const formatted = color(text);

  // EDR's ansi console crate uses the RESET code to reset the color
  return formatted.replaceAll("\x1B[39m", "\x1B[0m");
}

async function intervalMine(
  logger: FakeModulesLogger,
  mockTimer: MockTime
): Promise<void> {
  // Reset the hasChanged flag
  logger.hasChanged();

  // Convert to seconds
  mockTimer.addSeconds(BigInt(MINING_INTERVAL / 1000));

  // Wait until the block has been mined
  do {
    await sleep(50);
  } while (!logger.hasChanged());

  // For good measure, wait a bit longer to ensure all logs are printed
  await sleep(100);
}

async function setAutomine(
  provider: Provider,
  enabled: boolean
): Promise<void> {
  await provider.handleRequest(
    JSON.stringify({
      id: 1,
      jsonrpc: "2.0",
      method: "evm_setAutomine",
      params: [enabled],
    })
  );
}

const genesisState: AccountOverride[] = [
  {
    address: toBytes("0xbe862ad9abfe6f22bcb087716c7d89a26051f74c"),
    balance: 1000n * 10n ** 18n,
  },
  {
    address: toBytes("0x94a48723b9b46b19c72e3091838d0522618b9363"),
    balance: 1000n * 10n ** 18n,
  },
];

const MINING_INTERVAL = 1000; // 1000 milliseconds

const providerConfig = {
  // Allow blocks with the same timestamp, as some tests mine multiple blocks without changing the timestamp
  allowBlocksWithSameTimestamp: true,
  allowUnlimitedContractSize: true,
  bailOnCallFailure: false,
  bailOnTransactionFailure: false,
  blockGasLimit: 6_000_000n,
  chainId: 123n,
  chainOverrides: [],
  coinbase: Buffer.from("0000000000000000000000000000000000000000", "hex"),
  genesisState,
  hardfork: SHANGHAI,
  initialBlobGas: {
    gasUsed: 0n,
    excessGas: 0n,
  },
  initialParentBeaconBlockRoot: Buffer.from(
    "0000000000000000000000000000000000000000000000000000000000000000",
    "hex"
  ),
  minGasPrice: 0n,
  mining: {
    // Disable auto-mining for these tests
    autoMine: false,
    // Enable auto-mining
    interval: BigInt(MINING_INTERVAL),
    memPool: {
      order: MineOrdering.Priority,
    },
  },
  networkId: 123n,
  observability: {},
  ownedAccounts: [
    "0xe331b6d69882b4cb4ea581d88e0b604039a3de5967688d3dcffdd2270c0fd109",
    "0xe331b6d69882b4cb4ea581d88e0b604039a3de5967688d3dcffdd2270c0fd10a",
  ],
  precompileOverrides: [],
};

describe("Provider logs", function () {
  const mockTimer = MockTime.now();

  describe("Interval mining", function () {
    let gasPrice: bigint;
    let logger: FakeModulesLogger;
    let provider: Provider;
    beforeEach(async function () {
      logger = new FakeModulesLogger();

      const printLineFn = logger.printLineFn();
      const replaceLastLineFn = logger.replaceLastLineFn();

      const loggerConfig = {
        enable: true,
        decodeConsoleLogInputsCallback: (inputs: ArrayBuffer[]): string[] => {
          return ConsoleLogger.getDecodedLogs(
            inputs.map((input) => {
              return Buffer.from(input);
            })
          );
        },
        printLineCallback: (message: string, replace: boolean): void => {
          if (replace) {
            replaceLastLineFn(message);
          } else {
            printLineFn(message);
          }
        },
      };

      provider = await createProviderWithMockTimer(
        {
          ...providerConfig,
          genesisState: providerConfig.genesisState.concat(
            l1GenesisState(l1HardforkFromString(providerConfig.hardfork))
          ),
        },
        loggerConfig,
        {
          subscriptionCallback: (_event: SubscriptionEvent) => {},
        },
        new ContractDecoder(),
        mockTimer
      );

      gasPrice = await getGasPrice(provider);

      // Remove the `eth_gasPrice` call from the logger
      logger.reset();
    });

    it("should only print the mined block when there are no pending txs", async function () {
      await intervalMine(logger, mockTimer);

      assert.lengthOf(logger.lines, 1);
      assert.match(
        logger.lines[0],
        /Mined empty block #\d+ with base fee \d+$/
      );
    });

    it("should collapse the mined block info", async function () {
      await intervalMine(logger, mockTimer);
      await intervalMine(logger, mockTimer);

      assert.lengthOf(logger.lines, 1);
      assert.match(logger.lines[0], /Mined empty block range #\d+ to #\d+/);
    });

    it("should stop collapsing when a different method is called", async function () {
      await intervalMine(logger, mockTimer);
      await intervalMine(logger, mockTimer);
      await getBlockNumber(provider);
      await intervalMine(logger, mockTimer);

      assert.lengthOf(logger.lines, 3);

      // prettier-ignore
      {
        assert.match(logger.lines[0], /Mined empty block range #\d+ to #\d+/);
        assert.equal(logger.lines[1], ansiColor( "eth_blockNumber", chalk.green));
        assert.match(logger.lines[2], /Mined empty block #\d+ with base fee \d+$/);
      }
    });

    it("should print a block with one transaction", async function () {
      await sendTransaction(provider, { gasPrice });

      logger.reset();

      await intervalMine(logger, mockTimer);

      console.log(logger.lines);
      assert.lengthOf(logger.lines, 9);
      // prettier-ignore
      {
            assert.match(logger.lines[0], /^Mined block #\d+$/);
            assert.match(logger.lines[1], /^  Block: 0x[0-9a-f]{64}/);
            assert.match(logger.lines[2], /^    Base fee: \d+$/);
            assert.match(logger.lines[3], /^    Transaction: 0x[0-9a-f]{64}/);
            assert.match(logger.lines[4], /^      From:      0x[0-9a-f]{40}/);
            assert.match(logger.lines[5], /^      To:        0x[0-9a-f]{40}/);
            assert.match(logger.lines[6], /^      Value:     0 ETH$/);
            assert.match(logger.lines[7], /^      Gas used:  21000 of 21000$/);
            assert.equal(logger.lines[8], "");
          }
    });

    it("should print a block with two transactions", async function () {
      await sendTransaction(provider, { gasPrice });
      await sendTransaction(provider, { gasPrice });

      logger.reset();

      await intervalMine(logger, mockTimer);

      assert.lengthOf(logger.lines, 15);
      // prettier-ignore
      {
            assert.match(logger.lines[0], /^Mined block #\d+$/);
            assert.match(logger.lines[1 ], /^  Block: 0x[0-9a-f]{64}/);
            assert.match(logger.lines[2 ], /^    Base fee: \d+$/);
            assert.match(logger.lines[3 ], /^    Transaction: 0x[0-9a-f]{64}/);
            assert.match(logger.lines[4 ], /^      From:      0x[0-9a-f]{40}/);
            assert.match(logger.lines[5 ], /^      To:        0x[0-9a-f]{40}/);
            assert.match(logger.lines[6 ], /^      Value:     0 ETH$/);
            assert.match(logger.lines[7 ], /^      Gas used:  21000 of 21000$/);
            assert.equal(logger.lines[8 ], "");
            assert.match(logger.lines[9 ], /^    Transaction: 0x[0-9a-f]{64}/);
            assert.match(logger.lines[10], /^      From:      0x[0-9a-f]{40}/);
            assert.match(logger.lines[11], /^      To:        0x[0-9a-f]{40}/);
            assert.match(logger.lines[12], /^      Value:     0 ETH$/);
            assert.match(logger.lines[13], /^      Gas used:  21000 of 21000$/);
            assert.equal(logger.lines[14], "");
          }
    });

    it("should print stack traces", async function () {
      await setAutomine(provider, true);
      const address = await deployContract(
        provider,
        `0x${EXAMPLE_READ_CONTRACT.bytecode.object}`
      );
      await setAutomine(provider, false);

      await sendTransaction(provider, { gasPrice });
      await sendTransaction(provider, {
        to: address,
        gas: 1000000,
        data: EXAMPLE_READ_CONTRACT.selectors.blockGasLimit,
        value: 1,
        gasPrice,
      }).catch(() => {});

      logger.reset();

      await intervalMine(logger, mockTimer);

      assert.lengthOf(logger.lines, 18);
      // prettier-ignore
      {
            assert.match(logger.lines[0], /^Mined block #\d+$/);
            assert.match(logger.lines[1 ], /^  Block:\s+0x[0-9a-f]{64}/);
            assert.match(logger.lines[2 ], /^    Base fee: \d+$/);
            assert.match(logger.lines[3 ], /^    Transaction:\s+0x[0-9a-f]{64}/);
            assert.match(logger.lines[4 ], /^      From:\s+0x[0-9a-f]{40}/);
            assert.match(logger.lines[5 ], /^      To:\s+0x[0-9a-f]{40}/);
            assert.match(logger.lines[6 ], /^      Value:\s+0 ETH$/);
            assert.match(logger.lines[7 ], /^      Gas used:\s+21000 of 21000$/);
            assert.equal(logger.lines[8 ], "");
            assert.match(logger.lines[9 ], /^    Transaction:\s+0x[0-9a-f]{64}/);
            assert.match(logger.lines[10], /^      Contract call:\s+<UnrecognizedContract>$/);
            assert.match(logger.lines[11], /^      From:\s+0x[0-9a-f]{40}/);
            assert.match(logger.lines[12], /^      To:\s+0x[0-9a-f]{40}/);
            assert.match(logger.lines[13], /^      Value:\s+1 wei$/);
            assert.match(logger.lines[14], /^      Gas used:\s+21109 of 1000000$/);
            assert.equal(logger.lines[15], "");
            assert.match(logger.lines[16], /^      Error: Transaction reverted without a reason/);
            assert.equal(logger.lines[17], "");
          }
    });
  });
});
