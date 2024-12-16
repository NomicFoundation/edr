import { EdrContext } from "../index";

describe("EdrContext", () => {
  it("EdrContext doesn't throw if initialized twice", () => {
    new EdrContext();
    new EdrContext();
  });
});
