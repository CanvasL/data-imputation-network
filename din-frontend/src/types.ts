// Shared types for the DIN explorer.

export interface AgentResp {
  agent_id: string;
  public_key: string;
  secret_key: string;
}

export interface DatasetInfo {
  id: string;
  owner: string;
  schema: string;
  version: number;
  value: number;
}

export interface GraphData {
  nodes: { id: string }[];
  edges: { parent: string; child: string; price: number; alpha: number }[];
}

export interface SubmitResult {
  event_id: number;
  accepted: boolean;
}
