export type ApiErrorCode =
  | "UNAUTHORIZED"
  | "FORBIDDEN"
  | "INVALID_CREDENTIALS"
  | "NOT_FOUND"
  | "VALIDATION_ERROR"
  | "ACTIVE_PROFILE"
  | "SERVER_NOT_RUNNING"
  | "ORDER_INVALID"
  | "PAYLOAD_TOO_LARGE"
  | "INTERNAL_ERROR";

export interface ApiError {
  code: ApiErrorCode;
  message: string;
}
