// SPDX-License-Identifier: Apache-2.0
// Modified from the original Moor project for this Web/Docker distribution; see NOTICE.

export interface RuntimeInfo {
  port: number;
  baseUrl: string;
  mcpUrl?: string;
}
