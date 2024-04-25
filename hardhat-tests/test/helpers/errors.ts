import { assert, AssertionError } from "chai";

import { HardhatError } from "hardhat/internal/core/errors";
import { ErrorDescriptor } from "hardhat/internal/core/errors-list";

export async function expectErrorAsync(
  f: () => Promise<any>,
  errorMessage?: string | RegExp
) {
  const noError = new AssertionError("Async error expected but not thrown");
  const notExactMatch = new AssertionError(
    `Async error should have had message "${errorMessage}" but got "`
  );

  const notRegexpMatch = new AssertionError(
    `Async error should have matched regex ${errorMessage} but got "`
  );

  try {
    await f();
  } catch (err) {
    if (errorMessage === undefined) {
      return;
    }

    if (err instanceof Error) {
      if (typeof errorMessage === "string") {
        if (err.message !== errorMessage) {
          notExactMatch.message += `${err.message}"`;
          throw notExactMatch;
        }
      } else {
        if (errorMessage.exec(err.message) === null) {
          notRegexpMatch.message += `${err.message}"`;
          throw notRegexpMatch;
        }
      }
    }

    return;
  }

  throw noError;
}
