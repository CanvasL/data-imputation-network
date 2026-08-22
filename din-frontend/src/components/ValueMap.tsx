import { useMemo } from 'react';
import {
  ReactFlow,
  Background,
  Controls,
  MiniMap,
  Handle,
  Position,
  MarkerType,
  type Node,
  type Edge,
  type NodeProps,
} from '@xyflow/react';
import '@xyflow/react/dist/style.css';
import type { DatasetInfo, GraphData } from '../types';
import { layeredLayout } from '../layout';
import { colorForValue, scaleForValue } from '../colors';

// React Flow v12 requires node data to satisfy Record<string, unknown>,
// so the interface carries an index signature alongside its typed fields.
interface MapNodeData {
  schema: string;
  owner: string;
  version: number;
  value: number;
  [key: string]: unknown;
}

function ValueNode({ data, selected }: NodeProps<Node<MapNodeData>>) {
  const color = colorForValue(data.value);
  const scale = scaleForValue(data.value);
  return (
    <div
      className={`val-node${selected ? ' selected' : ''}`}
      style={{ borderColor: color, transform: `scale(${scale})` }}
    >
      <Handle type="target" position={Position.Top} className="val-handle" />
      <div className="val-node-owner">{data.owner}</div>
      <div className="val-node-schema">
        {data.schema}
        <span className="val-node-ver">v{data.version}</span>
      </div>
      <div className="val-node-value" style={{ background: color }}>
        {Math.round(data.value)}
      </div>
      <Handle type="source" position={Position.Bottom} className="val-handle" />
    </div>
  );
}

const nodeTypes = { value: ValueNode };

interface Props {
  datasets: DatasetInfo[];
  graph: GraphData;
  selectedId: string | null;
  onSelect: (id: string | null) => void;
}

export function ValueMap({ datasets, graph, selectedId, onSelect }: Props) {
  const byId = useMemo(() => new Map(datasets.map((d) => [d.id, d])), [datasets]);

  const layout = useMemo(
    () =>
      layeredLayout(
        graph.nodes.map((n) => n.id),
        graph.edges.map((e) => [e.parent, e.child] as [string, string]),
      ),
    [graph],
  );

  const nodes: Node<MapNodeData>[] = useMemo(
    () =>
      graph.nodes.map((n) => {
        const ds = byId.get(n.id);
        return {
          id: n.id,
          type: 'value',
          position: layout.get(n.id) ?? { x: 0, y: 0 },
          selected: n.id === selectedId,
          data: {
            schema: ds?.schema ?? 'unknown',
            owner: ds?.owner ?? '?',
            version: ds?.version ?? 1,
            value: ds?.value ?? 0,
          },
        };
      }),
    [graph, byId, layout, selectedId],
  );

  const edges: Edge[] = useMemo(
    () =>
      graph.edges.map((e, i) => ({
        id: `e-${i}`,
        source: e.parent,
        target: e.child,
        label: `${e.price}`,
        markerEnd: { type: MarkerType.ArrowClosed },
        style: { stroke: '#8b93a7', strokeWidth: 1.5 },
        labelStyle: { fill: '#aab2c5', fontSize: 12, fontWeight: 600 },
      })),
    [graph],
  );

  return (
    <ReactFlow
      key={`${graph.nodes.length}-${graph.edges.length}`}
      nodes={nodes}
      edges={edges}
      nodeTypes={nodeTypes}
      fitView
      fitViewOptions={{ padding: 0.15 }}
      onNodeClick={(_, n) => onSelect(n.id)}
      onPaneClick={() => onSelect(null)}
      proOptions={{ hideAttribution: true }}
      minZoom={0.2}
      maxZoom={2}
    >
      <Background gap={26} color="#1b2130" />
      <Controls />
      <MiniMap
        pannable
        zoomable
        nodeColor={(n) => colorForValue((n.data as MapNodeData).value)}
        maskColor="rgba(15,17,23,0.72)"
      />
    </ReactFlow>
  );
}
