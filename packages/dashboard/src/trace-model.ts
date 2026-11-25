import type { Span } from './api';

type Gap = 'Missing parent' | 'Parent outside loaded spans' | 'Possible clock skew' | 'Parent cycle';

interface Node {
  span: Span;
  children: Node[];
  gap?: Gap;
}

export interface Row {
  node: Node;
  depth: number;
}

const kinds: Record<number, string> = {
  0: 'Unspecified', 1: 'Internal', 2: 'Server', 3: 'Client', 4: 'Producer', 5: 'Consumer',
};

export function kind(span: Span) {
  return kinds[span.kind] ?? `Unknown (${span.kind})`;
}

export function service(span: Span) {
  const attribute = span.resource?.attributes.find(entry => entry.key === 'service.name')?.value;
  return attribute?.type === 'string' && attribute.value ? attribute.value : 'Unknown service';
}

export function duration(ns: bigint | string): string {
  const value = BigInt(ns);
  if (value < 0n) return '−' + duration(-value);
  const [scale, unit] = value >= 1_000_000_000n ? [1_000_000_000n, 's']
    : value >= 1_000_000n ? [1_000_000n, 'ms'] : value >= 1000n ? [1000n, 'µs'] : [1n, 'ns'];
  const fractional = scale === 1n ? '' : ((value % scale) * 1000n / scale).toString().padStart(3, '0').replace(/0+$/, '');
  return `${value / scale}${fractional ? '.' + fractional : ''} ${unit}`;
}

export function clock(ns: string) {
  const instant = BigInt(ns);
  const date = new Date(Number(instant / 1_000_000_000n) * 1000);
  return date.toISOString().slice(0, -5) + '.' + (instant % 1_000_000_000n).toString().padStart(9, '0') + 'Z';
}

export function tree(spans: Span[], partial: boolean) {
  const nodes = new Map(spans.map(span => [span.spanId, { span, children: [] } as Node]));
  const complete = new Set<string>();
  const cycles = new Set<string>();
  const roots: Node[] = [];

  for (const span of spans) {
    const chain = new Set<string>();
    let id: string | null = span.spanId;
    while (id && !complete.has(id)) {
      if (chain.has(id)) {
        const ordered = [...chain];
        for (const member of ordered.slice(ordered.indexOf(id))) cycles.add(member);
        break;
      }
      chain.add(id);
      id = nodes.get(id)?.span.parentSpanId ?? null;
    }
    for (const member of chain) complete.add(member);
  }

  for (const node of nodes.values()) {
    const { span } = node;
    const parent = span.parentSpanId ? nodes.get(span.parentSpanId) : undefined;
    if (cycles.has(span.spanId)) {
      node.gap = 'Parent cycle';
      roots.push(node);
    } else if (parent) {
      parent.children.push(node);
      if (service(span) !== service(parent.span) && service(span) !== 'Unknown service'
          && service(parent.span) !== 'Unknown service' && BigInt(span.startedAtUnixNs) < BigInt(parent.span.startedAtUnixNs)) {
        node.gap = 'Possible clock skew';
      }
    } else {
      if (span.parentSpanId) node.gap = partial ? 'Parent outside loaded spans' : 'Missing parent';
      roots.push(node);
    }
  }
  return { roots, nodes };
}

export function flatten(roots: Node[], collapsed: ReadonlySet<string>) {
  const rows: Row[] = [];
  const stack = roots.map(node => ({ node, depth: 0 })).reverse();
  while (stack.length) {
    const row = stack.pop()!;
    rows.push(row);
    if (collapsed.has(row.node.span.spanId)) continue;
    for (let index = row.node.children.length - 1; index >= 0; index--) {
      stack.push({ node: row.node.children[index]!, depth: row.depth + 1 });
    }
  }
  return rows;
}
