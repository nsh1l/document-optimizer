import { test } from 'node:test';
import assert from 'node:assert/strict';
import { suggestedOutput, runQueue, filterSupportedPaths } from '../src/queue.ts';
test('file drops accept supported document extensions case-insensitively', () => {
  const result = filterSupportedPaths(['C:/docs/a.PPTX', 'C:\\\\資料\\\\b.xlsx', 'C:/docs/readme.txt']);
  assert.deepEqual(result.accepted, ['C:/docs/a.PPTX', 'C:\\\\資料\\\\b.xlsx']);
  assert.deepEqual(result.unsupported, ['C:/docs/readme.txt']);
});

const result = { before: 100, after: 80, saved: 20, unchanged: false };
const jobs = () => ['a', 'b', 'c'].map(id => ({id, input: `${id}.xlsx`, output: `${id}_optimized.xlsx`, status: 'pending'}));
test('outputs keep extension and never default to the source', () => {
  assert.equal(suggestedOutput('C:\\資料\\quarter.v2.XLSX'), 'C:\\資料\\quarter.v2_optimized.XLSX');
  assert.equal(suggestedOutput('C:/docs/a.docx', 'D:/out'), 'D:/out/a_optimized.docx');
});
test('cancellation finishes active job and explicitly cancels the remaining queue', async () => {
  let stop = false; const calls = []; const states = {};
  await runQueue(jobs(), async job => { calls.push(job.id); stop = true; return result; }, (id, patch) => { states[id] = {...states[id], ...patch}; }, () => stop);
  assert.deepEqual(calls, ['a']); assert.equal(states.a.status, 'done'); assert.equal(states.b.status, 'cancelled'); assert.equal(states.c.status, 'cancelled');
});
test('errors do not abort later jobs and finished jobs are not repeated', async () => {
  const input = jobs(); input[2].status = 'done'; const calls = []; const states = {};
  await runQueue(input, async job => { calls.push(job.id); if (job.id === 'a') throw new Error('exists'); return {...result, unchanged: true}; }, (id, patch) => { states[id] = {...states[id], ...patch}; }, () => false);
  assert.deepEqual(calls, ['a', 'b']); assert.equal(states.a.status, 'error'); assert.match(states.a.error, /exists/); assert.equal(states.b.status, 'unchanged');
});
