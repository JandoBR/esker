import type { Item, ItemPatch, User } from "./types";

export class ApiError extends Error {
  constructor(
    public status: number,
    message: string,
  ) {
    super(message);
    this.name = "ApiError";
  }
}

async function request<T>(path: string, init?: RequestInit): Promise<T> {
  const res = await fetch(path, { credentials: "include", ...init });
  if (!res.ok) {
    let message = res.statusText || `request failed (${res.status})`;
    try {
      const data = (await res.json()) as { error?: string };
      if (data.error) message = data.error;
    } catch {
      // not JSON, keep the status text
    }
    throw new ApiError(res.status, message);
  }
  if (res.status === 204) return undefined as T;
  return (await res.json()) as T;
}

export const api = {
  me: () => request<User>("/api/me"),

  list: (query?: string) =>
    request<Item[]>(
      `/api/items${query ? `?q=${encodeURIComponent(query)}` : ""}`,
    ),

  create: (form: FormData) =>
    request<Item>("/api/items", { method: "POST", body: form }),

  update: (id: string, patch: ItemPatch) =>
    request<Item>(`/api/items/${id}`, {
      method: "PATCH",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(patch),
    }),

  remove: (id: string) =>
    request<void>(`/api/items/${id}`, { method: "DELETE" }),

  fileUrl: (id: string, inline = false) =>
    `/api/items/${id}/file${inline ? "?inline=true" : ""}`,

  login: () => {
    window.location.href = "/api/auth/login";
  },

  logout: async () => {
    await fetch("/api/auth/logout", {
      method: "POST",
      credentials: "include",
    });
    window.location.href = "/";
  },
};
