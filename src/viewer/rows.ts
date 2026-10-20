import type { StatusEntry, StatusPoint } from '../shared/types';

type Row =
  | { key: string; kind: 'project'; folder: string }
  | { key: string; kind: 'branch'; branch: string }
  | {
      key: string;
      kind: 'point';
      entry: StatusEntry;
      point: StatusPoint;
      pointIndex: number;
      last: boolean;
    };
export type ViewerRow = Row;

function pointIdentity(point: StatusPoint) {
  if (point.id) return `id:${point.id}`;
  return JSON.stringify([point.submitted_at ?? null, point.text, point.done, point.link]);
}

export function pointKey(entry: StatusEntry, point: StatusPoint, pointIndex: number) {
  if (point.id)
    return JSON.stringify(['point', entry.folder_name, entry.session_id, entry.branch, point.id]);
  const identity = pointIdentity(point);
  // A write assigns the same timestamp to multiple points. Include content and
  // an occurrence number so even identical points have distinct, stable keys.
  const occurrence = entry.points
    .slice(0, pointIndex)
    .filter((other) => pointIdentity(other) === identity).length;
  return JSON.stringify([
    'point',
    entry.folder_name,
    entry.session_id,
    entry.branch,
    identity,
    occurrence,
  ]);
}

// All moving rows must be siblings. Animating both a group and its children
// applies the same viewport displacement more than once.
export function viewerRows(entries: StatusEntry[], projects: string[]): Row[] {
  return projects.flatMap((folder) => {
    const projectEntries = entries.filter((entry) => entry.folder_name === folder);
    const rows: Row[] = [{ key: JSON.stringify(['project', folder]), kind: 'project', folder }];
    for (const branch of new Set(projectEntries.map((entry) => entry.branch))) {
      rows.push({ key: JSON.stringify(['branch', folder, branch]), kind: 'branch', branch });
      const points = projectEntries
        .filter((entry) => entry.branch === branch)
        .flatMap((entry) =>
          entry.points.map((point, pointIndex) => ({ entry, point, pointIndex })),
        );
      points.forEach((item, index) =>
        rows.push({
          key: pointKey(item.entry, item.point, item.pointIndex),
          kind: 'point',
          ...item,
          last: index === points.length - 1,
        }),
      );
    }
    return rows;
  });
}

export function projectNames(
  entries: StatusEntry[],
  order: 'project-directory-asc' | 'updated-at-desc' | 'updated-at-asc',
) {
  const latest = new Map<string, number>();
  for (const entry of entries)
    latest.set(
      entry.folder_name,
      Math.max(latest.get(entry.folder_name) ?? -Infinity, Date.parse(entry.updated_at)),
    );
  return [...latest.keys()].sort((left, right) => {
    if (order === 'project-directory-asc')
      return left.toLowerCase().localeCompare(right.toLowerCase());
    const difference = (latest.get(left) ?? 0) - (latest.get(right) ?? 0);
    return difference
      ? order === 'updated-at-asc'
        ? difference
        : -difference
      : left.toLowerCase().localeCompare(right.toLowerCase());
  });
}

export function relativeTime(value: string, now: number) {
  const minutes = Math.max(0, Math.floor((now - Date.parse(value)) / 60_000));
  if (!minutes) return 'now';
  if (minutes < 60) return `${minutes}m`;
  const hours = Math.floor(minutes / 60);
  const remaining = minutes % 60;
  if (hours < 24) return `${hours}h${remaining ? ` ${remaining}m` : ''}`;
  const days = Math.floor(hours / 24);
  const remainingHours = hours % 24;
  return `${days}d${remainingHours ? ` ${remainingHours}h` : ''}`;
}
