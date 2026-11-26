// The page's clock words: a time of day in 24 h form, in the browser's own zone (or `timeZone`), with the zone's name
// (`19:54:50 CET`), and the date in ISO form in front when the time is not of the day `now` is in (`2026-11-25 19:54:50
// CET`). The words do not depend on the browser's language: an en-US browser says CET for Central Europe, not GMT+1.

const formats = new Map();
const format = (locale, options) => {
  const key = `${locale}|${JSON.stringify(options)}`;
  if (!formats.has(key)) formats.set(key, new Intl.DateTimeFormat(locale, options));
  return formats.get(key);
};
const partsOf = (d, timeZone) => Object.fromEntries(format('en-CA', {timeZone, year: 'numeric', month: '2-digit', day: '2-digit', hour: '2-digit', minute: '2-digit', second: '2-digit', hourCycle: 'h23'}).formatToParts(d).map((p) => [p.type, p.value]));

// The zone's short name at `date`: the first of the en-US and en-GB names that is a name (EST, CET, BST), else the
// offset as Intl gives it (GMT+9, GMT+5:30).
export function zoneName(date, timeZone) {
  let offset = null;
  for (const locale of ['en-US', 'en-GB']) {
    const name = format(locale, {timeZone, timeZoneName: 'short'}).formatToParts(date).find((p) => p.type === 'timeZoneName')?.value;
    if (!name) continue;
    if (!/^GMT[+-−]/.test(name)) return name;
    offset ??= name;
  }
  return offset ?? '';
}

// `19:54:50 CET`, or `2026-11-25 19:54:50 CET` when `ms` lies on another day than `now`; '' for no time.
export function clockTime(ms, now = Date.now(), timeZone = undefined) {
  if (ms == null || Number.isNaN(ms)) return '';
  const d = new Date(ms);
  const p = partsOf(d, timeZone), n = partsOf(new Date(now), timeZone);
  const day = `${p.year}-${p.month}-${p.day}`;
  const time = `${p.hour}:${p.minute}:${p.second} ${zoneName(d, timeZone)}`.trim();
  return day === `${n.year}-${n.month}-${n.day}` ? time : `${day} ${time}`;
}

// The ISO date and the time, always (a title or a tooltip that names one moment in full).
export function fullTime(ms, timeZone = undefined) {
  if (ms == null || Number.isNaN(ms)) return '';
  const p = partsOf(new Date(ms), timeZone);
  return `${p.year}-${p.month}-${p.day} ${p.hour}:${p.minute}:${p.second} ${zoneName(new Date(ms), timeZone)}`.trim();
}
