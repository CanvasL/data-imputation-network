import { useCallback, useEffect, useMemo, useState } from 'react';
import { DinApi } from './api';
import type { DatasetInfo, GraphData } from './types';
import { ValueMap } from './components/ValueMap';
import { DatasetPanel } from './components/DatasetPanel';
import { EventPanel } from './components/EventPanel';

const DEFAULT_URL = 'http://127.0.0.1:8080';

export default function App() {
  const [nodeUrl, setNodeUrl] = useState(DEFAULT_URL);
  const [datasets, setDatasets] = useState<DatasetInfo[]>([]);
  const [graph, setGraph] = useState<GraphData>({ nodes: [], edges: [] });
  const [agents, setAgents] = useState<string[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [connected, setConnected] = useState<boolean | null>(null);
  const [error, setError] = useState<string | null>(null);

  const api = useMemo(() => new DinApi(nodeUrl), [nodeUrl]);

  const refresh = useCallback(async () => {
    try {
      const [ds, g] = await Promise.all([api.datasets(), api.graph()]);
      setDatasets(ds.datasets);
      setGraph(g);
      setConnected(true);
      setError(null);
    } catch (e) {
      setConnected(false);
      setError(e instanceof Error ? e.message : String(e));
    }
  }, [api]);

  useEffect(() => {
    refresh();
    const t = setInterval(refresh, 4000);
    return () => clearInterval(t);
  }, [refresh]);

  return (
    <div className="app">
      <header className="topbar">
        <div className="brand">
          <span className="brand-mark">DIN</span>
          <div>
            <div className="brand-title">数据协作网络</div>
            <div className="brand-sub">谁提供了数据、谁加工了数据、谁在为它付费 —— 一眼看清</div>
          </div>
        </div>
        <div className="topbar-right">
          <div className={`status ${connected === null ? '' : connected ? 'ok' : 'bad'}`}>
            <span className="status-dot" />
            {connected === null ? '连接中…' : connected ? '已连接' : '未连接'}
          </div>
          <input
            className="url-input"
            value={nodeUrl}
            onChange={(e) => setNodeUrl(e.target.value)}
            spellCheck={false}
            title="din-node 地址"
          />
          <button className="btn" onClick={refresh}>
            刷新
          </button>
        </div>
      </header>

      {error && <div className="banner err">{error}</div>}

      <div className="howto">
        <span><b>①</b> 创建身份</span>
        <i>→</i>
        <span><b>②</b> 提供数据 或 加工数据</span>
        <i>→</i>
        <span><b>③</b> 数据出现在地图上，价值从下游回流</span>
      </div>

      <div className="main">
        <section className="map-area">
          <ValueMap datasets={datasets} graph={graph} selectedId={selectedId} onSelect={setSelectedId} />
          <div className="map-legend">
            <span className="legend-item"><i style={{ background: 'rgb(94,129,244)' }} /> 低价值</span>
            <span className="legend-item"><i style={{ background: 'rgb(52,211,153)' }} /> 中</span>
            <span className="legend-item"><i style={{ background: 'rgb(251,191,36)' }} /> 高价值</span>
            <span className="legend-item sep">节点 = 数据集（颜色/大小 = 价值）</span>
            <span className="legend-item">边 = 付费引用（数字 = 订阅价）</span>
          </div>
        </section>
        <aside className="side">
          <DatasetPanel datasets={datasets} selectedId={selectedId} onSelect={setSelectedId} />
          <EventPanel
            api={api}
            datasets={datasets}
            agents={agents}
            onChanged={refresh}
            onAgentsChange={setAgents}
          />
        </aside>
      </div>
    </div>
  );
}
