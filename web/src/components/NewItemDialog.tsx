import { useEffect, useRef, useState, type FormEvent } from "react";
import { Clock, FileText, FileUp, Link2, Loader2, Trash2 } from "lucide-react";
import { api } from "../api";
import type { DropPayload, Item, Kind } from "../types";
import { cn, formatBytes } from "../lib/utils";
import { Button, Input, Modal, ModalHeader, Textarea } from "./ui";

const tabs: { kind: Kind; label: string; icon: typeof FileText }[] = [
  { kind: "text", label: "Text", icon: FileText },
  { kind: "link", label: "Link", icon: Link2 },
  { kind: "file", label: "File", icon: FileUp },
];

const ttlOptions = [
  { label: "Never", seconds: 0 },
  { label: "1 hour", seconds: 3600 },
  { label: "6 hours", seconds: 21600 },
  { label: "1 day", seconds: 86400 },
  { label: "7 days", seconds: 604800 },
  { label: "30 days", seconds: 2592000 },
];

function hostOf(url: string): string {
  try {
    return new URL(url).hostname.replace(/^www\./, "");
  } catch {
    return url;
  }
}

function firstLine(text: string): string {
  const line = text.split(/\r?\n/).find((l) => l.trim()) ?? "";
  return line.trim().slice(0, 80);
}

function titleForFiles(files: File[]): string {
  if (files.length === 1) return files[0].name;
  return `${files.length} files`;
}

