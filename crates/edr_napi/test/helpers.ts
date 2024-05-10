import { TracingMessage, TracingMessageResult, TracingStep } from "..";

/**
  * Given a trace, return only its steps.
  */
export function collectSteps(
  trace: Array<TracingMessage | TracingStep | TracingMessageResult>
): TracingStep[] {
  return trace.filter((traceItem) => "pc" in traceItem) as TracingStep[];
}
