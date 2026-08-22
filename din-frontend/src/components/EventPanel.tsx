import { useState } from 'react';
import type { DatasetInfo } from '../types';
import { DinApi } from '../api';
import { shorten } from '../colors';

interface Props {
  api: DinApi;
  datasets: DatasetInfo[];
  agents: string[];
  onChanged: () => void;
  onAgentsChange: (agents: string[]) => void;
}

type Tab = 'register' | 'contribute' | 'subscribe';

interface SourceRow {
  id: string;
  price: string;
  alpha: string;
}

export function EventPanel({ api, datasets, agents, onChanged, onAgentsChange }: Props) {
  const [tab, setTab] = useState<Tab>('register');
  const [busy, setBusy] = useState(false);
  const [msg, setMsg] = useState<{ kind: 'ok' | 'err'; text: string } | null>(null);

  // register
  const [newAgent, setNewAgent] = useState('');
  // contribute
  const [cAgent, setCAgent] = useState('');
  const [cSchema, setCSchema] = useState('');
  const [cContent, setCContent] = useState('');
  // subscribe
  const [sAgent, setSAgent] = useState('');
  const [sSchema, setSSchema] = useState('');
  const [sContent, setSContent] = useState('');
  const [sRevenue, setSRevenue] = useState('0');
  const [sources, setSources] = useState<SourceRow[]>([{ id: '', price: '100', alpha: '1' }]);

  async function run(fn: () => Promise<unknown>, okText: string) {
    setBusy(true);
    setMsg(null);
    try {
      await fn();
      setMsg({ kind: 'ok', text: okText });
      onChanged();
    } catch (e) {
      setMsg({ kind: 'err', text: e instanceof Error ? e.message : String(e) });
    } finally {
      setBusy(false);
    }
  }

  function doRegister() {
    const id = newAgent.trim();
    if (!id) return;
    run(async () => {
      await api.registerAgent(id);
      if (!agents.includes(id)) onAgentsChange([...agents, id]);
      setNewAgent('');
    }, `身份「${id}」已创建`);
  }

  function doContribute() {
    run(
      () => api.contribute(cAgent, cSchema || '我的数据', cContent),
      `已提供「${cSchema || '我的数据'}」`,
    );
  }

  function doSubscribe() {
    const parsed = sources.map((s) => ({
      parent: s.id,
      price: Number(s.price) || 0,
      alpha: Number(s.alpha) || 0,
    }));
    const sum = parsed.reduce((a, s) => a + s.alpha, 0) || 1;
    const norm = parsed.map((s) => ({ ...s, alpha: s.alpha / sum }));
    run(
      () => api.subscribe(sAgent, sSchema || '我的成品', sContent, norm, Number(sRevenue) || 0),
      `已发布「${sSchema || '我的成品'}」`,
    );
  }

  function setSource(i: number, patch: Partial<SourceRow>) {
    setSources((prev) => prev.map((s, idx) => (idx === i ? { ...s, ...patch } : s)));
  }

  return (
    <div className="panel">
      <div className="panel-title">参与网络</div>

      <div className="tabs">
        {(
          [
            ['register', '创建身份'],
            ['contribute', '提供数据'],
            ['subscribe', '加工数据'],
          ] as [Tab, string][]
        ).map(([key, label]) => (
          <button key={key} className={`tab${tab === key ? ' active' : ''}`} onClick={() => setTab(key)}>
            {label}
          </button>
        ))}
      </div>

      {tab === 'register' && (
        <div className="form">
          <label className="field">
            <span className="field-name">
              给自己起个名字 <small>其他参与者将用这个名字认识你</small>
            </span>
            <input value={newAgent} onChange={(e) => setNewAgent(e.target.value)} placeholder="例如：alice" />
          </label>
          <button className="btn primary" disabled={busy} onClick={doRegister}>
            {busy ? '处理中…' : '创建身份'}
          </button>
        </div>
      )}

      {tab === 'contribute' && (
        <div className="form">
          <label className="field">
            <span className="field-name">以谁的名义提供</span>
            <select value={cAgent} onChange={(e) => setCAgent(e.target.value)}>
              <option value="">选择身份…</option>
              {agents.map((a) => (
                <option key={a} value={a}>
                  {a}
                </option>
              ))}
            </select>
          </label>
          <label className="field">
            <span className="field-name">
              给这份数据起个名字 <small>描述它的内容/用途，例如「设备温度」</small>
            </span>
            <input value={cSchema} onChange={(e) => setCSchema(e.target.value)} placeholder="例如：设备温度" />
          </label>
          <label className="field">
            <span className="field-name">数据内容</span>
            <textarea value={cContent} onChange={(e) => setCContent(e.target.value)} placeholder="粘贴你的数据内容" rows={2} />
          </label>
          <button className="btn primary" disabled={busy || !cAgent} onClick={doContribute}>
            {busy ? '处理中…' : '提供数据'}
          </button>
        </div>
      )}

      {tab === 'subscribe' && (
        <div className="form">
          <label className="field">
            <span className="field-name">以谁的名义加工</span>
            <select value={sAgent} onChange={(e) => setSAgent(e.target.value)}>
              <option value="">选择身份…</option>
              {agents.map((a) => (
                <option key={a} value={a}>
                  {a}
                </option>
              ))}
            </select>
          </label>
          <label className="field">
            <span className="field-name">
              新数据集的名字 <small>你加工后产出的成果，例如「清洗后的设备温度」</small>
            </span>
            <input value={sSchema} onChange={(e) => setSSchema(e.target.value)} placeholder="例如：清洗后的设备温度" />
          </label>
          <label className="field">
            <span className="field-name">新数据集内容</span>
            <textarea value={sContent} onChange={(e) => setSContent(e.target.value)} rows={2} placeholder="加工结果的内容" />
          </label>
          <label className="field">
            <span className="field-name">
              下游真实收入 <small>终端用户为基于此数据的产品付的钱；不知道就填 0</small>
            </span>
            <input value={sRevenue} onChange={(e) => setSRevenue(e.target.value)} type="number" placeholder="0" />
          </label>

          <div className="sources-label">
            我基于哪些数据集加工
            <small className="sources-tip">（每项填：用哪个数据集、订阅价、贡献权重）</small>
            {datasets.length === 0 && <em> —— 请先在上方提供数据</em>}
          </div>
          {sources.map((s, i) => (
            <div key={i} className="source-row">
              <select value={s.id} onChange={(e) => setSource(i, { id: e.target.value })} title="选择来源数据集">
                <option value="">选数据集…</option>
                {datasets.map((d) => (
                  <option key={d.id} value={d.id}>
                    {d.schema} · {d.owner} ({shorten(d.id, 5)})
                  </option>
                ))}
              </select>
              <input
                value={s.price}
                onChange={(e) => setSource(i, { price: e.target.value })}
                type="number"
                placeholder="订阅价"
                title="订阅价：为使用这份数据付的钱"
              />
              <input
                value={s.alpha}
                onChange={(e) => setSource(i, { alpha: e.target.value })}
                type="number"
                step="0.1"
                placeholder="权重"
                title="贡献权重：这份来源占多大贡献（自动归一化）"
              />
              <button className="btn small" onClick={() => setSources((p) => p.filter((_, idx) => idx !== i))}>
                ✕
              </button>
            </div>
          ))}
          <button className="btn" onClick={() => setSources((p) => [...p, { id: '', price: '100', alpha: '1' }])}>
            + 再加一个来源
          </button>

          <button className="btn primary" disabled={busy || !sAgent} onClick={doSubscribe}>
            {busy ? '处理中…' : '加工并发布'}
          </button>
        </div>
      )}

      {msg && <div className={`msg ${msg.kind}`}>{msg.text}</div>}
      {agents.length === 0 && (
        <div className="hint">还没有身份 —— 先在「创建身份」里创建一个，才能提供或加工数据。</div>
      )}
    </div>
  );
}
