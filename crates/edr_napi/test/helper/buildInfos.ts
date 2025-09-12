export const exampleBuildInfo = {
  _format: "hh3-sol-build-info-1",
  id: "solc-0_8_24-15ab458a32a758d340ff3c3978fddae8748ee3ff",
  solcVersion: "0.8.24",
  solcLongVersion: "0.8.24",
  input: {
    language: "Solidity",
    settings: {
      evmVersion: "cancun",
      outputSelection: {
        "*": {
          "": ["ast"],
          "*": [
            "abi",
            "evm.bytecode",
            "evm.deployedBytecode",
            "evm.methodIdentifiers",
            "metadata",
          ],
        },
      },
      remappings: [],
    },
    sources: {
      "project/contracts/MyLibrary.sol": {
        content:
          "// SPDX-License-Identifier: MIT OR Apache-2.0\npragma solidity ^0.8.24;\n\nlibrary MyLibrary {\n    function plus100(uint256 a) public pure returns (uint256) {\n        return a + 100;\n    }\n}\n",
      },
    },
  },
  output: {
    contracts: {
      "project/contracts/MyLibrary.sol": {
        MyLibrary: {
          abi: [
            {
              inputs: [{ internalType: "uint256", name: "a", type: "uint256" }],
              name: "plus100",
              outputs: [{ internalType: "uint256", name: "", type: "uint256" }],
              stateMutability: "pure",
              type: "function",
            },
          ],
          evm: {
            bytecode: {
              functionDebugData: {},
              generatedSources: [],
              linkReferences: {},
              object:
                "61019d61004e600b8282823980515f1a607314610042577f4e487b71000000000000000000000000000000000000000000000000000000005f525f60045260245ffd5b305f52607381538281f3fe7300000000000000000000000000000000000000003014608060405260043610610034575f3560e01c806368ba353b14610038575b5f80fd5b610052600480360381019061004d91906100b4565b610068565b60405161005f91906100ee565b60405180910390f35b5f6064826100769190610134565b9050919050565b5f80fd5b5f819050919050565b61009381610081565b811461009d575f80fd5b50565b5f813590506100ae8161008a565b92915050565b5f602082840312156100c9576100c861007d565b5b5f6100d6848285016100a0565b91505092915050565b6100e881610081565b82525050565b5f6020820190506101015f8301846100df565b92915050565b7f4e487b71000000000000000000000000000000000000000000000000000000005f52601160045260245ffd5b5f61013e82610081565b915061014983610081565b925082820190508082111561016157610160610107565b5b9291505056fea26469706673582212202d022e8c05e8d0961c6ef992cf96b566345825ce7b045868f0c7d8ec1386be7c64736f6c63430008180033",
              opcodes:
                "PUSH2 0x19D PUSH2 0x4E PUSH1 0xB DUP3 DUP3 DUP3 CODECOPY DUP1 MLOAD PUSH0 BYTE PUSH1 0x73 EQ PUSH2 0x42 JUMPI PUSH32 0x4E487B7100000000000000000000000000000000000000000000000000000000 PUSH0 MSTORE PUSH0 PUSH1 0x4 MSTORE PUSH1 0x24 PUSH0 REVERT JUMPDEST ADDRESS PUSH0 MSTORE PUSH1 0x73 DUP2 MSTORE8 DUP3 DUP2 RETURN INVALID PUSH20 0x0 ADDRESS EQ PUSH1 0x80 PUSH1 0x40 MSTORE PUSH1 0x4 CALLDATASIZE LT PUSH2 0x34 JUMPI PUSH0 CALLDATALOAD PUSH1 0xE0 SHR DUP1 PUSH4 0x68BA353B EQ PUSH2 0x38 JUMPI JUMPDEST PUSH0 DUP1 REVERT JUMPDEST PUSH2 0x52 PUSH1 0x4 DUP1 CALLDATASIZE SUB DUP2 ADD SWAP1 PUSH2 0x4D SWAP2 SWAP1 PUSH2 0xB4 JUMP JUMPDEST PUSH2 0x68 JUMP JUMPDEST PUSH1 0x40 MLOAD PUSH2 0x5F SWAP2 SWAP1 PUSH2 0xEE JUMP JUMPDEST PUSH1 0x40 MLOAD DUP1 SWAP2 SUB SWAP1 RETURN JUMPDEST PUSH0 PUSH1 0x64 DUP3 PUSH2 0x76 SWAP2 SWAP1 PUSH2 0x134 JUMP JUMPDEST SWAP1 POP SWAP2 SWAP1 POP JUMP JUMPDEST PUSH0 DUP1 REVERT JUMPDEST PUSH0 DUP2 SWAP1 POP SWAP2 SWAP1 POP JUMP JUMPDEST PUSH2 0x93 DUP2 PUSH2 0x81 JUMP JUMPDEST DUP2 EQ PUSH2 0x9D JUMPI PUSH0 DUP1 REVERT JUMPDEST POP JUMP JUMPDEST PUSH0 DUP2 CALLDATALOAD SWAP1 POP PUSH2 0xAE DUP2 PUSH2 0x8A JUMP JUMPDEST SWAP3 SWAP2 POP POP JUMP JUMPDEST PUSH0 PUSH1 0x20 DUP3 DUP5 SUB SLT ISZERO PUSH2 0xC9 JUMPI PUSH2 0xC8 PUSH2 0x7D JUMP JUMPDEST JUMPDEST PUSH0 PUSH2 0xD6 DUP5 DUP3 DUP6 ADD PUSH2 0xA0 JUMP JUMPDEST SWAP2 POP POP SWAP3 SWAP2 POP POP JUMP JUMPDEST PUSH2 0xE8 DUP2 PUSH2 0x81 JUMP JUMPDEST DUP3 MSTORE POP POP JUMP JUMPDEST PUSH0 PUSH1 0x20 DUP3 ADD SWAP1 POP PUSH2 0x101 PUSH0 DUP4 ADD DUP5 PUSH2 0xDF JUMP JUMPDEST SWAP3 SWAP2 POP POP JUMP JUMPDEST PUSH32 0x4E487B7100000000000000000000000000000000000000000000000000000000 PUSH0 MSTORE PUSH1 0x11 PUSH1 0x4 MSTORE PUSH1 0x24 PUSH0 REVERT JUMPDEST PUSH0 PUSH2 0x13E DUP3 PUSH2 0x81 JUMP JUMPDEST SWAP2 POP PUSH2 0x149 DUP4 PUSH2 0x81 JUMP JUMPDEST SWAP3 POP DUP3 DUP3 ADD SWAP1 POP DUP1 DUP3 GT ISZERO PUSH2 0x161 JUMPI PUSH2 0x160 PUSH2 0x107 JUMP JUMPDEST JUMPDEST SWAP3 SWAP2 POP POP JUMP INVALID LOG2 PUSH5 0x6970667358 0x22 SLT KECCAK256 0x2D MUL 0x2E DUP13 SDIV 0xE8 0xD0 SWAP7 SHR PUSH15 0xF992CF96B566345825CE7B045868F0 0xC7 0xD8 0xEC SGT DUP7 0xBE PUSH29 0x64736F6C63430008180033000000000000000000000000000000000000 ",
              sourceMap: "72:115:0:-:0;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;",
            },
            deployedBytecode: {
              functionDebugData: {
                "@plus100_13": {
                  entryPoint: 104,
                  id: 13,
                  parameterSlots: 1,
                  returnSlots: 1,
                },
                abi_decode_t_uint256: {
                  entryPoint: 160,
                  id: null,
                  parameterSlots: 2,
                  returnSlots: 1,
                },
                abi_decode_tuple_t_uint256: {
                  entryPoint: 180,
                  id: null,
                  parameterSlots: 2,
                  returnSlots: 1,
                },
                abi_encode_t_uint256_to_t_uint256_fromStack_library: {
                  entryPoint: 223,
                  id: null,
                  parameterSlots: 2,
                  returnSlots: 0,
                },
                abi_encode_tuple_t_uint256__to_t_uint256__fromStack_library_reversed:
                  {
                    entryPoint: 238,
                    id: null,
                    parameterSlots: 2,
                    returnSlots: 1,
                  },
                allocate_unbounded: {
                  entryPoint: null,
                  id: null,
                  parameterSlots: 0,
                  returnSlots: 1,
                },
                checked_add_t_uint256: {
                  entryPoint: 308,
                  id: null,
                  parameterSlots: 2,
                  returnSlots: 1,
                },
                cleanup_t_uint256: {
                  entryPoint: 129,
                  id: null,
                  parameterSlots: 1,
                  returnSlots: 1,
                },
                panic_error_0x11: {
                  entryPoint: 263,
                  id: null,
                  parameterSlots: 0,
                  returnSlots: 0,
                },
                revert_error_c1322bf8034eace5e0b5c7295db60986aa89aae5e0ea0873e4689e076861a5db:
                  {
                    entryPoint: null,
                    id: null,
                    parameterSlots: 0,
                    returnSlots: 0,
                  },
                revert_error_dbdddcbe895c83990c08b3492a0e83918d802a52331272ac6fdb6a7c4aea3b1b:
                  {
                    entryPoint: 125,
                    id: null,
                    parameterSlots: 0,
                    returnSlots: 0,
                  },
                validator_revert_t_uint256: {
                  entryPoint: 138,
                  id: null,
                  parameterSlots: 1,
                  returnSlots: 0,
                },
              },
              generatedSources: [
                {
                  ast: {
                    nativeSrc: "0:1781:1",
                    nodeType: "YulBlock",
                    src: "0:1781:1",
                    statements: [
                      {
                        body: {
                          nativeSrc: "47:35:1",
                          nodeType: "YulBlock",
                          src: "47:35:1",
                          statements: [
                            {
                              nativeSrc: "57:19:1",
                              nodeType: "YulAssignment",
                              src: "57:19:1",
                              value: {
                                arguments: [
                                  {
                                    kind: "number",
                                    nativeSrc: "73:2:1",
                                    nodeType: "YulLiteral",
                                    src: "73:2:1",
                                    type: "",
                                    value: "64",
                                  },
                                ],
                                functionName: {
                                  name: "mload",
                                  nativeSrc: "67:5:1",
                                  nodeType: "YulIdentifier",
                                  src: "67:5:1",
                                },
                                nativeSrc: "67:9:1",
                                nodeType: "YulFunctionCall",
                                src: "67:9:1",
                              },
                              variableNames: [
                                {
                                  name: "memPtr",
                                  nativeSrc: "57:6:1",
                                  nodeType: "YulIdentifier",
                                  src: "57:6:1",
                                },
                              ],
                            },
                          ],
                        },
                        name: "allocate_unbounded",
                        nativeSrc: "7:75:1",
                        nodeType: "YulFunctionDefinition",
                        returnVariables: [
                          {
                            name: "memPtr",
                            nativeSrc: "40:6:1",
                            nodeType: "YulTypedName",
                            src: "40:6:1",
                            type: "",
                          },
                        ],
                        src: "7:75:1",
                      },
                      {
                        body: {
                          nativeSrc: "177:28:1",
                          nodeType: "YulBlock",
                          src: "177:28:1",
                          statements: [
                            {
                              expression: {
                                arguments: [
                                  {
                                    kind: "number",
                                    nativeSrc: "194:1:1",
                                    nodeType: "YulLiteral",
                                    src: "194:1:1",
                                    type: "",
                                    value: "0",
                                  },
                                  {
                                    kind: "number",
                                    nativeSrc: "197:1:1",
                                    nodeType: "YulLiteral",
                                    src: "197:1:1",
                                    type: "",
                                    value: "0",
                                  },
                                ],
                                functionName: {
                                  name: "revert",
                                  nativeSrc: "187:6:1",
                                  nodeType: "YulIdentifier",
                                  src: "187:6:1",
                                },
                                nativeSrc: "187:12:1",
                                nodeType: "YulFunctionCall",
                                src: "187:12:1",
                              },
                              nativeSrc: "187:12:1",
                              nodeType: "YulExpressionStatement",
                              src: "187:12:1",
                            },
                          ],
                        },
                        name: "revert_error_dbdddcbe895c83990c08b3492a0e83918d802a52331272ac6fdb6a7c4aea3b1b",
                        nativeSrc: "88:117:1",
                        nodeType: "YulFunctionDefinition",
                        src: "88:117:1",
                      },
                      {
                        body: {
                          nativeSrc: "300:28:1",
                          nodeType: "YulBlock",
                          src: "300:28:1",
                          statements: [
                            {
                              expression: {
                                arguments: [
                                  {
                                    kind: "number",
                                    nativeSrc: "317:1:1",
                                    nodeType: "YulLiteral",
                                    src: "317:1:1",
                                    type: "",
                                    value: "0",
                                  },
                                  {
                                    kind: "number",
                                    nativeSrc: "320:1:1",
                                    nodeType: "YulLiteral",
                                    src: "320:1:1",
                                    type: "",
                                    value: "0",
                                  },
                                ],
                                functionName: {
                                  name: "revert",
                                  nativeSrc: "310:6:1",
                                  nodeType: "YulIdentifier",
                                  src: "310:6:1",
                                },
                                nativeSrc: "310:12:1",
                                nodeType: "YulFunctionCall",
                                src: "310:12:1",
                              },
                              nativeSrc: "310:12:1",
                              nodeType: "YulExpressionStatement",
                              src: "310:12:1",
                            },
                          ],
                        },
                        name: "revert_error_c1322bf8034eace5e0b5c7295db60986aa89aae5e0ea0873e4689e076861a5db",
                        nativeSrc: "211:117:1",
                        nodeType: "YulFunctionDefinition",
                        src: "211:117:1",
                      },
                      {
                        body: {
                          nativeSrc: "379:32:1",
                          nodeType: "YulBlock",
                          src: "379:32:1",
                          statements: [
                            {
                              nativeSrc: "389:16:1",
                              nodeType: "YulAssignment",
                              src: "389:16:1",
                              value: {
                                name: "value",
                                nativeSrc: "400:5:1",
                                nodeType: "YulIdentifier",
                                src: "400:5:1",
                              },
                              variableNames: [
                                {
                                  name: "cleaned",
                                  nativeSrc: "389:7:1",
                                  nodeType: "YulIdentifier",
                                  src: "389:7:1",
                                },
                              ],
                            },
                          ],
                        },
                        name: "cleanup_t_uint256",
                        nativeSrc: "334:77:1",
                        nodeType: "YulFunctionDefinition",
                        parameters: [
                          {
                            name: "value",
                            nativeSrc: "361:5:1",
                            nodeType: "YulTypedName",
                            src: "361:5:1",
                            type: "",
                          },
                        ],
                        returnVariables: [
                          {
                            name: "cleaned",
                            nativeSrc: "371:7:1",
                            nodeType: "YulTypedName",
                            src: "371:7:1",
                            type: "",
                          },
                        ],
                        src: "334:77:1",
                      },
                      {
                        body: {
                          nativeSrc: "460:79:1",
                          nodeType: "YulBlock",
                          src: "460:79:1",
                          statements: [
                            {
                              body: {
                                nativeSrc: "517:16:1",
                                nodeType: "YulBlock",
                                src: "517:16:1",
                                statements: [
                                  {
                                    expression: {
                                      arguments: [
                                        {
                                          kind: "number",
                                          nativeSrc: "526:1:1",
                                          nodeType: "YulLiteral",
                                          src: "526:1:1",
                                          type: "",
                                          value: "0",
                                        },
                                        {
                                          kind: "number",
                                          nativeSrc: "529:1:1",
                                          nodeType: "YulLiteral",
                                          src: "529:1:1",
                                          type: "",
                                          value: "0",
                                        },
                                      ],
                                      functionName: {
                                        name: "revert",
                                        nativeSrc: "519:6:1",
                                        nodeType: "YulIdentifier",
                                        src: "519:6:1",
                                      },
                                      nativeSrc: "519:12:1",
                                      nodeType: "YulFunctionCall",
                                      src: "519:12:1",
                                    },
                                    nativeSrc: "519:12:1",
                                    nodeType: "YulExpressionStatement",
                                    src: "519:12:1",
                                  },
                                ],
                              },
                              condition: {
                                arguments: [
                                  {
                                    arguments: [
                                      {
                                        name: "value",
                                        nativeSrc: "483:5:1",
                                        nodeType: "YulIdentifier",
                                        src: "483:5:1",
                                      },
                                      {
                                        arguments: [
                                          {
                                            name: "value",
                                            nativeSrc: "508:5:1",
                                            nodeType: "YulIdentifier",
                                            src: "508:5:1",
                                          },
                                        ],
                                        functionName: {
                                          name: "cleanup_t_uint256",
                                          nativeSrc: "490:17:1",
                                          nodeType: "YulIdentifier",
                                          src: "490:17:1",
                                        },
                                        nativeSrc: "490:24:1",
                                        nodeType: "YulFunctionCall",
                                        src: "490:24:1",
                                      },
                                    ],
                                    functionName: {
                                      name: "eq",
                                      nativeSrc: "480:2:1",
                                      nodeType: "YulIdentifier",
                                      src: "480:2:1",
                                    },
                                    nativeSrc: "480:35:1",
                                    nodeType: "YulFunctionCall",
                                    src: "480:35:1",
                                  },
                                ],
                                functionName: {
                                  name: "iszero",
                                  nativeSrc: "473:6:1",
                                  nodeType: "YulIdentifier",
                                  src: "473:6:1",
                                },
                                nativeSrc: "473:43:1",
                                nodeType: "YulFunctionCall",
                                src: "473:43:1",
                              },
                              nativeSrc: "470:63:1",
                              nodeType: "YulIf",
                              src: "470:63:1",
                            },
                          ],
                        },
                        name: "validator_revert_t_uint256",
                        nativeSrc: "417:122:1",
                        nodeType: "YulFunctionDefinition",
                        parameters: [
                          {
                            name: "value",
                            nativeSrc: "453:5:1",
                            nodeType: "YulTypedName",
                            src: "453:5:1",
                            type: "",
                          },
                        ],
                        src: "417:122:1",
                      },
                      {
                        body: {
                          nativeSrc: "597:87:1",
                          nodeType: "YulBlock",
                          src: "597:87:1",
                          statements: [
                            {
                              nativeSrc: "607:29:1",
                              nodeType: "YulAssignment",
                              src: "607:29:1",
                              value: {
                                arguments: [
                                  {
                                    name: "offset",
                                    nativeSrc: "629:6:1",
                                    nodeType: "YulIdentifier",
                                    src: "629:6:1",
                                  },
                                ],
                                functionName: {
                                  name: "calldataload",
                                  nativeSrc: "616:12:1",
                                  nodeType: "YulIdentifier",
                                  src: "616:12:1",
                                },
                                nativeSrc: "616:20:1",
                                nodeType: "YulFunctionCall",
                                src: "616:20:1",
                              },
                              variableNames: [
                                {
                                  name: "value",
                                  nativeSrc: "607:5:1",
                                  nodeType: "YulIdentifier",
                                  src: "607:5:1",
                                },
                              ],
                            },
                            {
                              expression: {
                                arguments: [
                                  {
                                    name: "value",
                                    nativeSrc: "672:5:1",
                                    nodeType: "YulIdentifier",
                                    src: "672:5:1",
                                  },
                                ],
                                functionName: {
                                  name: "validator_revert_t_uint256",
                                  nativeSrc: "645:26:1",
                                  nodeType: "YulIdentifier",
                                  src: "645:26:1",
                                },
                                nativeSrc: "645:33:1",
                                nodeType: "YulFunctionCall",
                                src: "645:33:1",
                              },
                              nativeSrc: "645:33:1",
                              nodeType: "YulExpressionStatement",
                              src: "645:33:1",
                            },
                          ],
                        },
                        name: "abi_decode_t_uint256",
                        nativeSrc: "545:139:1",
                        nodeType: "YulFunctionDefinition",
                        parameters: [
                          {
                            name: "offset",
                            nativeSrc: "575:6:1",
                            nodeType: "YulTypedName",
                            src: "575:6:1",
                            type: "",
                          },
                          {
                            name: "end",
                            nativeSrc: "583:3:1",
                            nodeType: "YulTypedName",
                            src: "583:3:1",
                            type: "",
                          },
                        ],
                        returnVariables: [
                          {
                            name: "value",
                            nativeSrc: "591:5:1",
                            nodeType: "YulTypedName",
                            src: "591:5:1",
                            type: "",
                          },
                        ],
                        src: "545:139:1",
                      },
                      {
                        body: {
                          nativeSrc: "756:263:1",
                          nodeType: "YulBlock",
                          src: "756:263:1",
                          statements: [
                            {
                              body: {
                                nativeSrc: "802:83:1",
                                nodeType: "YulBlock",
                                src: "802:83:1",
                                statements: [
                                  {
                                    expression: {
                                      arguments: [],
                                      functionName: {
                                        name: "revert_error_dbdddcbe895c83990c08b3492a0e83918d802a52331272ac6fdb6a7c4aea3b1b",
                                        nativeSrc: "804:77:1",
                                        nodeType: "YulIdentifier",
                                        src: "804:77:1",
                                      },
                                      nativeSrc: "804:79:1",
                                      nodeType: "YulFunctionCall",
                                      src: "804:79:1",
                                    },
                                    nativeSrc: "804:79:1",
                                    nodeType: "YulExpressionStatement",
                                    src: "804:79:1",
                                  },
                                ],
                              },
                              condition: {
                                arguments: [
                                  {
                                    arguments: [
                                      {
                                        name: "dataEnd",
                                        nativeSrc: "777:7:1",
                                        nodeType: "YulIdentifier",
                                        src: "777:7:1",
                                      },
                                      {
                                        name: "headStart",
                                        nativeSrc: "786:9:1",
                                        nodeType: "YulIdentifier",
                                        src: "786:9:1",
                                      },
                                    ],
                                    functionName: {
                                      name: "sub",
                                      nativeSrc: "773:3:1",
                                      nodeType: "YulIdentifier",
                                      src: "773:3:1",
                                    },
                                    nativeSrc: "773:23:1",
                                    nodeType: "YulFunctionCall",
                                    src: "773:23:1",
                                  },
                                  {
                                    kind: "number",
                                    nativeSrc: "798:2:1",
                                    nodeType: "YulLiteral",
                                    src: "798:2:1",
                                    type: "",
                                    value: "32",
                                  },
                                ],
                                functionName: {
                                  name: "slt",
                                  nativeSrc: "769:3:1",
                                  nodeType: "YulIdentifier",
                                  src: "769:3:1",
                                },
                                nativeSrc: "769:32:1",
                                nodeType: "YulFunctionCall",
                                src: "769:32:1",
                              },
                              nativeSrc: "766:119:1",
                              nodeType: "YulIf",
                              src: "766:119:1",
                            },
                            {
                              nativeSrc: "895:117:1",
                              nodeType: "YulBlock",
                              src: "895:117:1",
                              statements: [
                                {
                                  nativeSrc: "910:15:1",
                                  nodeType: "YulVariableDeclaration",
                                  src: "910:15:1",
                                  value: {
                                    kind: "number",
                                    nativeSrc: "924:1:1",
                                    nodeType: "YulLiteral",
                                    src: "924:1:1",
                                    type: "",
                                    value: "0",
                                  },
                                  variables: [
                                    {
                                      name: "offset",
                                      nativeSrc: "914:6:1",
                                      nodeType: "YulTypedName",
                                      src: "914:6:1",
                                      type: "",
                                    },
                                  ],
                                },
                                {
                                  nativeSrc: "939:63:1",
                                  nodeType: "YulAssignment",
                                  src: "939:63:1",
                                  value: {
                                    arguments: [
                                      {
                                        arguments: [
                                          {
                                            name: "headStart",
                                            nativeSrc: "974:9:1",
                                            nodeType: "YulIdentifier",
                                            src: "974:9:1",
                                          },
                                          {
                                            name: "offset",
                                            nativeSrc: "985:6:1",
                                            nodeType: "YulIdentifier",
                                            src: "985:6:1",
                                          },
                                        ],
                                        functionName: {
                                          name: "add",
                                          nativeSrc: "970:3:1",
                                          nodeType: "YulIdentifier",
                                          src: "970:3:1",
                                        },
                                        nativeSrc: "970:22:1",
                                        nodeType: "YulFunctionCall",
                                        src: "970:22:1",
                                      },
                                      {
                                        name: "dataEnd",
                                        nativeSrc: "994:7:1",
                                        nodeType: "YulIdentifier",
                                        src: "994:7:1",
                                      },
                                    ],
                                    functionName: {
                                      name: "abi_decode_t_uint256",
                                      nativeSrc: "949:20:1",
                                      nodeType: "YulIdentifier",
                                      src: "949:20:1",
                                    },
                                    nativeSrc: "949:53:1",
                                    nodeType: "YulFunctionCall",
                                    src: "949:53:1",
                                  },
                                  variableNames: [
                                    {
                                      name: "value0",
                                      nativeSrc: "939:6:1",
                                      nodeType: "YulIdentifier",
                                      src: "939:6:1",
                                    },
                                  ],
                                },
                              ],
                            },
                          ],
                        },
                        name: "abi_decode_tuple_t_uint256",
                        nativeSrc: "690:329:1",
                        nodeType: "YulFunctionDefinition",
                        parameters: [
                          {
                            name: "headStart",
                            nativeSrc: "726:9:1",
                            nodeType: "YulTypedName",
                            src: "726:9:1",
                            type: "",
                          },
                          {
                            name: "dataEnd",
                            nativeSrc: "737:7:1",
                            nodeType: "YulTypedName",
                            src: "737:7:1",
                            type: "",
                          },
                        ],
                        returnVariables: [
                          {
                            name: "value0",
                            nativeSrc: "749:6:1",
                            nodeType: "YulTypedName",
                            src: "749:6:1",
                            type: "",
                          },
                        ],
                        src: "690:329:1",
                      },
                      {
                        body: {
                          nativeSrc: "1098:53:1",
                          nodeType: "YulBlock",
                          src: "1098:53:1",
                          statements: [
                            {
                              expression: {
                                arguments: [
                                  {
                                    name: "pos",
                                    nativeSrc: "1115:3:1",
                                    nodeType: "YulIdentifier",
                                    src: "1115:3:1",
                                  },
                                  {
                                    arguments: [
                                      {
                                        name: "value",
                                        nativeSrc: "1138:5:1",
                                        nodeType: "YulIdentifier",
                                        src: "1138:5:1",
                                      },
                                    ],
                                    functionName: {
                                      name: "cleanup_t_uint256",
                                      nativeSrc: "1120:17:1",
                                      nodeType: "YulIdentifier",
                                      src: "1120:17:1",
                                    },
                                    nativeSrc: "1120:24:1",
                                    nodeType: "YulFunctionCall",
                                    src: "1120:24:1",
                                  },
                                ],
                                functionName: {
                                  name: "mstore",
                                  nativeSrc: "1108:6:1",
                                  nodeType: "YulIdentifier",
                                  src: "1108:6:1",
                                },
                                nativeSrc: "1108:37:1",
                                nodeType: "YulFunctionCall",
                                src: "1108:37:1",
                              },
                              nativeSrc: "1108:37:1",
                              nodeType: "YulExpressionStatement",
                              src: "1108:37:1",
                            },
                          ],
                        },
                        name: "abi_encode_t_uint256_to_t_uint256_fromStack_library",
                        nativeSrc: "1025:126:1",
                        nodeType: "YulFunctionDefinition",
                        parameters: [
                          {
                            name: "value",
                            nativeSrc: "1086:5:1",
                            nodeType: "YulTypedName",
                            src: "1086:5:1",
                            type: "",
                          },
                          {
                            name: "pos",
                            nativeSrc: "1093:3:1",
                            nodeType: "YulTypedName",
                            src: "1093:3:1",
                            type: "",
                          },
                        ],
                        src: "1025:126:1",
                      },
                      {
                        body: {
                          nativeSrc: "1263:132:1",
                          nodeType: "YulBlock",
                          src: "1263:132:1",
                          statements: [
                            {
                              nativeSrc: "1273:26:1",
                              nodeType: "YulAssignment",
                              src: "1273:26:1",
                              value: {
                                arguments: [
                                  {
                                    name: "headStart",
                                    nativeSrc: "1285:9:1",
                                    nodeType: "YulIdentifier",
                                    src: "1285:9:1",
                                  },
                                  {
                                    kind: "number",
                                    nativeSrc: "1296:2:1",
                                    nodeType: "YulLiteral",
                                    src: "1296:2:1",
                                    type: "",
                                    value: "32",
                                  },
                                ],
                                functionName: {
                                  name: "add",
                                  nativeSrc: "1281:3:1",
                                  nodeType: "YulIdentifier",
                                  src: "1281:3:1",
                                },
                                nativeSrc: "1281:18:1",
                                nodeType: "YulFunctionCall",
                                src: "1281:18:1",
                              },
                              variableNames: [
                                {
                                  name: "tail",
                                  nativeSrc: "1273:4:1",
                                  nodeType: "YulIdentifier",
                                  src: "1273:4:1",
                                },
                              ],
                            },
                            {
                              expression: {
                                arguments: [
                                  {
                                    name: "value0",
                                    nativeSrc: "1361:6:1",
                                    nodeType: "YulIdentifier",
                                    src: "1361:6:1",
                                  },
                                  {
                                    arguments: [
                                      {
                                        name: "headStart",
                                        nativeSrc: "1374:9:1",
                                        nodeType: "YulIdentifier",
                                        src: "1374:9:1",
                                      },
                                      {
                                        kind: "number",
                                        nativeSrc: "1385:1:1",
                                        nodeType: "YulLiteral",
                                        src: "1385:1:1",
                                        type: "",
                                        value: "0",
                                      },
                                    ],
                                    functionName: {
                                      name: "add",
                                      nativeSrc: "1370:3:1",
                                      nodeType: "YulIdentifier",
                                      src: "1370:3:1",
                                    },
                                    nativeSrc: "1370:17:1",
                                    nodeType: "YulFunctionCall",
                                    src: "1370:17:1",
                                  },
                                ],
                                functionName: {
                                  name: "abi_encode_t_uint256_to_t_uint256_fromStack_library",
                                  nativeSrc: "1309:51:1",
                                  nodeType: "YulIdentifier",
                                  src: "1309:51:1",
                                },
                                nativeSrc: "1309:79:1",
                                nodeType: "YulFunctionCall",
                                src: "1309:79:1",
                              },
                              nativeSrc: "1309:79:1",
                              nodeType: "YulExpressionStatement",
                              src: "1309:79:1",
                            },
                          ],
                        },
                        name: "abi_encode_tuple_t_uint256__to_t_uint256__fromStack_library_reversed",
                        nativeSrc: "1157:238:1",
                        nodeType: "YulFunctionDefinition",
                        parameters: [
                          {
                            name: "headStart",
                            nativeSrc: "1235:9:1",
                            nodeType: "YulTypedName",
                            src: "1235:9:1",
                            type: "",
                          },
                          {
                            name: "value0",
                            nativeSrc: "1247:6:1",
                            nodeType: "YulTypedName",
                            src: "1247:6:1",
                            type: "",
                          },
                        ],
                        returnVariables: [
                          {
                            name: "tail",
                            nativeSrc: "1258:4:1",
                            nodeType: "YulTypedName",
                            src: "1258:4:1",
                            type: "",
                          },
                        ],
                        src: "1157:238:1",
                      },
                      {
                        body: {
                          nativeSrc: "1429:152:1",
                          nodeType: "YulBlock",
                          src: "1429:152:1",
                          statements: [
                            {
                              expression: {
                                arguments: [
                                  {
                                    kind: "number",
                                    nativeSrc: "1446:1:1",
                                    nodeType: "YulLiteral",
                                    src: "1446:1:1",
                                    type: "",
                                    value: "0",
                                  },
                                  {
                                    kind: "number",
                                    nativeSrc: "1449:77:1",
                                    nodeType: "YulLiteral",
                                    src: "1449:77:1",
                                    type: "",
                                    value:
                                      "35408467139433450592217433187231851964531694900788300625387963629091585785856",
                                  },
                                ],
                                functionName: {
                                  name: "mstore",
                                  nativeSrc: "1439:6:1",
                                  nodeType: "YulIdentifier",
                                  src: "1439:6:1",
                                },
                                nativeSrc: "1439:88:1",
                                nodeType: "YulFunctionCall",
                                src: "1439:88:1",
                              },
                              nativeSrc: "1439:88:1",
                              nodeType: "YulExpressionStatement",
                              src: "1439:88:1",
                            },
                            {
                              expression: {
                                arguments: [
                                  {
                                    kind: "number",
                                    nativeSrc: "1543:1:1",
                                    nodeType: "YulLiteral",
                                    src: "1543:1:1",
                                    type: "",
                                    value: "4",
                                  },
                                  {
                                    kind: "number",
                                    nativeSrc: "1546:4:1",
                                    nodeType: "YulLiteral",
                                    src: "1546:4:1",
                                    type: "",
                                    value: "0x11",
                                  },
                                ],
                                functionName: {
                                  name: "mstore",
                                  nativeSrc: "1536:6:1",
                                  nodeType: "YulIdentifier",
                                  src: "1536:6:1",
                                },
                                nativeSrc: "1536:15:1",
                                nodeType: "YulFunctionCall",
                                src: "1536:15:1",
                              },
                              nativeSrc: "1536:15:1",
                              nodeType: "YulExpressionStatement",
                              src: "1536:15:1",
                            },
                            {
                              expression: {
                                arguments: [
                                  {
                                    kind: "number",
                                    nativeSrc: "1567:1:1",
                                    nodeType: "YulLiteral",
                                    src: "1567:1:1",
                                    type: "",
                                    value: "0",
                                  },
                                  {
                                    kind: "number",
                                    nativeSrc: "1570:4:1",
                                    nodeType: "YulLiteral",
                                    src: "1570:4:1",
                                    type: "",
                                    value: "0x24",
                                  },
                                ],
                                functionName: {
                                  name: "revert",
                                  nativeSrc: "1560:6:1",
                                  nodeType: "YulIdentifier",
                                  src: "1560:6:1",
                                },
                                nativeSrc: "1560:15:1",
                                nodeType: "YulFunctionCall",
                                src: "1560:15:1",
                              },
                              nativeSrc: "1560:15:1",
                              nodeType: "YulExpressionStatement",
                              src: "1560:15:1",
                            },
                          ],
                        },
                        name: "panic_error_0x11",
                        nativeSrc: "1401:180:1",
                        nodeType: "YulFunctionDefinition",
                        src: "1401:180:1",
                      },
                      {
                        body: {
                          nativeSrc: "1631:147:1",
                          nodeType: "YulBlock",
                          src: "1631:147:1",
                          statements: [
                            {
                              nativeSrc: "1641:25:1",
                              nodeType: "YulAssignment",
                              src: "1641:25:1",
                              value: {
                                arguments: [
                                  {
                                    name: "x",
                                    nativeSrc: "1664:1:1",
                                    nodeType: "YulIdentifier",
                                    src: "1664:1:1",
                                  },
                                ],
                                functionName: {
                                  name: "cleanup_t_uint256",
                                  nativeSrc: "1646:17:1",
                                  nodeType: "YulIdentifier",
                                  src: "1646:17:1",
                                },
                                nativeSrc: "1646:20:1",
                                nodeType: "YulFunctionCall",
                                src: "1646:20:1",
                              },
                              variableNames: [
                                {
                                  name: "x",
                                  nativeSrc: "1641:1:1",
                                  nodeType: "YulIdentifier",
                                  src: "1641:1:1",
                                },
                              ],
                            },
                            {
                              nativeSrc: "1675:25:1",
                              nodeType: "YulAssignment",
                              src: "1675:25:1",
                              value: {
                                arguments: [
                                  {
                                    name: "y",
                                    nativeSrc: "1698:1:1",
                                    nodeType: "YulIdentifier",
                                    src: "1698:1:1",
                                  },
                                ],
                                functionName: {
                                  name: "cleanup_t_uint256",
                                  nativeSrc: "1680:17:1",
                                  nodeType: "YulIdentifier",
                                  src: "1680:17:1",
                                },
                                nativeSrc: "1680:20:1",
                                nodeType: "YulFunctionCall",
                                src: "1680:20:1",
                              },
                              variableNames: [
                                {
                                  name: "y",
                                  nativeSrc: "1675:1:1",
                                  nodeType: "YulIdentifier",
                                  src: "1675:1:1",
                                },
                              ],
                            },
                            {
                              nativeSrc: "1709:16:1",
                              nodeType: "YulAssignment",
                              src: "1709:16:1",
                              value: {
                                arguments: [
                                  {
                                    name: "x",
                                    nativeSrc: "1720:1:1",
                                    nodeType: "YulIdentifier",
                                    src: "1720:1:1",
                                  },
                                  {
                                    name: "y",
                                    nativeSrc: "1723:1:1",
                                    nodeType: "YulIdentifier",
                                    src: "1723:1:1",
                                  },
                                ],
                                functionName: {
                                  name: "add",
                                  nativeSrc: "1716:3:1",
                                  nodeType: "YulIdentifier",
                                  src: "1716:3:1",
                                },
                                nativeSrc: "1716:9:1",
                                nodeType: "YulFunctionCall",
                                src: "1716:9:1",
                              },
                              variableNames: [
                                {
                                  name: "sum",
                                  nativeSrc: "1709:3:1",
                                  nodeType: "YulIdentifier",
                                  src: "1709:3:1",
                                },
                              ],
                            },
                            {
                              body: {
                                nativeSrc: "1749:22:1",
                                nodeType: "YulBlock",
                                src: "1749:22:1",
                                statements: [
                                  {
                                    expression: {
                                      arguments: [],
                                      functionName: {
                                        name: "panic_error_0x11",
                                        nativeSrc: "1751:16:1",
                                        nodeType: "YulIdentifier",
                                        src: "1751:16:1",
                                      },
                                      nativeSrc: "1751:18:1",
                                      nodeType: "YulFunctionCall",
                                      src: "1751:18:1",
                                    },
                                    nativeSrc: "1751:18:1",
                                    nodeType: "YulExpressionStatement",
                                    src: "1751:18:1",
                                  },
                                ],
                              },
                              condition: {
                                arguments: [
                                  {
                                    name: "x",
                                    nativeSrc: "1741:1:1",
                                    nodeType: "YulIdentifier",
                                    src: "1741:1:1",
                                  },
                                  {
                                    name: "sum",
                                    nativeSrc: "1744:3:1",
                                    nodeType: "YulIdentifier",
                                    src: "1744:3:1",
                                  },
                                ],
                                functionName: {
                                  name: "gt",
                                  nativeSrc: "1738:2:1",
                                  nodeType: "YulIdentifier",
                                  src: "1738:2:1",
                                },
                                nativeSrc: "1738:10:1",
                                nodeType: "YulFunctionCall",
                                src: "1738:10:1",
                              },
                              nativeSrc: "1735:36:1",
                              nodeType: "YulIf",
                              src: "1735:36:1",
                            },
                          ],
                        },
                        name: "checked_add_t_uint256",
                        nativeSrc: "1587:191:1",
                        nodeType: "YulFunctionDefinition",
                        parameters: [
                          {
                            name: "x",
                            nativeSrc: "1618:1:1",
                            nodeType: "YulTypedName",
                            src: "1618:1:1",
                            type: "",
                          },
                          {
                            name: "y",
                            nativeSrc: "1621:1:1",
                            nodeType: "YulTypedName",
                            src: "1621:1:1",
                            type: "",
                          },
                        ],
                        returnVariables: [
                          {
                            name: "sum",
                            nativeSrc: "1627:3:1",
                            nodeType: "YulTypedName",
                            src: "1627:3:1",
                            type: "",
                          },
                        ],
                        src: "1587:191:1",
                      },
                    ],
                  },
                  contents:
                    "{\n\n    function allocate_unbounded() -> memPtr {\n        memPtr := mload(64)\n    }\n\n    function revert_error_dbdddcbe895c83990c08b3492a0e83918d802a52331272ac6fdb6a7c4aea3b1b() {\n        revert(0, 0)\n    }\n\n    function revert_error_c1322bf8034eace5e0b5c7295db60986aa89aae5e0ea0873e4689e076861a5db() {\n        revert(0, 0)\n    }\n\n    function cleanup_t_uint256(value) -> cleaned {\n        cleaned := value\n    }\n\n    function validator_revert_t_uint256(value) {\n        if iszero(eq(value, cleanup_t_uint256(value))) { revert(0, 0) }\n    }\n\n    function abi_decode_t_uint256(offset, end) -> value {\n        value := calldataload(offset)\n        validator_revert_t_uint256(value)\n    }\n\n    function abi_decode_tuple_t_uint256(headStart, dataEnd) -> value0 {\n        if slt(sub(dataEnd, headStart), 32) { revert_error_dbdddcbe895c83990c08b3492a0e83918d802a52331272ac6fdb6a7c4aea3b1b() }\n\n        {\n\n            let offset := 0\n\n            value0 := abi_decode_t_uint256(add(headStart, offset), dataEnd)\n        }\n\n    }\n\n    function abi_encode_t_uint256_to_t_uint256_fromStack_library(value, pos) {\n        mstore(pos, cleanup_t_uint256(value))\n    }\n\n    function abi_encode_tuple_t_uint256__to_t_uint256__fromStack_library_reversed(headStart , value0) -> tail {\n        tail := add(headStart, 32)\n\n        abi_encode_t_uint256_to_t_uint256_fromStack_library(value0,  add(headStart, 0))\n\n    }\n\n    function panic_error_0x11() {\n        mstore(0, 35408467139433450592217433187231851964531694900788300625387963629091585785856)\n        mstore(4, 0x11)\n        revert(0, 0x24)\n    }\n\n    function checked_add_t_uint256(x, y) -> sum {\n        x := cleanup_t_uint256(x)\n        y := cleanup_t_uint256(y)\n        sum := add(x, y)\n\n        if gt(x, sum) { panic_error_0x11() }\n\n    }\n\n}\n",
                  id: 1,
                  language: "Yul",
                  name: "#utility.yul",
                },
              ],
              immutableReferences: {},
              linkReferences: {},
              object:
                "7300000000000000000000000000000000000000003014608060405260043610610034575f3560e01c806368ba353b14610038575b5f80fd5b610052600480360381019061004d91906100b4565b610068565b60405161005f91906100ee565b60405180910390f35b5f6064826100769190610134565b9050919050565b5f80fd5b5f819050919050565b61009381610081565b811461009d575f80fd5b50565b5f813590506100ae8161008a565b92915050565b5f602082840312156100c9576100c861007d565b5b5f6100d6848285016100a0565b91505092915050565b6100e881610081565b82525050565b5f6020820190506101015f8301846100df565b92915050565b7f4e487b71000000000000000000000000000000000000000000000000000000005f52601160045260245ffd5b5f61013e82610081565b915061014983610081565b925082820190508082111561016157610160610107565b5b9291505056fea26469706673582212202d022e8c05e8d0961c6ef992cf96b566345825ce7b045868f0c7d8ec1386be7c64736f6c63430008180033",
              opcodes:
                "PUSH20 0x0 ADDRESS EQ PUSH1 0x80 PUSH1 0x40 MSTORE PUSH1 0x4 CALLDATASIZE LT PUSH2 0x34 JUMPI PUSH0 CALLDATALOAD PUSH1 0xE0 SHR DUP1 PUSH4 0x68BA353B EQ PUSH2 0x38 JUMPI JUMPDEST PUSH0 DUP1 REVERT JUMPDEST PUSH2 0x52 PUSH1 0x4 DUP1 CALLDATASIZE SUB DUP2 ADD SWAP1 PUSH2 0x4D SWAP2 SWAP1 PUSH2 0xB4 JUMP JUMPDEST PUSH2 0x68 JUMP JUMPDEST PUSH1 0x40 MLOAD PUSH2 0x5F SWAP2 SWAP1 PUSH2 0xEE JUMP JUMPDEST PUSH1 0x40 MLOAD DUP1 SWAP2 SUB SWAP1 RETURN JUMPDEST PUSH0 PUSH1 0x64 DUP3 PUSH2 0x76 SWAP2 SWAP1 PUSH2 0x134 JUMP JUMPDEST SWAP1 POP SWAP2 SWAP1 POP JUMP JUMPDEST PUSH0 DUP1 REVERT JUMPDEST PUSH0 DUP2 SWAP1 POP SWAP2 SWAP1 POP JUMP JUMPDEST PUSH2 0x93 DUP2 PUSH2 0x81 JUMP JUMPDEST DUP2 EQ PUSH2 0x9D JUMPI PUSH0 DUP1 REVERT JUMPDEST POP JUMP JUMPDEST PUSH0 DUP2 CALLDATALOAD SWAP1 POP PUSH2 0xAE DUP2 PUSH2 0x8A JUMP JUMPDEST SWAP3 SWAP2 POP POP JUMP JUMPDEST PUSH0 PUSH1 0x20 DUP3 DUP5 SUB SLT ISZERO PUSH2 0xC9 JUMPI PUSH2 0xC8 PUSH2 0x7D JUMP JUMPDEST JUMPDEST PUSH0 PUSH2 0xD6 DUP5 DUP3 DUP6 ADD PUSH2 0xA0 JUMP JUMPDEST SWAP2 POP POP SWAP3 SWAP2 POP POP JUMP JUMPDEST PUSH2 0xE8 DUP2 PUSH2 0x81 JUMP JUMPDEST DUP3 MSTORE POP POP JUMP JUMPDEST PUSH0 PUSH1 0x20 DUP3 ADD SWAP1 POP PUSH2 0x101 PUSH0 DUP4 ADD DUP5 PUSH2 0xDF JUMP JUMPDEST SWAP3 SWAP2 POP POP JUMP JUMPDEST PUSH32 0x4E487B7100000000000000000000000000000000000000000000000000000000 PUSH0 MSTORE PUSH1 0x11 PUSH1 0x4 MSTORE PUSH1 0x24 PUSH0 REVERT JUMPDEST PUSH0 PUSH2 0x13E DUP3 PUSH2 0x81 JUMP JUMPDEST SWAP2 POP PUSH2 0x149 DUP4 PUSH2 0x81 JUMP JUMPDEST SWAP3 POP DUP3 DUP3 ADD SWAP1 POP DUP1 DUP3 GT ISZERO PUSH2 0x161 JUMPI PUSH2 0x160 PUSH2 0x107 JUMP JUMPDEST JUMPDEST SWAP3 SWAP2 POP POP JUMP INVALID LOG2 PUSH5 0x6970667358 0x22 SLT KECCAK256 0x2D MUL 0x2E DUP13 SDIV 0xE8 0xD0 SWAP7 SHR PUSH15 0xF992CF96B566345825CE7B045868F0 0xC7 0xD8 0xEC SGT DUP7 0xBE PUSH29 0x64736F6C63430008180033000000000000000000000000000000000000 ",
              sourceMap:
                "72:115:0:-:0;;;;;;;;;;;;;;;;;;;;;;;;96:89;;;;;;;;;;;;;:::i;:::-;;:::i;:::-;;;;;;;:::i;:::-;;;;;;;;;145:7;175:3;171:1;:7;;;;:::i;:::-;164:14;;96:89;;;:::o;88:117:1:-;197:1;194;187:12;334:77;371:7;400:5;389:16;;334:77;;;:::o;417:122::-;490:24;508:5;490:24;:::i;:::-;483:5;480:35;470:63;;529:1;526;519:12;470:63;417:122;:::o;545:139::-;591:5;629:6;616:20;607:29;;645:33;672:5;645:33;:::i;:::-;545:139;;;;:::o;690:329::-;749:6;798:2;786:9;777:7;773:23;769:32;766:119;;;804:79;;:::i;:::-;766:119;924:1;949:53;994:7;985:6;974:9;970:22;949:53;:::i;:::-;939:63;;895:117;690:329;;;;:::o;1025:126::-;1120:24;1138:5;1120:24;:::i;:::-;1115:3;1108:37;1025:126;;:::o;1157:238::-;1258:4;1296:2;1285:9;1281:18;1273:26;;1309:79;1385:1;1374:9;1370:17;1361:6;1309:79;:::i;:::-;1157:238;;;;:::o;1401:180::-;1449:77;1446:1;1439:88;1546:4;1543:1;1536:15;1570:4;1567:1;1560:15;1587:191;1627:3;1646:20;1664:1;1646:20;:::i;:::-;1641:25;;1680:20;1698:1;1680:20;:::i;:::-;1675:25;;1723:1;1720;1716:9;1709:16;;1744:3;1741:1;1738:10;1735:36;;;1751:18;;:::i;:::-;1735:36;1587:191;;;;:::o",
            },
            methodIdentifiers: { "plus100(uint256)": "68ba353b" },
          },
          metadata:
            '{"compiler":{"version":"0.8.24+commit.e11b9ed9"},"language":"Solidity","output":{"abi":[{"inputs":[{"internalType":"uint256","name":"a","type":"uint256"}],"name":"plus100","outputs":[{"internalType":"uint256","name":"","type":"uint256"}],"stateMutability":"pure","type":"function"}],"devdoc":{"kind":"dev","methods":{},"version":1},"userdoc":{"kind":"user","methods":{},"version":1}},"settings":{"compilationTarget":{"project/contracts/MyLibrary.sol":"MyLibrary"},"evmVersion":"cancun","libraries":{},"metadata":{"bytecodeHash":"ipfs"},"optimizer":{"enabled":false,"runs":200},"remappings":[]},"sources":{"project/contracts/MyLibrary.sol":{"keccak256":"0x4a2f942b02b749815eb368d0df5817638827ee01723a28a140d58f1ee0f64c83","license":"MIT OR Apache-2.0","urls":["bzz-raw://07059b32b69d2008abb8d3731c963dae094424be0360ddd85cf23908a2356d5c","dweb:/ipfs/QmSHPXFzDC5CrRLS2Bv1SF2yqimpBFncqrdczjW3T15Lzi"]}},"version":1}',
        },
      },
    },
    sources: {
      "project/contracts/MyLibrary.sol": {
        ast: {
          absolutePath: "project/contracts/MyLibrary.sol",
          exportedSymbols: { MyLibrary: [14] },
          id: 15,
          license: "MIT OR Apache-2.0",
          nodeType: "SourceUnit",
          nodes: [
            {
              id: 1,
              literals: ["solidity", "^", "0.8", ".24"],
              nodeType: "PragmaDirective",
              src: "46:24:0",
            },
            {
              abstract: false,
              baseContracts: [],
              canonicalName: "MyLibrary",
              contractDependencies: [],
              contractKind: "library",
              fullyImplemented: true,
              id: 14,
              linearizedBaseContracts: [14],
              name: "MyLibrary",
              nameLocation: "80:9:0",
              nodeType: "ContractDefinition",
              nodes: [
                {
                  body: {
                    id: 12,
                    nodeType: "Block",
                    src: "154:31:0",
                    statements: [
                      {
                        expression: {
                          commonType: {
                            typeIdentifier: "t_uint256",
                            typeString: "uint256",
                          },
                          id: 10,
                          isConstant: false,
                          isLValue: false,
                          isPure: false,
                          lValueRequested: false,
                          leftExpression: {
                            id: 8,
                            name: "a",
                            nodeType: "Identifier",
                            overloadedDeclarations: [],
                            referencedDeclaration: 3,
                            src: "171:1:0",
                            typeDescriptions: {
                              typeIdentifier: "t_uint256",
                              typeString: "uint256",
                            },
                          },
                          nodeType: "BinaryOperation",
                          operator: "+",
                          rightExpression: {
                            hexValue: "313030",
                            id: 9,
                            isConstant: false,
                            isLValue: false,
                            isPure: true,
                            kind: "number",
                            lValueRequested: false,
                            nodeType: "Literal",
                            src: "175:3:0",
                            typeDescriptions: {
                              typeIdentifier: "t_rational_100_by_1",
                              typeString: "int_const 100",
                            },
                            value: "100",
                          },
                          src: "171:7:0",
                          typeDescriptions: {
                            typeIdentifier: "t_uint256",
                            typeString: "uint256",
                          },
                        },
                        functionReturnParameters: 7,
                        id: 11,
                        nodeType: "Return",
                        src: "164:14:0",
                      },
                    ],
                  },
                  functionSelector: "68ba353b",
                  id: 13,
                  implemented: true,
                  kind: "function",
                  modifiers: [],
                  name: "plus100",
                  nameLocation: "105:7:0",
                  nodeType: "FunctionDefinition",
                  parameters: {
                    id: 4,
                    nodeType: "ParameterList",
                    parameters: [
                      {
                        constant: false,
                        id: 3,
                        mutability: "mutable",
                        name: "a",
                        nameLocation: "121:1:0",
                        nodeType: "VariableDeclaration",
                        scope: 13,
                        src: "113:9:0",
                        stateVariable: false,
                        storageLocation: "default",
                        typeDescriptions: {
                          typeIdentifier: "t_uint256",
                          typeString: "uint256",
                        },
                        typeName: {
                          id: 2,
                          name: "uint256",
                          nodeType: "ElementaryTypeName",
                          src: "113:7:0",
                          typeDescriptions: {
                            typeIdentifier: "t_uint256",
                            typeString: "uint256",
                          },
                        },
                        visibility: "internal",
                      },
                    ],
                    src: "112:11:0",
                  },
                  returnParameters: {
                    id: 7,
                    nodeType: "ParameterList",
                    parameters: [
                      {
                        constant: false,
                        id: 6,
                        mutability: "mutable",
                        name: "",
                        nameLocation: "-1:-1:-1",
                        nodeType: "VariableDeclaration",
                        scope: 13,
                        src: "145:7:0",
                        stateVariable: false,
                        storageLocation: "default",
                        typeDescriptions: {
                          typeIdentifier: "t_uint256",
                          typeString: "uint256",
                        },
                        typeName: {
                          id: 5,
                          name: "uint256",
                          nodeType: "ElementaryTypeName",
                          src: "145:7:0",
                          typeDescriptions: {
                            typeIdentifier: "t_uint256",
                            typeString: "uint256",
                          },
                        },
                        visibility: "internal",
                      },
                    ],
                    src: "144:9:0",
                  },
                  scope: 14,
                  src: "96:89:0",
                  stateMutability: "pure",
                  virtual: false,
                  visibility: "public",
                },
              ],
              scope: 15,
              src: "72:115:0",
              usedErrors: [],
              usedEvents: [],
            },
          ],
          src: "46:142:0",
        },
        id: 0,
      },
    },
  },
};
