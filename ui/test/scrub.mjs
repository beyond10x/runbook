// A `reason:` value that opens with one capitalised word, a comma and a date ("Name, 2026-11-21, ...")
// names the person who took the decision; the committed fixture names the role instead.
export const OPERATOR = 'The operator';
const REASON_BY_NAME = /(\breason"?:\s*["']?)[A-Z][A-Za-z]*(?=, \d{4}-\d{2}-\d{2})/g;
export const scrubReasons = (text) => text.replace(REASON_BY_NAME, `$1${OPERATOR}`);
