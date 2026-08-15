import type { ServerStatus } from "./server.js";
import type { Settings } from "./settings.js";

export interface ServerStatusEvent {
  type: "server:status";
  data: {
    serverId: string;
    status: ServerStatus;
    errorMessage?: string | null;
  };
}

export interface ServerToolsEvent {
  type: "server:tools";
  data: {
    serverId: string;
  };
}

export interface SettingsChangedEvent {
  type: "settings:changed";
  data: Settings;
}

export type MoorEvent =
  | ServerStatusEvent
  | ServerToolsEvent
  | SettingsChangedEvent;

export type MoorEventType = MoorEvent["type"];

export type MoorEventData<T extends MoorEventType> = Extract<MoorEvent, { type: T }>["data"];
