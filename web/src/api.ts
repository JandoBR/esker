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

  addFiles: (id: string, files: File[]) => {
    const form = new FormData();
    for (const file of files) form.append("file", file);
    return request<Item>(`/api/items/${id}/files`, {
      method: "POST",
      body: form,
    });
  },

  removeFile: (id: string, fileId: string) =>
    request<Item>(`/api/items/${id}/files/${fileId}`, { method: "DELETE" }),

  fileUrl: (id: string, fileId: string, inline = false) =>
    `/api/items/${id}/files/${fileId}${inline ? "?inline=true" : ""}`,

  /** Every file of an item, zipped. */
  archiveUrl: (id: string) => `/api/items/${id}/archive`,

  /** What one download action should fetch: the file itself, or all of them. */
  downloadUrl: (item: Item) => {
    if (item.files.length === 0) return null;
    return item.files.length === 1
      ? `/api/items/${item.id}/files/${item.files[0].id}`
      : `/api/items/${item.id}/archive`;
  },

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
