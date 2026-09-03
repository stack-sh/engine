import init, {
  check,
  format,
  render,
  type CheckResult,
  type Diagnostic,
  type FormatResult,
  type RenderResult,
  type StackSource,
} from "@stack-sh/engine";

const text: StackSource = 'stack 1.0 diagram "API" { node api "API" }';
const bytes: StackSource = new TextEncoder().encode(text);

const formatted: FormatResult = format(text);
const checked: CheckResult = check(bytes);
const rendered: RenderResult = render(text);
const diagnostic: Diagnostic | undefined = checked.diagnostics[0];

formatted.formattedSource?.toUpperCase();
rendered.svg?.startsWith("<svg");
diagnostic?.range.start.byteOffset.toFixed(0);

// @ts-expect-error Stack source is intentionally limited to string or Uint8Array.
format({ source: text });

void init;
