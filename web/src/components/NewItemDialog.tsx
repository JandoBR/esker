import { useRef, useState, type FormEvent } from "react";
import { FileText, FileUp, Link2, Loader2 } from "lucide-react";
import { api } from "../api";
import type { Item, Kind } from "../types";
import { cn, formatBytes } from "../lib/utils";
import { Button, Input, Modal, ModalHeader, Textarea } from "./ui";

const tabs: { kind: Kind; label: string; icon: typeof FileText }[] = [
  { kind: "text", label: "Text", icon: FileText },
  { kind: "link", label: "Link", icon: Link2 },
  { kind: "file", label: "File", icon: FileUp },
];

export function NewItemDialog({
  open,
  onClose,
  onCreated,
}: {
  open: boolean;
  onClose: () => void;
  onCreated: (item: Item) => void;
}) {
  const [kind, setKind] = useState<Kind>("text");
  const [title, setTitle] = useState("");
  const [description, setDescription] = useState("");
  const [body, setBody] = useState("");
  const [url, setUrl] = useState("");
  const [file, setFile] = useState<File | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const fileInput = useRef<HTMLInputElement>(null);

  function reset() {
    setKind("text");
    setTitle("");
    setDescription("");
    setBody("");
    setUrl("");
    setFile(null);
    setError(null);
  }

  function close() {
    if (busy) return;
    reset();
    onClose();
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
    if (kind === "file" && !file) {
      setError("Choose a file to upload.");
      return;
    }

    const form = new FormData();
    form.append("kind", kind);
    form.append("title", title.trim());
    form.append("description", description.trim());
    if (kind === "text") form.append("body", body);
    if (kind === "link") form.append("url", url.trim());
    if (kind === "file" && file) form.append("file", file);

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

          <div>
            <label className="mb-1.5 block text-sm font-medium text-zinc-700 dark:text-zinc-300">
              Description <span className="text-zinc-400">(optional)</span>
            </label>
            <Input
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              placeholder="A short note shown on the card"
            />
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
                File
              </label>
              <button
                type="button"
                onClick={() => fileInput.current?.click()}
                className="flex w-full flex-col items-center justify-center gap-2 rounded-xl border-2 border-dashed border-zinc-300 px-4 py-8 text-center transition-colors hover:border-indigo-400 hover:bg-indigo-50/40 dark:border-zinc-700 dark:hover:border-indigo-500 dark:hover:bg-indigo-500/5"
              >
                <FileUp className="h-6 w-6 text-zinc-400" />
                {file ? (
                  <span className="text-sm text-zinc-700 dark:text-zinc-300">
                    {file.name}{" "}
                    <span className="text-zinc-400">
                      ({formatBytes(file.size)})
                    </span>
                  </span>
                ) : (
                  <span className="text-sm text-zinc-500 dark:text-zinc-400">
                    Click to choose a file
                  </span>
                )}
              </button>
              <input
                ref={fileInput}
                type="file"
                className="hidden"
                onChange={(e) => setFile(e.target.files?.[0] ?? null)}
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
            {kind === "file" ? "Upload" : "Add"}
          </Button>
        </div>
      </form>
    </Modal>
  );
}
