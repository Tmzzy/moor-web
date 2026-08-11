// SPDX-License-Identifier: Apache-2.0
// Modified from the original Moor project for this Web/Docker distribution; see NOTICE.

export interface AuthSession {
  authenticated: boolean;
  username?: string;
}

export interface LoginInput {
  username: string;
  password: string;
}

export interface McpTokenResponse {
  token: string;
}
