import { useState } from "react";
import {
  Check,
  Clock,
  Copy,
  Download,
  ExternalLink,
  FileUp,
  Loader2,
  Pencil,
  Trash2,
} from "lucide-react";
import { api } from "../api";
import type { Item } from "../types";
import { download, formatBytes, formatDate, formatExpiry } from "../lib/utils";
import { Button, Input, Modal, ModalHeader, Textarea } from "./ui";

const ttlOptions = [
  { label: "Keep", value: 0 },
  { label: "Never", value: -1 },
  { label: "1 hour", value: 3600 },
  { label: "6 hours", value: 21600 },
  { label: "1 day", value: 86400 },
  { label: "7 days", value: 604800 },
  { label: "30 days", value: 2592000 },
];

const selectClass =
  "h-10 w-full rounded-lg border-0 bg-white px-3 text-sm text-zinc-900 ring-1 ring-inset ring-zinc-300 focus:ring-2 focus:ring-inset focus:ring-indigo-500 dark:bg-zinc-800 dark:text-zinc-100 dark:ring-zinc-700";

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
  const [ttl, setTtl] = useState(0);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);

  const expiry = formatExpiry(item.expires_at);

  function startEditing() {
    setTitle(item.title);
    setDescription(item.description ?? "");
    setBody(item.body ?? "");
    setUrl(item.url ?? "");
    setTtl(0);
    setError(null);
    setEditing(true);
  }

  async function save() {
    setBusy(true);
    setError(null);
    try {
      const updated = await api.update(item.id, {
        title: title.trim(),
        description: description.trim(),
        expires_in: ttl,
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

  async function addFiles(list: FileList | null) {
    if (!list || list.length === 0) return;
    setBusy(true);
    setError(null);
    try {
      onUpdate(await api.addFiles(item.id, Array.from(list)));
    } catch (e) {
      setError(e instanceof Error ? e.message : "Could not add files.");
    } finally {
      setBusy(false);
    }
  }

  async function removeFile(fileId: string) {
    setBusy(true);
    setError(null);
    try {
      onUpdate(await api.removeFile(item.id, fileId));
    } catch (e) {
      setError(e instanceof Error ? e.message : "Could not remove file.");
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
            <div className="grid gap-4 sm:grid-cols-2">
              <div>
                <label className="mb-1.5 block text-sm font-medium text-zinc-700 dark:text-zinc-300">
                  Description
                </label>
                <Input
                  value={description}
                  onChange={(e) => setDescription(e.target.value)}
                />
              </div>
              <div>
                <label className="mb-1.5 flex items-center gap-1.5 text-sm font-medium text-zinc-700 dark:text-zinc-300">
                  <Clock className="h-3.5 w-3.5 text-zinc-400" />
                  Expires
                </label>
                <select
                  value={ttl}
                  onChange={(e) => setTtl(Number(e.target.value))}
                  className={selectClass}
                >
                  {ttlOptions.map((option) => (
                    <option key={option.value} value={option.value}>
                      {option.label}
                    </option>
                  ))}
                </select>
              </div>
            </div>

            {item.kind === "text" && (
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
            )}

            {item.kind === "link" && (
              <div>
                <label className="mb-1.5 block text-sm font-medium text-zinc-700 dark:text-zinc-300">
                  URL
                </label>
                <Input value={url} onChange={(e) => setUrl(e.target.value)} />
              </div>
            )}

            {item.kind === "file" && (
              <div>
                <label className="mb-1.5 block text-sm font-medium text-zinc-700 dark:text-zinc-300">
                  Files
                </label>
                <ul className="space-y-1.5">
                  {item.files.map((file) => (
                    <li
                      key={file.id}
                      className="flex items-center gap-2 rounded-lg bg-zinc-50 px-3 py-2 text-sm ring-1 ring-zinc-200 dark:bg-zinc-800/60 dark:ring-zinc-700"
                    >
                      <FileUp className="h-4 w-4 shrink-0 text-zinc-400" />
                      <span className="min-w-0 flex-1 truncate text-zinc-700 dark:text-zinc-200">
                        {file.file_name}
                      </span>
                      {file.size != null && (
                        <span className="shrink-0 text-xs text-zinc-400">
                          {formatBytes(file.size)}
                        </span>
                      )}
                      <Button
                        type="button"
                        variant="ghost"
                        size="icon"
                        onClick={() => void removeFile(file.id)}
                        disabled={busy}
                        aria-label="Remove file"
                      >
                        <Trash2 className="h-3.5 w-3.5" />
                      </Button>
                    </li>
                  ))}
                </ul>
                <label className="mt-2 flex cursor-pointer items-center justify-center gap-2 rounded-xl border-2 border-dashed border-zinc-300 px-4 py-4 text-sm text-zinc-500 transition-colors hover:border-indigo-400 hover:bg-indigo-50/40 dark:border-zinc-700 dark:text-zinc-400 dark:hover:border-indigo-500 dark:hover:bg-indigo-500/5">
                  <FileUp className="h-4 w-4" />
                  Add files
                  <input
                    type="file"
                    multiple
                    className="hidden"
                    onChange={(e) => {
                      void addFiles(e.target.files);
                      e.target.value = "";
                    }}
                  />
                </label>
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
              <div className="space-y-2">
                {item.files.length === 0 && (
                  <p className="text-sm text-zinc-400">No files.</p>
                )}
                {item.files.length > 1 && (
                  <div className="flex items-center justify-between gap-2">
                    <p className="text-xs font-medium tracking-wide text-zinc-400 uppercase">
                      {item.files.length} files
                    </p>
                    <Button
                      variant="secondary"
                      size="sm"
                      onClick={() => download(api.archiveUrl(item.id))}
                    >
                      <Download className="h-4 w-4" />
                      Download all
                    </Button>
                  </div>
                )}
                {item.files.map((file) => (
                  <div
                    key={file.id}
                    className="flex items-center gap-3 rounded-xl bg-zinc-50 p-3 ring-1 ring-zinc-200 dark:bg-zinc-950 dark:ring-zinc-800"
                  >
                    <div className="min-w-0 flex-1">
                      <p className="truncate text-sm font-medium text-zinc-800 dark:text-zinc-200">
                        {file.file_name}
                      </p>
                      {file.size != null && (
                        <p className="text-xs text-zinc-400">
                          {formatBytes(file.size)}
                        </p>
                      )}
                    </div>
                    <Button
                      size="sm"
                      onClick={() => download(api.fileUrl(item.id, file.id))}
                    >
                      <Download className="h-4 w-4" />
                      Download
                    </Button>
                  </div>
                ))}
              </div>
            )}

            <div className="flex flex-wrap items-center gap-x-3 gap-y-1 text-xs text-zinc-400">
              <span>{item.author ? `by ${item.author}` : ""}</span>
              <span>·</span>
              <span>{formatDate(item.created_at)}</span>
              {expiry && (
                <>
                  <span>·</span>
                  <span className="inline-flex items-center gap-1">
                    <Clock className="h-3 w-3" />
                    {expiry}
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
                <Button variant="secondary" size="sm" onClick={startEditing}>
                  <Pencil className="h-4 w-4" />
                  Edit
                </Button>
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
