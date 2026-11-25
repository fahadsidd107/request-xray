'use strict';

const element = id => document.getElementById(id);
let cursor = null;
let spanCursor = null;
let selected = null;
let generation = 0;
let listGeneration = 0;

async function get(path, next) {
  const response = await fetch(path + (next ? '?cursor=' + encodeURIComponent(next) : ''));
  if (!response.ok) throw new Error((await response.json()).error.message);
  return response.json();
}

function append(list, text, clicked) {
  const row = document.createElement('li');
  if (clicked) {
    const button = document.createElement('button');
    button.textContent = text;
    button.addEventListener('click', clicked);
    row.append(button);
  } else {
    row.textContent = text;
  }
  list.append(row);
}

async function loadTraces(reset) {
  const current = ++listGeneration;
  if (reset) {
    cursor = null;
    element('traces').replaceChildren();
    element('details').hidden = true;
    generation++;
  }
  element('more').disabled = true;
  try {
    const page = await get('/api/v1/traces', cursor);
    if (current !== listGeneration) return;
    cursor = page.nextCursor;
    element('more').hidden = !cursor;
    element('traces').replaceChildren();
    for (const trace of page.items) {
      const label = trace.traceId + ' · ' + trace.spanCount + ' spans · ' + trace.durationNs + ' ns · ' + trace.status;
      append(element('traces'), label, () => loadSpans(trace.traceId, true));
    }
    element('status').textContent = element('traces').children.length ? '' : 'No traces yet. Send OTLP spans to the collector, then refresh.';
  } catch (error) {
    if (current === listGeneration) element('status').textContent = error.message;
  } finally {
    if (current === listGeneration) element('more').disabled = false;
  }
}

async function loadSpans(traceId, reset) {
  const current = ++generation;
  if (reset) {
    selected = traceId;
    spanCursor = null;
    element('spans').replaceChildren();
    element('details').hidden = false;
  }
  element('more-spans').disabled = true;
  try {
    const page = await get('/api/v1/traces/' + encodeURIComponent(traceId) + '/spans', spanCursor);
    if (current !== generation) return;
    spanCursor = page.nextCursor;
    element('more-spans').hidden = !spanCursor;
    element('spans').replaceChildren();
    for (const span of page.items) append(element('spans'), span.name + ' · ' + span.durationNs + ' ns');
  } catch (error) {
    if (current === generation) append(element('spans'), error.message);
  } finally {
    if (current === generation) element('more-spans').disabled = false;
  }
}

if (document.body.dataset.mode === 'demo') element('context').textContent = 'Demo · synthetic trace data';
element('refresh').addEventListener('click', () => loadTraces(true));
element('more').addEventListener('click', () => loadTraces(false));
element('more-spans').addEventListener('click', () => loadSpans(selected, false));
loadTraces(true);
