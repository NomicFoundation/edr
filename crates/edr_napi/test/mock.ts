import { JsonStreamStringify } from "json-stream-stringify";
import fs from "fs";
import { getContext } from "./helpers";

describe("Provider", () => {
  const context = getContext();

  it("issue 543", async function () {
    const fileContent = fs.readFileSync("test/data/issue-543.json", "utf-8");
    const parsedJson = JSON.parse(fileContent);
    const structLog = parsedJson.structLogs[0];

    // This creates a JSON of length ~950 000 000 characters.
    // JSON.stringify() crashes at ~500 000 000 characters.
    for (let i = 1; i < 20000; i++) {
      parsedJson.structLogs.push(structLog);
    }

    this.timeout(500_000);

    // Ignore this on testNoBuild
    // @ts-ignore
    const provider = context.createMockProvider(parsedJson);

    // This is a transaction that has a very large response.
    // It won't be used, the provider will return the mocked response.
    const debugTraceTransaction = `{
        "jsonrpc": "2.0",
        "method": "debug_traceTransaction",
        "params": ["0x7e460f200343e5ab6653a8857cc5ef798e3f5bea6a517b156f90c77ef311a57c"],
        "id": 1
      }`;

    const response = await provider.handleRequest(debugTraceTransaction);

    let responseData = response;

    if (typeof response.data === "string") {
      responseData = JSON.parse(response.data);
    } else {
      responseData = response.data;
    }

    // Validate that we can query the response data without crashing.
    const _json = new JsonStreamStringify(responseData);
  });
});
