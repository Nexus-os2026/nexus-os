export class NexusApiError extends Error {
  public readonly statusCode: number;
  public readonly endpoint: string;

  constructor(statusCode: number, endpoint: string, message: string) {
    super(message);
    this.name = "NexusApiError";
    this.statusCode = statusCode;
    this.endpoint = endpoint;
  }
}

export class NexusAuthError extends NexusApiError {
  constructor(endpoint: string, message: string) {
    super(401, endpoint, message);
    this.name = "NexusAuthError";
  }
}

export class NexusNotFoundError extends NexusApiError {
  constructor(endpoint: string, message: string) {
    super(404, endpoint, message);
    this.name = "NexusNotFoundError";
  }
}

export class NexusRateLimitError extends NexusApiError {
  constructor(endpoint: string, message: string) {
    super(429, endpoint, message);
    this.name = "NexusRateLimitError";
  }
}
