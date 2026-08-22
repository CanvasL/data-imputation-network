import type { AgentResp, DatasetInfo, GraphData, SubmitResult } from './types';

export class ApiError extends Error {
  status: number;
  constructor(status: number, message: string) {
    super(message);
    this.status = status;
  }
}

/** Thin client for the din-node HTTP API. */
export class DinApi {
  constructor(public base: string) {}

  private async req<T>(path: string, init?: RequestInit): Promise<T> {
    let res: Response;
    try {
      res = await fetch(`${this.base}${path}`, init);
    } catch (e) {
      throw new ApiError(0, `无法连接节点: ${String(e)}`);
    }
    if (!res.ok) {
      let msg = res.statusText;
      try {
        const j = (await res.json()) as { error?: string };
        msg = j?.error ?? msg;
      } catch {
        /* keep statusText */
      }
      throw new ApiError(res.status, msg);
    }
    return res.json() as Promise<T>;
  }

  private json(method: string, body: unknown): RequestInit {
    return {
      method,
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify(body),
    };
  }

  health() {
    return this.req<{ status: string }>('/health');
  }

  registerAgent(agentId: string) {
    return this.req<AgentResp>('/agents', this.json('POST', { agent_id: agentId }));
  }

  contribute(agentId: string, schema: string, content: string) {
    return this.req<SubmitResult>(
      '/events',
      this.json('POST', {
        agent_id: agentId,
        event_type: 'contribute',
        payload: { owner: agentId, schema, version: 1, content },
      }),
    );
  }

  subscribe(
    agentId: string,
    childSchema: string,
    content: string,
    sources: { parent: string; price: number; alpha: number }[],
    externalRevenue: number,
  ) {
    return this.req<SubmitResult>(
      '/events',
      this.json('POST', {
        agent_id: agentId,
        event_type: 'subscribe',
        payload: {
          child: { owner: agentId, schema: childSchema, version: 1, content },
          sources,
          external_revenue: externalRevenue,
        },
      }),
    );
  }

  datasets() {
    return this.req<{ datasets: DatasetInfo[] }>('/datasets');
  }

  graph() {
    return this.req<GraphData>('/graph');
  }
}
