import type { ScannedServer } from "./scanned.js";

export interface ClientSnippet {
  client: string;
  description: string;
  snippet: string;
  cliCommand: string;
}

export interface ConvertResult {
  content: string;
  warnings: string[];
  targetPath: string;
  targetClient: string;
}

export interface ExecuteImportInput {
  servers: ScannedServer[];
  profileIds: string[];
}

export interface ExecuteImportResult {
  imported: string[];
  skipped: string[];
}
