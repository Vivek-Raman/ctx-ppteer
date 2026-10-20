export type RetentionUnit = 'minutes' | 'hours';

export function retentionMinutes(value: number, unit: RetentionUnit): number {
  return Math.round(value * (unit === 'hours' ? 60 : 1));
}
export function needsShorterRetentionConfirmation(
  next: number | null,
  applied: number | null,
): boolean {
  return next !== null && applied !== null && next < applied;
}

export type RetentionDraft = {
  enabled: boolean;
  value: number;
  unit: RetentionUnit;
  applied: number | null;
  dirty: boolean;
  confirmation: boolean;
};

export type RetentionSave = { minutes: number | null; revision: number };

export function retentionDraft(minutes: number | null): RetentionDraft {
  if (minutes === null)
    return {
      enabled: false,
      value: 1,
      unit: 'hours',
      applied: null,
      dirty: false,
      confirmation: false,
    };
  const unit = minutes % 60 === 0 ? 'hours' : 'minutes';
  return {
    enabled: true,
    value: minutes / (unit === 'hours' ? 60 : 1),
    unit,
    applied: minutes,
    dirty: false,
    confirmation: false,
  };
}

export function draftMinutes(
  draft: Pick<RetentionDraft, 'enabled' | 'value' | 'unit'>,
): number | null {
  return draft.enabled ? retentionMinutes(Number(draft.value), draft.unit) : null;
}

export function editRetentionDraft(
  draft: RetentionDraft,
  changes: Partial<Pick<RetentionDraft, 'enabled' | 'value' | 'unit'>>,
): RetentionDraft {
  const next = { ...draft, ...changes, dirty: true };
  return {
    ...next,
    confirmation: needsShorterRetentionConfirmation(draftMinutes(next), next.applied),
  };
}

export function reconcileRetentionDraft(
  draft: RetentionDraft,
  baseline: number | null,
): RetentionDraft {
  if (!draft.dirty) return retentionDraft(baseline);
  return {
    ...draft,
    applied: baseline,
    confirmation: needsShorterRetentionConfirmation(draftMinutes(draft), baseline),
  };
}

export function beginRetentionSave(draft: RetentionDraft, revision: number): RetentionSave {
  return { minutes: draftMinutes(draft), revision };
}

export function completeRetentionSave(
  draft: RetentionDraft,
  save: RetentionSave,
  currentRevision: number,
): RetentionDraft {
  if (save.revision !== currentRevision) return draft;
  return { ...draft, applied: save.minutes, dirty: false, confirmation: false };
}
