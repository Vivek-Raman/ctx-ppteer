import test from 'node:test';
import assert from 'node:assert/strict';
import {
  beginRetentionSave,
  completeRetentionSave,
  draftMinutes,
  editRetentionDraft,
  needsShorterRetentionConfirmation,
  reconcileRetentionDraft,
  retentionDraft,
} from '../src/settings/retention.ts';

test('retention draft keeps a disabled baseline and can enable without mutating settings', () => {
  const draft = retentionDraft(null);
  assert.equal(draft.enabled, false);
  assert.equal(draftMinutes({ ...draft, enabled: true, value: 2, unit: 'hours' }), 120);
  assert.equal(draft.applied, null);
});

test('shorter retention requires confirmation against the applied baseline', () => {
  const draft = retentionDraft(120);
  assert.equal(needsShorterRetentionConfirmation(60, draft.applied), true);
  assert.equal(needsShorterRetentionConfirmation(120, draft.applied), false);
});

test('dirty draft survives unrelated settings updates and tracks the new baseline', () => {
  const draft = {
    ...retentionDraft(120),
    enabled: true,
    value: 30,
    unit: 'minutes' as const,
    dirty: true,
  };
  const reconciled = reconcileRetentionDraft(draft, 240);
  assert.equal(draftMinutes(reconciled), 30);
  assert.equal(reconciled.applied, 240);
  assert.equal(reconciled.confirmation, true);
});

test('clean draft resyncs after an external retention update', () => {
  const reconciled = reconcileRetentionDraft(retentionDraft(120), 30);
  assert.equal(reconciled.value, 30);
  assert.equal(reconciled.unit, 'minutes');
  assert.equal(reconciled.dirty, false);
  assert.equal(reconciled.applied, 30);
});

test('edits during a save remain dirty and an old completion cannot clean them', () => {
  const initial = retentionDraft(120);
  const submitted = editRetentionDraft(initial, { value: 60, unit: 'minutes' });
  const save = beginRetentionSave(submitted, 1);
  const newerDraft = editRetentionDraft(submitted, { value: 30 });
  assert.equal(completeRetentionSave(newerDraft, save, 2).dirty, true);
  assert.equal(save.minutes, 60);
  assert.equal(newerDraft.value, 30);
});

test('reset invalidates an in-flight save and failure leaves the draft dirty', () => {
  const draft = editRetentionDraft(retentionDraft(120), { value: 30, unit: 'minutes' });
  const save = beginRetentionSave(draft, 4);
  const reset = retentionDraft(120);
  assert.equal(reset.dirty, false);
  assert.equal(save.revision, 4);
  assert.equal(draft.dirty, true);
});
