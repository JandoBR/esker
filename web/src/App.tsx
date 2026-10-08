import { useEffect, useMemo, useState } from "react";
import {
  FileArchive,
  FileText,
  Inbox,
  Link2,
  Loader2,
  Plus,
  Search,
} from "lucide-react";
import { api, ApiError } from "./api";
import type { Item, Kind, User } from "./types";
import { cn } from "./lib/utils";
import { Header } from "./components/Header";
import { ItemCard } from "./components/ItemCard";
import { ItemView } from "./components/ItemView";
import { NewItemDialog } from "./components/NewItemDialog";
import { Button, Input } from "./components/ui";

type Filter = "all" | Kind;

const filters: { key: Filter; label: string; icon: typeof Inbox }[] = [
  { key: "all", label: "All", icon: Inbox },
  { key: "text", label: "Text", icon: FileText },
  { key: "link", label: "Links", icon: Link2 },
  { key: "file", label: "Files", icon: FileArchive },
];

export function App() {
  const [user, setUser] = useState<User | null | undefined>(undefined);
  const [items, setItems] = useState<Item[]>([]);
  const [loading, setLoading] = useState(true);
  const [query, setQuery] = useState("");
  const [filter, setFilter] = useState<Filter>("all");
  const [creating, setCreating] = useState(false);
  const [selected, setSelected] = useState<Item | null>(null);

  useEffect(() => {
    api
      .me()
      .then(setUser)
      .catch((e) => {
        if (e instanceof ApiError && e.status === 401) setUser(null);
        else setUser(null);
      });
  }, []);

  useEffect(() => {
    if (!user) return;
    const timer = setTimeout(() => {
      setLoading(true);
      api
        .list(query)
        .then(setItems)
        .catch(() => undefined)
        .finally(() => setLoading(false));
    }, 200);
    return () => clearTimeout(timer);
  }, [query, user]);

  const counts = useMemo(() => {
    const base: Record<Filter, number> = {
      all: items.length,
      text: 0,
      link: 0,
      file: 0,
    };
    for (const item of items) base[item.kind] += 1;
    return base;
  }, [items]);

  const visible = useMemo(
    () => items.filter((item) => filter === "all" || item.kind === filter),
    [items, filter],
  );

  function canModify(item: Item) {
    return Boolean(user && (user.is_admin || item.created_by === user.sub));
  }

  async function remove(item: Item) {
    if (!window.confirm(`Delete "${item.title}"?`)) return;
    await api.remove(item.id);
    setItems((current) => current.filter((i) => i.id !== item.id));
    setSelected((current) => (current?.id === item.id ? null : current));
  }

  if (user === undefined) {
    return (
      <div className="flex h-full items-center justify-center bg-zinc-50 dark:bg-zinc-950">
        <Loader2 className="h-6 w-6 animate-spin text-zinc-400" />
      </div>
    );
  }

  if (user === null) {
    return (
      <div className="flex h-full items-center justify-center bg-gradient-to-br from-zinc-50 to-zinc-100 px-4 dark:from-zinc-950 dark:to-zinc-900">
        <div className="w-full max-w-sm text-center">
          <div className="mx-auto mb-6 flex h-14 w-14 items-center justify-center rounded-2xl bg-gradient-to-br from-indigo-500 to-violet-600 text-xl font-bold text-white shadow-lg">
            B
          </div>
          <h1 className="text-2xl font-semibold tracking-tight text-zinc-900 dark:text-zinc-100">
            Board
          </h1>
          <p className="mt-2 text-sm text-zinc-500 dark:text-zinc-400">
            Text, links and files, in one place.
          </p>
          <Button className="mt-6 w-full" onClick={() => api.login()}>
            Sign in
          </Button>
        </div>
      </div>
    );
  }

  return (
    <div className="min-h-full bg-zinc-50 dark:bg-zinc-950">
      <Header user={user} onNew={() => setCreating(true)} />

      <main className="mx-auto max-w-5xl px-4 py-6">
        <div className="flex flex-col gap-3 sm:flex-row sm:items-center">
          <div className="relative flex-1">
            <Search className="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-zinc-400" />
            <Input
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder="Search the board"
              className="pl-9"
            />
          </div>
          <div className="flex gap-1 rounded-xl bg-zinc-100 p-1 dark:bg-zinc-800">
            {filters.map((entry) => {
              const Icon = entry.icon;
              return (
                <button
                  key={entry.key}
                  onClick={() => setFilter(entry.key)}
                  className={cn(
                    "flex items-center gap-1.5 rounded-lg px-3 py-1.5 text-sm font-medium transition-colors",
                    filter === entry.key
                      ? "bg-white text-zinc-900 shadow-sm dark:bg-zinc-900 dark:text-zinc-100"
                      : "text-zinc-500 hover:text-zinc-800 dark:text-zinc-400 dark:hover:text-zinc-200",
                  )}
                >
                  <Icon className="h-3.5 w-3.5" />
                  {entry.label}
                  <span className="text-xs text-zinc-400">{counts[entry.key]}</span>
                </button>
              );
            })}
          </div>
        </div>

        {loading ? (
          <div className="flex justify-center py-20">
            <Loader2 className="h-6 w-6 animate-spin text-zinc-400" />
          </div>
        ) : visible.length === 0 ? (
          <div className="mt-10 flex flex-col items-center justify-center rounded-2xl border border-dashed border-zinc-300 py-16 text-center dark:border-zinc-800">
            <Inbox className="h-8 w-8 text-zinc-300 dark:text-zinc-700" />
            <p className="mt-3 text-sm font-medium text-zinc-600 dark:text-zinc-300">
              {query ? "Nothing matches that search" : "The board is empty"}
            </p>
            <Button className="mt-4" size="sm" onClick={() => setCreating(true)}>
              <Plus className="h-4 w-4" />
              Add something
            </Button>
          </div>
        ) : (
          <div className="mt-5 grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3">
            {visible.map((item) => (
              <ItemCard
                key={item.id}
                item={item}
                canDelete={canModify(item)}
                onOpen={() => setSelected(item)}
                onDelete={() => void remove(item)}
              />
            ))}
          </div>
        )}
      </main>

      <NewItemDialog
        open={creating}
        onClose={() => setCreating(false)}
        onCreated={(item) => setItems((current) => [item, ...current])}
      />

      {selected && (
        <ItemView
          item={selected}
          canModify={canModify(selected)}
          onClose={() => setSelected(null)}
          onUpdate={(updated) => {
            setItems((current) =>
              current.map((i) => (i.id === updated.id ? updated : i)),
            );
            setSelected(updated);
          }}
          onDelete={(item) => void remove(item)}
        />
      )}
    </div>
  );
}
