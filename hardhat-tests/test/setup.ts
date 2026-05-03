import chai from "chai";
import chaiAsPromised from "chai-as-promised";
import chalk from "chalk";

chai.use(chaiAsPromised);

function getEnv(key: string): string | undefined {
  const variable = process.env[key];
  if (variable === undefined || variable === "") {
    return undefined;
  }

  const trimmed = variable.trim();

  return trimmed.length === 0 ? undefined : trimmed;
}

export const INFURA_URL = getEnv("INFURA_URL");
export const ALCHEMY_URL = getEnv("ALCHEMY_URL");

function printForkingLogicNotBeingTestedWarning(varName: string) {
  console.warn(
    chalk.yellow(
      `TEST RUN INCOMPLETE: You need to define the env variable ${varName}`
    )
  );
}

if (INFURA_URL === undefined) {
  printForkingLogicNotBeingTestedWarning("INFURA_URL");
}

if (ALCHEMY_URL === undefined) {
  printForkingLogicNotBeingTestedWarning("ALCHEMY_URL");
}

// Diagnostic for the recurring 6h CI hang: if the process is still alive a
// few seconds after the last test, dump active async resources and force-exit
// so CI surfaces *what* is keeping the event loop open (likely a leaked
// edr_napi handle / tokio runtime / HTTP socket).
after(function () {
  setTimeout(() => {
    const active =
      typeof (process as any).getActiveResourcesInfo === "function"
        ? (process as any).getActiveResourcesInfo()
        : [];
    // eslint-disable-next-line no-console
    console.error(
      `[mocha-exit-diagnostic] process still alive after suite; active resources (${active.length}): ${JSON.stringify(active)}`
    );
    process.exit(0);
  }, 5000).unref();
});
