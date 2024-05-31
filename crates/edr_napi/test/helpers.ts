import { TracingMessage, TracingMessageResult, TracingStep } from "..";

/**
 * Given a trace, return only its steps.
 */
export function collectSteps(
  trace: Array<TracingMessage | TracingStep | TracingMessageResult>,
): TracingStep[] {
  return trace.filter((traceItem) => "pc" in traceItem) as TracingStep[];
}

/**
 * Given a trace, return only its messages.
 */
export function collectMessages(
  trace: Array<TracingMessage | TracingStep | TracingMessageResult>,
): TracingMessage[] {
  return trace.filter(
    (traceItem) => "isStaticCall" in traceItem,
  ) as TracingMessage[];
}
