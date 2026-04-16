import * as fs from "fs";
import * as path from "path";

import { COVERAGE_LIBRARY_FILE_NAME } from "@nomicfoundation/edr";

export interface CoverageLib {
  content: string;
  filename: string;
}

export function getCoverageLibrary(): CoverageLib {
  const packageRoot = path.dirname(require.resolve("@nomicfoundation/edr"));
  const sourcePath = path.join(packageRoot, "coverage.sol");
  return {
    content: fs.readFileSync(sourcePath, "utf-8"),
    filename: COVERAGE_LIBRARY_FILE_NAME,
  };
}
