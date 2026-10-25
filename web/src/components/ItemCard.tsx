import {
  Clock,
  Download,
  ExternalLink,
  FileArchive,
  FileText,
  Link2,
  Trash2,
} from "lucide-react";
import type { Item } from "../types";
import { download, formatBytes, formatDate, formatExpiry } from "../lib/utils";
import { api } from "../api";
import { Badge, Button } from "./ui";

const kindStyles: Record<
  Item["kind"],
  { icon: typeof FileText; className: string; label: string }
> = {
  text: {
    icon: FileText,
    className: "bg-sky-50 text-sky-600 dark:bg-sky-500/10 dark:text-sky-400",
    label: "Text",
  },
  link: {
    icon: Link2,
    className:
      "bg-violet-50 text-violet-600 dark:bg-violet-500/10 dark:text-violet-400",
    label: "Link",
  },
  file: {
    icon: FileArchive,
    className:
      "bg-amber-50 text-amber-600 dark:bg-amber-500/10 dark:text-amber-400",
    label: "File",
  },
};

export function ItemCard({
  item,
  canDelete,
  onOpen,
  onDelete,
}: {
  item: Item;
  canDelete: boolean;
  onOpen: () => void;
  onDelete: () => void;
}) {
  const style = kindStyles[item.kind];
  const Icon = style.icon;
  const expiry = formatExpiry(item.expires_at);
  const totalSize = item.files.reduce((sum, file) => sum + (file.size ?? 0), 0);
  const filesLabel =
    item.files.length === 1 ? item.files[0].file_name : `${item.files.length} files`;

  return (
    <div
      onClick={onOpen}
      className="group flex cursor-pointer flex-col rounded-xl border border-zinc-200 bg-white p-4 shadow-sm transition-all hover:-translate-y-0.5 hover:shadow-md dark:border-zinc-800 dark:bg-zinc-900"
    >
      <div className="flex items-start gap-3">
        <div
          className={`flex h-10 w-10 shrink-0 items-center justify-center rounded-lg ${style.className}`}
        >
          <Icon className="h-5 w-5" />
        </div>
        <div className="min-w-0 flex-1">
          <button
            onClick={(event) => {
              event.stopPropagation();
              onOpen();
            }}
            className="block w-full truncate text-left text-sm font-semibold text-zinc-900 hover:text-indigo-600 dark:text-zinc-100 dark:hover:text-indigo-400"
            title={item.title}
          >
            {item.title}
          </button>
          <p className="mt-0.5 line-clamp-2 min-h-[2.5rem] text-sm text-zinc-500 dark:text-zinc-400">
            {item.description ||
              (item.kind === "link"
                ? item.url
                : item.kind === "file"
                  ? filesLabel
                  : (item.body ?? "").slice(0, 160))}
          </p>
        </div>
      </div>

      <div className="mt-3 flex items-center gap-2 text-xs text-zinc-400 dark:text-zinc-500">
        <Badge className={style.className}>{style.label}</Badge>
        {item.kind === "file" && item.files.length > 0 && (
          <span>{formatBytes(totalSize)}</span>
        )}
        {expiry && (
          <span className="inline-flex items-center gap-1 whitespace-nowrap">
            <Clock className="h-3 w-3" />
            {expiry}
          </span>
        )}
        <span className="truncate">
          {item.author ? `by ${item.author}` : ""} · {formatDate(item.created_at)}
        </span>
      </div>

      <div className="mt-3 flex items-center gap-1 border-t border-zinc-100 pt-3 dark:border-zinc-800">
        {item.kind === "file" ? (
          <Button
            variant="ghost"
            size="sm"
            disabled={item.files.length === 0}
            onClick={(event) => {
              event.stopPropagation();
              const url = api.downloadUrl(item);
              if (url) download(url);
            }}
          >
            <Download className="h-3.5 w-3.5" /> Download
          </Button>
        ) : (
          <Button
            variant="ghost"
            size="sm"
            onClick={(event) => {
              event.stopPropagation();
              onOpen();
            }}
          >
            {item.kind === "link" ? (
              <>
                <ExternalLink className="h-3.5 w-3.5" /> Open
              </>
            ) : (
              <>
                <FileText className="h-3.5 w-3.5" /> View
              </>
            )}
          </Button>
        )}
        {canDelete && (
          <Button
            variant="ghost"
            size="sm"
            className="ml-auto text-zinc-400 hover:bg-red-50 hover:text-red-600 dark:hover:bg-red-500/10"
            onClick={(event) => {
              event.stopPropagation();
              onDelete();
            }}
            aria-label="Delete"
            title="Delete"
          >
            <Trash2 className="h-3.5 w-3.5" />
          </Button>
        )}
      </div>
    </div>
  );
}
