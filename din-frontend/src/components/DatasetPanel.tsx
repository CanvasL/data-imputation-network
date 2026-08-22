import type { DatasetInfo } from '../types';
import { colorForValue, shorten } from '../colors';

interface Props {
  datasets: DatasetInfo[];
  selectedId: string | null;
  onSelect: (id: string | null) => void;
}

export function DatasetPanel({ datasets, selectedId, onSelect }: Props) {
  const sorted = [...datasets].sort((a, b) => b.value - a.value);
  const selected = datasets.find((d) => d.id === selectedId);

  return (
    <div className="panel">
      <div className="panel-title">
        网络中的数据 <span className="count">{datasets.length}</span>
      </div>

      {selected && (
        <div className="detail-card">
          <div className="detail-row">
            <span>名称 / 类型</span>
            <code>{selected.schema}</code>
          </div>
          <div className="detail-row">
            <span>提供者</span>
            <code>{selected.owner}</code>
          </div>
          <div className="detail-row">
            <span>版本</span>
            <code>v{selected.version}</code>
          </div>
          <div className="detail-row">
            <span>当前价值</span>
            <b style={{ color: colorForValue(selected.value) }}>{selected.value.toFixed(2)}</b>
          </div>
          <div className="detail-row">
            <span>标识</span>
            <code className="mono small">{shorten(selected.id)}</code>
          </div>
        </div>
      )}

      <ul className="dataset-list">
        {sorted.map((d) => (
          <li
            key={d.id}
            className={d.id === selectedId ? 'active' : ''}
            onClick={() => onSelect(d.id)}
          >
            <span className="dot" style={{ background: colorForValue(d.value) }} />
            <span className="ds-schema">
              {d.schema} <em>· {d.owner}</em>
            </span>
            <span className="ds-value">{Math.round(d.value)}</span>
          </li>
        ))}
        {sorted.length === 0 && <li className="empty">还没有数据 —— 在右侧「参与网络」里提供或加工</li>}
      </ul>
    </div>
  );
}
