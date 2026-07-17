/**
 * Character handling for ESC/POS thermal printers (extracted from medusa-pos).
 *
 * - "ascii"  (default): NFD-normalizes, strips combining marks (so "Crème" →
 *   "Creme") and anything outside printable ASCII. Safe on any printer
 *   regardless of firmware. (Diverges from medusa-pos's item-title sanitizer,
 *   which also stripped ASCII punctuation — receipts legitimately need
 *   "()", "*", "%", …)
 * - "utf8":  only strips raw ESC/POS control bytes; all Unicode passes through.
 *   Requires printer firmware with UTF-8 support (needed for e.g. Georgian).
 * - "cp852": maps Central-European characters to ASCII lookalikes, then "?"
 *   for anything else non-ASCII.
 */
export type PrinterEncoding = "ascii" | "utf8" | "cp852";

// prettier-ignore
const CP852_MAP: Record<string, string> = {
  "ą": "a", "ć": "c", "ę": "e", "ł": "l", "ń": "n", "ó": "o", "ś": "s", "ź": "z", "ż": "z",
  "Ą": "A", "Ć": "C", "Ę": "E", "Ł": "L", "Ń": "N", "Ó": "O", "Ś": "S", "Ź": "Z", "Ż": "Z",
  "á": "a", "à": "a", "â": "a", "ä": "a", "ã": "a", "å": "a",
  "Á": "A", "À": "A", "Â": "A", "Ä": "A", "Ã": "A", "Å": "A",
  "é": "e", "è": "e", "ê": "e", "ë": "e", "É": "E", "È": "E", "Ê": "E", "Ë": "E",
  "í": "i", "ì": "i", "î": "i", "ï": "i", "Í": "I", "Ì": "I", "Î": "I", "Ï": "I",
  "ú": "u", "ù": "u", "û": "u", "ü": "u", "Ú": "U", "Ù": "U", "Û": "U", "Ü": "U",
  "ö": "o", "ô": "o", "ò": "o", "õ": "o", "ø": "o",
  "Ö": "O", "Ô": "O", "Ò": "O", "Õ": "O", "Ø": "O",
  "ñ": "n", "Ñ": "N", "ý": "y", "ÿ": "y", "Ý": "Y", "ß": "ss",
  "č": "c", "Č": "C", "š": "s", "Š": "S", "ž": "z", "Ž": "Z",
  "ř": "r", "Ř": "R", "ď": "d", "Ď": "D", "ť": "t", "Ť": "T",
  "ľ": "l", "Ľ": "L", "ĺ": "l", "Ĺ": "L", "ŕ": "r", "Ŕ": "R",
  "ě": "e", "Ě": "E", "ů": "u", "Ů": "U", "ő": "o", "Ő": "O", "ű": "u", "Ű": "U",
  "ā": "a", "Ā": "A", "ē": "e", "Ē": "E", "ī": "i", "Ī": "I", "ū": "u", "Ū": "U",
  "ģ": "g", "Ģ": "G", "ķ": "k", "Ķ": "K", "ļ": "l", "Ļ": "L", "ņ": "n", "Ņ": "N",
};

// eslint-disable-next-line no-control-regex
const ESCPOS_CONTROL_BYTES = /[\x00-\x08\x0B\x0C\x0E-\x1F\x7F]/g;

export function sanitizePrinterString(
  text: string,
  encoding: PrinterEncoding = "ascii",
  onUnmapped?: (char: string) => void
): string {
  if (!text) return "";

  if (encoding === "utf8") {
    return text.replace(ESCPOS_CONTROL_BYTES, "").replace(/\s+/g, " ").trim();
  }

  if (encoding === "cp852") {
    const mapped = text
      .split("")
      .map((ch) => {
        if (CP852_MAP[ch] !== undefined) return CP852_MAP[ch];
        if (ch.charCodeAt(0) > 127) {
          onUnmapped?.(ch);
          return "?";
        }
        return ch;
      })
      .join("");
    return mapped.replace(ESCPOS_CONTROL_BYTES, "").replace(/\s+/g, " ").trim();
  }

  // "ascii"
  const normalized = text.normalize("NFD");
  return normalized
    .replace(/[̀-ͯ]/g, "")
    .replace(/[^\x20-\x7E]/g, "")
    .replace(/\s+/g, " ")
    .trim();
}