export function NewItemDialog({
  open,
  initial,
  onClose,
  onCreated,
}: {
  open: boolean;
  initial?: DropPayload | null;
  onClose: () => void;
  onCreated: (item: Item) => void;
}) {
  const [kind, setKind] = useState<Kind>("text");
  const [title, setTitle] = useState("");
  const [description, setDescription] = useState("");
  const [body, setBody] = useState("");
  const [url, setUrl] = useState("");
  const [files, setFiles] = useState<File[]>([]);
  const [ttl, setTtl] = useState(0);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const fileInput = useRef<HTMLInputElement>(null);

  // Prefill from a dropped file, URL or text each time the dialog opens.
  useEffect(() => {
    if (!open) return;
    setError(null);
    setTtl(0);
    if (initial?.files && initial.files.length > 0) {
      setKind("file");
      setFiles(initial.files);
      setTitle(titleForFiles(initial.files));
      setDescription("");
      setBody("");
      setUrl("");
    } else if (initial?.url) {
      setKind("link");
      setUrl(initial.url);
      setTitle(hostOf(initial.url));
      setFiles([]);
      setDescription("");
      setBody("");
    } else if (initial?.text) {
      setKind("text");
      setBody(initial.text);
      setTitle(firstLine(initial.text));
      setFiles([]);
      setDescription("");
      setUrl("");
    }
  }, [open, initial]);

  function reset() {
    setKind("text");
    setTitle("");
    setDescription("");
    setBody("");
    setUrl("");
    setFiles([]);
    setTtl(0);
    setError(null);
  }

  function close() {
    if (busy) return;
    reset();
    onClose();
  }

  function addFiles(list: FileList | null) {
    if (!list || list.length === 0) return;
    const next = [...files, ...Array.from(list)];
    setFiles(next);
    if (!title.trim() || title === titleForFiles(files)) {
      setTitle(titleForFiles(next));
    }
  }

  function removeFile(index: number) {
    setFiles((current) => current.filter((_, i) => i !== index));
  }

  async function submit(event: FormEvent) {
    event.preventDefault();
    setError(null);
    if (!title.trim()) {
      setError("A title is required.");
      return;
    }
    if (kind === "link" && !url.trim()) {
      setError("A URL is required.");
      return;
    }
    if (kind === "file" && files.length === 0) {
      setError("Choose at least one file.");
      return;
    }

    const form = new FormData();
    form.append("kind", kind);
    form.append("title", title.trim());
    form.append("description", description.trim());
    form.append("ttl_seconds", String(ttl));
    if (kind === "text") form.append("body", body);
    if (kind === "link") form.append("url", url.trim());
    if (kind === "file") for (const file of files) form.append("file", file);

    setBusy(true);
    try {
      const item = await api.create(form);
      onCreated(item);
      reset();
      onClose();
    } catch (e) {
      setError(e instanceof Error ? e.message : "Upload failed.");
    } finally {
      setBusy(false);
    }
  }

  return (
    <Modal open={open} onClose={close} className="max-w-xl">
      <ModalHeader title="Add to the board" onClose={close} />
      <form onSubmit={submit} className="flex flex-col overflow-y-auto">
        <div className="space-y-4 px-5 py-5">
          <div className="grid grid-cols-3 gap-2 rounded-xl bg-zinc-100 p-1 dark:bg-zinc-800">
            {tabs.map((tab) => {
              const Icon = tab.icon;
              const active = kind === tab.kind;
              return (
                <button
                  key={tab.kind}
                  type="button"
                  onClick={() => setKind(tab.kind)}
                  className={cn(
                    "flex items-center justify-center gap-2 rounded-lg py-2 text-sm font-medium transition-colors",
                    active
                      ? "bg-white text-zinc-900 shadow-sm dark:bg-zinc-900 dark:text-zinc-100"
                      : "text-zinc-500 hover:text-zinc-800 dark:text-zinc-400 dark:hover:text-zinc-200",
                  )}
                >
                  <Icon className="h-4 w-4" />
                  {tab.label}
                </button>
              );
            })}
          </div>

          <div>
            <label className="mb-1.5 block text-sm font-medium text-zinc-700 dark:text-zinc-300">
              Title
            </label>
            <Input
              value={title}
              onChange={(e) => setTitle(e.target.value)}
              placeholder="A name for this item"
              autoFocus
            />
          </div>

          <div className="grid gap-4 sm:grid-cols-2">
            <div>
              <label className="mb-1.5 block text-sm font-medium text-zinc-700 dark:text-zinc-300">
                Description <span className="text-zinc-400">(optional)</span>
              </label>
              <Input
                value={description}
                onChange={(e) => setDescription(e.target.value)}
                placeholder="A short note"
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
                className="h-10 w-full rounded-lg border-0 bg-white px-3 text-sm text-zinc-900 ring-1 ring-inset ring-zinc-300 focus:ring-2 focus:ring-inset focus:ring-indigo-500 dark:bg-zinc-800 dark:text-zinc-100 dark:ring-zinc-700"
              >
                {ttlOptions.map((option) => (
                  <option key={option.seconds} value={option.seconds}>
                    {option.label}
                  </option>
                ))}
              </select>
            </div>
          </div>

          {kind === "text" && (
            <div>
              <label className="mb-1.5 block text-sm font-medium text-zinc-700 dark:text-zinc-300">
                Content
              </label>
              <Textarea
                value={body}
                onChange={(e) => setBody(e.target.value)}
                rows={7}
                placeholder="Paste or type the text here"
              />
            </div>
          )}

          {kind === "link" && (
            <div>
              <label className="mb-1.5 block text-sm font-medium text-zinc-700 dark:text-zinc-300">
                URL
              </label>
              <Input
                value={url}
                onChange={(e) => setUrl(e.target.value)}
                placeholder="https://example.com"
                inputMode="url"
              />
            </div>
          )}

          {kind === "file" && (
            <div>
              <label className="mb-1.5 block text-sm font-medium text-zinc-700 dark:text-zinc-300">
                Files
              </label>
              {files.length > 0 && (
                <ul className="mb-2 space-y-1.5">
                  {files.map((file, index) => (
                    <li
                      key={`${file.name}-${index}`}
                      className="flex items-center gap-2 rounded-lg bg-zinc-50 px-3 py-2 text-sm ring-1 ring-zinc-200 dark:bg-zinc-800/60 dark:ring-zinc-700"
                    >
                      <FileUp className="h-4 w-4 shrink-0 text-zinc-400" />
                      <span className="min-w-0 flex-1 truncate text-zinc-700 dark:text-zinc-200">
                        {file.name}
                      </span>
                      <span className="shrink-0 text-xs text-zinc-400">
                        {formatBytes(file.size)}
                      </span>
                      <Button
                        type="button"
                        variant="ghost"
                        size="icon"
                        onClick={() => removeFile(index)}
                        aria-label="Remove file"
                      >
                        <Trash2 className="h-3.5 w-3.5" />
                      </Button>
                    </li>
                  ))}
                </ul>
              )}
              <button
                type="button"
                onClick={() => fileInput.current?.click()}
                className="flex w-full flex-col items-center justify-center gap-2 rounded-xl border-2 border-dashed border-zinc-300 px-4 py-6 text-center transition-colors hover:border-indigo-400 hover:bg-indigo-50/40 dark:border-zinc-700 dark:hover:border-indigo-500 dark:hover:bg-indigo-500/5"
              >
                <FileUp className="h-6 w-6 text-zinc-400" />
                <span className="text-sm text-zinc-500 dark:text-zinc-400">
                  {files.length > 0
                    ? "Add more files"
                    : "Click to choose files (you can pick several)"}
                </span>
              </button>
              <input
                ref={fileInput}
                type="file"
                multiple
                className="hidden"
                onChange={(e) => {
                  addFiles(e.target.files);
                  e.target.value = "";
                }}
              />
            </div>
          )}

          {error && (
            <p className="rounded-lg bg-red-50 px-3 py-2 text-sm text-red-700 dark:bg-red-500/10 dark:text-red-400">
              {error}
            </p>
          )}
        </div>

        <div className="flex items-center justify-end gap-2 border-t border-zinc-100 bg-zinc-50 px-5 py-4 dark:border-zinc-800 dark:bg-zinc-900/50">
          <Button type="button" variant="secondary" onClick={close} disabled={busy}>
            Cancel
          </Button>
          <Button type="submit" disabled={busy}>
            {busy && <Loader2 className="h-4 w-4 animate-spin" />}
            {kind === "file"
              ? `Upload${files.length > 1 ? ` ${files.length} files` : ""}`
              : "Add"}
          </Button>
        </div>
      </form>
    </Modal>
  );
}
