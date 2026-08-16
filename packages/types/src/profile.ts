import type { Server } from "./server.js";

export interface Profile {
  id: string;
  name: string;
  serverCount?: number;
  createdAt: string;
  updatedAt: string;
}

export interface ProfileServerState {
  enabled: boolean;
  disabledTools: string[];
}

export interface ProfileDetail extends Profile {
  servers: Array<Server & { profileServer: ProfileServerState }>;
}
