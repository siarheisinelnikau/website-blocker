// Pure policy evaluation shared by the service worker and isolated Node tests.
const formatters = new Map();
const weekdays = { Mon: 0, Tue: 1, Wed: 2, Thu: 3, Fri: 4, Sat: 5, Sun: 6 };

export function wallTime(timezone, seconds) {
  if (!formatters.has(timezone)) {
    if (formatters.size >= 100) formatters.clear();
    formatters.set(timezone, new Intl.DateTimeFormat("en-US", {
      timeZone: timezone, weekday: "short", hour: "2-digit", minute: "2-digit", hourCycle: "h23"
    }));
  }
  const parts = Object.fromEntries(formatters.get(timezone).formatToParts(new Date(seconds * 1000)).map(p => [p.type, p.value]));
  return { day: weekdays[parts.weekday], minute: Number(parts.hour) * 60 + Number(parts.minute) };
}

export function scheduled(schedule, seconds) {
  const { day, minute } = wallTime(schedule.timezone, seconds);
  const start = schedule.start_minute, end = schedule.end_minute;
  if (start < end) return schedule.days.includes(day) && minute >= start && minute < end;
  return (schedule.days.includes(day) && minute >= start) ||
    (schedule.days.includes((day + 6) % 7) && minute < end);
}

export function validatePolicy(policy) {
  if (!policy || policy.protocol !== 1 || !Number.isSafeInteger(policy.revision) ||
      !Number.isSafeInteger(policy.generated_at) || !Number.isSafeInteger(policy.expires_at) ||
      policy.expires_at <= policy.generated_at || policy.expires_at - policy.generated_at > 8 * 86400 ||
      !Number.isSafeInteger(policy.pause_until) || !Array.isArray(policy.groups) || policy.groups.length > 20 ||
      !Array.isArray(policy.allow_regexes) || policy.allow_regexes.length > 50) throw new Error("Invalid companion policy.");
  const regexes = [...policy.allow_regexes];
  const ids = new Set();
  for (const group of policy.groups) {
    if (!Number.isSafeInteger(group.id) || group.id < 1 || ids.has(group.id) ||
        typeof group.name !== "string" || group.name.length > 80 ||
        !Array.isArray(group.regexes) || group.regexes.length < 1 || group.regexes.length > 50 ||
        !Array.isArray(group.schedules) || group.schedules.length > 14) throw new Error("Invalid rule set.");
    ids.add(group.id);
    regexes.push(...group.regexes);
    for (const s of group.schedules) {
      if (!Array.isArray(s.days) || !s.days.length || s.days.some(d => !Number.isInteger(d) || d < 0 || d > 6) ||
          !Number.isInteger(s.start_minute) || !Number.isInteger(s.end_minute) || s.start_minute < 0 ||
          s.end_minute < 0 || s.start_minute > 1439 || s.end_minute > 1439 || s.start_minute === s.end_minute ||
          typeof s.timezone !== "string" || s.timezone.length > 80) throw new Error("Invalid schedule.");
      wallTime(s.timezone, policy.generated_at); // Intl rejects unknown IANA zones.
    }
  }
  if (policy.focus !== null && policy.focus !== undefined &&
      (!ids.has(policy.focus.group_id) || !Number.isSafeInteger(policy.focus.end))) throw new Error("Invalid focus timer.");
  if (regexes.length > 1000) throw new Error("Too many browser regex rules.");
  for (const regex of regexes) {
    if (typeof regex !== "string" || regex.length > 2048 || !regex.startsWith("^https?://")) throw new Error("Invalid website matcher.");
    new RegExp(regex);
  }
  return policy;
}

export function activeGroups(policy, seconds) {
  if (!policy || seconds >= policy.expires_at || seconds < policy.generated_at - 60 || seconds < policy.pause_until) return [];
  return policy.groups.filter(g => (policy.focus?.group_id === g.id && seconds < policy.focus.end) ||
    g.schedules.some(s => scheduled(s, seconds)));
}

export function compileRules(policy, seconds) {
  const active = activeGroups(policy, seconds);
  if (!active.length) return [];
  let id = 1;
  const condition = regexFilter => ({ regexFilter, isUrlFilterCaseSensitive: true, resourceTypes: ["main_frame"] });
  return [
    ...policy.allow_regexes.map(regex => ({ id: id++, priority: 100, action: { type: "allow" }, condition: condition(regex) })),
    ...active.flatMap(group => group.regexes.map(regex => ({ id: id++, priority: 1,
      action: { type: "redirect", redirect: { extensionPath: "/blocked.html" } }, condition: condition(regex) })))
  ];
}

export function nextBoundary(policy, seconds) {
  // Wake at the next wall-clock minute, or a precise focus/break/expiry boundary.
  const boundaries = [(Math.floor(seconds / 60) + 1) * 60];
  if (policy) boundaries.push(policy.pause_until, policy.focus?.end, policy.expires_at);
  return Math.min(...boundaries.filter(value => Number.isFinite(value) && value > seconds));
}
