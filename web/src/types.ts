export type Kind = "text" | "link" | "file";

export interface User {
  sub: string;
  username: string | null;
  display_name: string | null;
  email: string | null;
  is_admin: boolean;
}

export interface FileMeta {
  id: string;
  file_name: string;
  content_type: string | null;
  size: number | null;
}

export interface Item {
  id: string;
  kind: Kind;
  title: string;
  description: string | null;
  body: string | null;
  url: string | null;
  created_by: string | null;
  created_at: string;
  expires_at: string | null;
  author: string | null;
  files: FileMeta[];
}

/** What a drag-and-drop (or paste) handed us, before the dialog opens. */
export interface DropPayload {
  files?: File[];
  url?: string;
  text?: string;
}

export interface ItemPatch {
  title?: string;
  description?: string;
  body?: string;
  url?: string;
  expires_in?: number;
}
