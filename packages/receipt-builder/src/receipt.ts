/**
 * Fixed-width receipt text builder for ESC/POS printers (ported from
 * medusa-pos and generalized).
 *
 * The input is a fully host-composed document: the host decides which lines
 * exist (VAT math, tender splitting, i18n labels, date formatting) and this
 * module only lays them out for a given paper width. Amounts are opaque
 * numbers rendered through the host's `formatAmount` — integer minor units
 * and decimal majors both work.
 */
import { sanitizePrinterString, type PrinterEncoding } from "./encoding";

export type PaperWidth = "80mm" | "57mm";

export const PAPER_CONFIG: Record<
  PaperWidth,
  { lineWidth: number; maxItemTitleLen: number }
> = {
  "80mm": { lineWidth: 48, maxItemTitleLen: 30 },
  "57mm": { lineWidth: 32, maxItemTitleLen: 18 },
};

/** One priced line in a totals/payments block: `label ......... amount`. */
export interface MoneyRow {
  label: string;
  amount: number;
}

/** Extra line under an item: modifier, per-item discount, note. */
export interface ItemSubline {
  text: string;
  amount?: number;
}

export interface ReceiptItem {
  title: string;
  qty: number;
  unitPrice: number;
  /** Defaults to `unitPrice * qty` (fine for integer minor units). */
  total?: number;
  sublines?: ItemSubline[];
}

export interface ReceiptDoc {
  /** Venue identity block, centered: name, address, phone… */
  headerLines: string[];
  /** Centered document title between separators, e.g. "SALES RECEIPT". */
  title?: string;
  /** `label: value` rows: date, order ref, table, server… */
  metaRows?: { label: string; value: string }[];
  itemsHeading?: string;
  items: ReceiptItem[];
  /** Subtotal/discount/VAT/total rows — the host decides which exist. */
  totalRows: MoneyRow[];
  /** Tenders, tips, change, amount due. */
  paymentRows?: MoneyRow[];
  /** Centered emphasis lines, e.g. "** UNPAID **". */
  messages?: string[];
  /** Centered footer block, e.g. "Thank you for your visit!". */
  footerLines?: string[];
}

export interface BuildOptions {
  formatAmount: (amount: number) => string;
  paperWidth?: PaperWidth;
  encoding?: PrinterEncoding;
}

/** `left ... right` padded to the line width (minimum one space between). */
export function padLine(left: string, right: string, totalWidth: number): string {
  const padding = totalWidth - left.length - right.length;
  return left + " ".repeat(Math.max(1, padding)) + right;
}

export function centerText(text: string, totalWidth: number): string {
  const padding = Math.floor((totalWidth - text.length) / 2);
  return " ".repeat(Math.max(0, padding)) + text;
}

/** Greedy word wrap; words longer than `width` are hard-broken. */
export function wrapText(text: string, width: number): string[] {
  const words = text.split(/\s+/).filter(Boolean);
  const lines: string[] = [];
  let current = "";
  for (let word of words) {
    while (word.length > width) {
      if (current) {
        lines.push(current);
        current = "";
      }
      lines.push(word.slice(0, width));
      word = word.slice(width);
    }
    if (!current) current = word;
    else if (current.length + 1 + word.length <= width) current += " " + word;
    else {
      lines.push(current);
      current = word;
    }
  }
  if (current) lines.push(current);
  return lines;
}

export function buildReceiptText(doc: ReceiptDoc, opts: BuildOptions): string {
  const paperWidth = opts.paperWidth ?? "80mm";
  const encoding = opts.encoding ?? "ascii";
  const { lineWidth, maxItemTitleLen } = PAPER_CONFIG[paperWidth];
  const fmt = opts.formatAmount;
  const clean = (s: string) => sanitizePrinterString(s, encoding);

  const separator = "=".repeat(lineWidth);
  const thinSeparator = "-".repeat(lineWidth);
  const lines: string[] = [];

  for (const line of doc.headerLines) {
    lines.push(centerText(clean(line), lineWidth));
  }

  if (doc.title) {
    lines.push("", separator, centerText(clean(doc.title), lineWidth), separator);
  }

  for (const row of doc.metaRows ?? []) {
    lines.push(padLine(`${clean(row.label)}:`, clean(row.value), lineWidth));
  }

  lines.push("", thinSeparator);
  if (doc.itemsHeading) lines.push(`${clean(doc.itemsHeading)}:`);
  lines.push(thinSeparator);

  for (const item of doc.items) {
    const total = item.total ?? item.unitPrice * item.qty;
    const title = clean(item.title).substring(0, maxItemTitleLen);
    lines.push(padLine(title, fmt(total), lineWidth));
    lines.push(`  ${item.qty} x ${fmt(item.unitPrice)}`);
    for (const sub of item.sublines ?? []) {
      const text = `  ${clean(sub.text)}`;
      lines.push(
        sub.amount !== undefined
          ? padLine(text.substring(0, maxItemTitleLen + 2), fmt(sub.amount), lineWidth)
          : text
      );
    }
  }

  lines.push(thinSeparator);
  for (const row of doc.totalRows) {
    lines.push(padLine(`${clean(row.label)}:`, fmt(row.amount), lineWidth));
  }

  if (doc.paymentRows?.length) {
    lines.push("");
    for (const row of doc.paymentRows) {
      lines.push(padLine(`${clean(row.label)}:`, fmt(row.amount), lineWidth));
    }
  }

  for (const message of doc.messages ?? []) {
    lines.push("", centerText(clean(message), lineWidth));
  }

  if (doc.footerLines?.length) {
    lines.push("");
    for (const line of doc.footerLines) {
      for (const wrapped of wrapText(clean(line), lineWidth)) {
        lines.push(centerText(wrapped, lineWidth));
      }
    }
  }

  lines.push("", separator, "");
  return lines.join("\n");
}
