import { useState } from "react";
import {
  Check,
  Clock,
  Copy,
  Download,
  ExternalLink,
  Loader2,
  Pencil,
  Trash2,
} from "lucide-react";
import { api } from "../api";
import type { Item } from "../types";
import { formatBytes, formatDate, formatExpiry } from "../lib/utils";
import { Button, Input, Modal, ModalHeader, Textarea } from "./ui";

export function ItemView({
  item,
  canModify,
  onClose,
  onUpdate,
  onDelete,
}: {
  item: Item;
  canModify: boolean;
  onClose: () => void;
  onUpdate: (item: Item) => void;
  onDelete: (item: Item) => void;
}) {
  const [editing, setEditing] = useState(false);
  const [title, setTitle] = useState(item.title);
  const [description, setDescription] = useState(item.description ?? "");
  const [body, setBody] = useState(item.body ?? "");
  const [url, setUrl] = useState(item.url ?? "");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);

  async function save() {
    setBusy(true);
    setError(null);
    try {
      const updated = await api.update(item.id, {
        title: title.trim(),
        description: description.trim(),
        ...(item.kind === "text" ? { body } : {}),
        ...(item.kind === "link" ? { url: url.trim() } : {}),
      });
      onUpdate(updated);
      setEditing(false);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Could not save.");
    } finally {
      setBusy(false);
    }
  }

  async function copy() {
    if (!item.url) return;
    await navigator.clipboard.writeText(item.url);
    setCopied(true);
    setTimeout(() => setCopied(false), 1500);
  }

  return (
    <Modal open onClose={onClose} className="max-w-2xl">
      <ModalHeader title={editing ? "Edit item" : item.title} onClose={onClose} />

      <div className="overflow-y-auto px-5 py-5">
        {editing ? (
          <div className="space-y-4">
            <div>
              <label className="mb-1.5 block text-sm font-medium text-zinc-700 dark:text-zinc-300">
                Title
              </label>
              <Input value={title} onChange={(e) => setTitle(e.target.value)} />
            </div>
            <div>
              <label className="mb-1.5 block text-sm font-medium text-zinc-700 dark:text-zinc-300">
                Description
              </label>
              <Input
                value={description}
                onChange={(e) => setDescription(e.target.value)}
              />
            </div>
            {item.kind === "text" ? (
              <div>
                <label className="mb-1.5 block text-sm font-medium text-zinc-700 dark:text-zinc-300">
                  Content
                </label>
                <Textarea
                  value={body}
                  onChange={(e) => setBody(e.target.value)}
                  rows={10}
                />
              </div>
            ) : (
              <div>
                <label className="mb-1.5 block text-sm font-medium text-zinc-700 dark:text-zinc-300">
                  URL
                </label>
                <Input value={url} onChange={(e) => setUrl(e.target.value)} />
              </div>
            )}
            {error && (
              <p className="rounded-lg bg-red-50 px-3 py-2 text-sm text-red-700 dark:bg-red-500/10 dark:text-red-400">
                {error}
              </p>
            )}
          </div>
        ) : (
          <div className="space-y-5">
            {item.description && (
              <p className="text-sm text-zinc-600 dark:text-zinc-400">
                {item.description}
              </p>
            )}

            {item.kind === "text" && (
              <pre className="max-h-[50vh] overflow-auto whitespace-pre-wrap break-words rounded-xl bg-zinc-50 p-4 font-mono text-sm text-zinc-800 ring-1 ring-zinc-200 dark:bg-zinc-950 dark:text-zinc-200 dark:ring-zinc-800">
                {item.body}
              </pre>
            )}

            {item.kind === "link" && item.url && (
              <div className="flex items-center gap-2 rounded-xl bg-zinc-50 p-3 ring-1 ring-zinc-200 dark:bg-zinc-950 dark:ring-zinc-800">
                <span className="min-w-0 flex-1 truncate text-sm text-zinc-600 dark:text-zinc-300">
                  {item.url}
                </span>
                <Button variant="ghost" size="icon" onClick={() => void copy()} title="Copy link">
                  {copied ? (
                    <Check className="h-4 w-4 text-emerald-500" />
                  ) : (
                    <Copy className="h-4 w-4" />
                  )}
                </Button>
              </div>
            )}

            {item.kind === "file" && (
              <div className="flex items-center gap-3 rounded-xl bg-zinc-50 p-4 ring-1 ring-zinc-200 dark:bg-zinc-950 dark:ring-zinc-800">
                <div className="min-w-0 flex-1">
                  <p className="truncate text-sm font-medium text-zinc-800 dark:text-zinc-200">
                    {item.file_name}
                  </p>
                  {item.size != null && (
                    <p className="text-xs text-zinc-400">{formatBytes(item.size)}</p>
                  )}
                </div>
                <Button
                  size="sm"
                  onClick={() => {
                    window.location.href = api.fileUrl(item.id);
                  }}
                >
                  <Download className="h-4 w-4" />
                  Download
                </Button>
              </div>
            )}

            <div className="flex flex-wrap items-center gap-x-3 gap-y-1 text-xs text-zinc-400">
              <span>{item.author ? `by ${item.author}` : ""}</span>
              <span>·</span>
              <span>{formatDate(item.created_at)}</span>
              {formatExpiry(item.expires_at) && (
                <>
                  <span>·</span>
                  <span className="inline-flex items-center gap-1">
                    <Clock className="h-3 w-3" />
                    {formatExpiry(item.expires_at)}
                  </span>
                </>
              )}
            </div>
          </div>
        )}
      </div>

      <div className="flex items-center gap-2 border-t border-zinc-100 bg-zinc-50 px-5 py-4 dark:border-zinc-800 dark:bg-zinc-900/50">
        {!editing && item.kind === "link" && item.url && (
          <Button
            variant="secondary"
            size="sm"
            onClick={() => window.open(item.url!, "_blank", "noopener")}
          >
            <ExternalLink className="h-4 w-4" />
            Open link
          </Button>
        )}
        <div className="ml-auto flex items-center gap-2">
          {canModify &&
            (editing ? (
              <>
                <Button
                  variant="secondary"
                  size="sm"
                  onClick={() => setEditing(false)}
                  disabled={busy}
                >
                  Cancel
                </Button>
                <Button size="sm" onClick={() => void save()} disabled={busy}>
                  {busy && <Loader2 className="h-4 w-4 animate-spin" />}
                  Save
                </Button>
              </>
            ) : (
              <>
                {(item.kind === "text" || item.kind === "link") && (
                  <Button
                    variant="secondary"
                    size="sm"
                    onClick={() => setEditing(true)}
                  >
                    <Pencil className="h-4 w-4" />
                    Edit
                  </Button>
                )}
                <Button
                  variant="ghost"
                  size="sm"
                  className="text-zinc-400 hover:bg-red-50 hover:text-red-600 dark:hover:bg-red-500/10"
                  onClick={() => onDelete(item)}
                >
                  <Trash2 className="h-4 w-4" />
                  Delete
                </Button>
              </>
            ))}
        </div>
      </div>
    </Modal>
  );
}
