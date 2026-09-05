import init, {
  check,
  checkWithProviderPacks,
  completion,
  completionWithProviderPacks,
  format,
  hover,
  render,
  renderWithProviderPacks,
  type CheckResult,
  type CompletionResult,
  type Diagnostic,
  type FormatResult,
  type HoverResult,
  type ProviderPackInput,
  type RenderResult,
  type StackSource,
} from "@stack-sh/engine";

const text: StackSource = 'stack 1.0 diagram "API" { node api "API" }';
const bytes: StackSource = new TextEncoder().encode(text);

const formatted: FormatResult = format(text);
const checked: CheckResult = check(bytes);
const rendered: RenderResult = render(text);
const diagnostic: Diagnostic | undefined = checked.diagnostics[0];
const providerPacks = JSON.parse("[]") as readonly ProviderPackInput[];
const providerChecked: CheckResult = checkWithProviderPacks(text, providerPacks);
const providerRendered: RenderResult = renderWithProviderPacks(bytes, providerPacks);
const position = { byteOffset: 0, line: 1, column: 1 } as const;
const completed: CompletionResult = completion(text, 1, position);
const providerCompleted: CompletionResult = completionWithProviderPacks(
  text,
  1,
  position,
  providerPacks,
);
const hovered: HoverResult = hover(text, 1, position);

formatted.formattedSource?.toUpperCase();
rendered.svg?.startsWith("<svg");
diagnostic?.range.start.byteOffset.toFixed(0);
diagnostic?.expected.join(", ");
providerChecked.metadata.engineVersion.toUpperCase();
providerRendered.providerNotices[0]?.packRevision.toUpperCase();
completed.items[0]?.edit.newText.toUpperCase();
providerCompleted.documentVersion.toFixed(0);
hovered.hover?.range.start.byteOffset.toFixed(0);

// @ts-expect-error Stack source is intentionally limited to string or Uint8Array.
format({ source: text });

// @ts-expect-error Language-intelligence positions require UTF-8 text, not bytes.
completion(bytes, 1, position);

void init;
