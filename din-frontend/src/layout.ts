// Deterministic layered layout for the lineage DAG.
// Roots at the top (layer 0), derived datasets below. Uses longest-path
// layering, which matches the protocol's DAG semantics (no cycles).

export interface Point {
  x: number;
  y: number;
}

export function layeredLayout(
  nodeIds: string[],
  edges: [string, string][],
): Map<string, Point> {
  const layer = new Map<string, number>();
  const children = new Map<string, string[]>();
  const indeg = new Map<string, number>();

  for (const id of nodeIds) {
    layer.set(id, 0);
    children.set(id, []);
    indeg.set(id, 0);
  }
  for (const [p, c] of edges) {
    children.get(p)?.push(c);
    indeg.set(c, (indeg.get(c) ?? 0) + 1);
  }

  // Kahn's topological order; layer = longest path from a root.
  const queue = nodeIds.filter((id) => (indeg.get(id) ?? 0) === 0);
  const order: string[] = [];
  while (queue.length > 0) {
    const n = queue.shift()!;
    order.push(n);
    for (const c of children.get(n) ?? []) {
      layer.set(c, Math.max(layer.get(c)!, layer.get(n)! + 1));
      indeg.set(c, (indeg.get(c) ?? 0) - 1);
      if (indeg.get(c) === 0) queue.push(c);
    }
  }

  const maxLayer = nodeIds.reduce((m, id) => Math.max(m, layer.get(id) ?? 0), 0);
  const perLayer: string[][] = Array.from({ length: maxLayer + 1 }, () => []);
  for (const n of order) perLayer[layer.get(n) ?? 0].push(n);

  const GAP_X = 240;
  const GAP_Y = 190;
  const pos = new Map<string, Point>();
  perLayer.forEach((arr, li) => {
    const width = (arr.length - 1) * GAP_X;
    arr.forEach((id, i) => {
      pos.set(id, { x: -width / 2 + i * GAP_X, y: li * GAP_Y });
    });
  });
  return pos;
}
