import { LogOut, Plus, ShieldCheck } from "lucide-react";
import { api } from "../api";
import type { User } from "../types";
import { Badge, Button } from "./ui";

export function Header({ user, onNew }: { user: User; onNew: () => void }) {
  const name = user.display_name || user.username || "Signed in";
  return (
    <header className="sticky top-0 z-40 border-b border-zinc-200/70 bg-white/80 backdrop-blur-md dark:border-zinc-800/70 dark:bg-zinc-900/80">
      <div className="mx-auto flex h-16 max-w-5xl items-center gap-4 px-4">
        <div className="flex items-center gap-2.5">
          <div className="flex h-8 w-8 items-center justify-center rounded-lg bg-gradient-to-br from-indigo-500 to-violet-600 text-sm font-bold text-white">
            E
          </div>
          <span className="text-lg font-semibold tracking-tight text-zinc-900 dark:text-zinc-100">
            Esker
          </span>
        </div>

        <div className="ml-auto flex items-center gap-3">
          <div className="hidden items-center gap-2 sm:flex">
            <span className="text-sm text-zinc-600 dark:text-zinc-400">{name}</span>
            {user.is_admin && (
              <Badge className="bg-indigo-50 text-indigo-700 dark:bg-indigo-500/10 dark:text-indigo-300">
                <ShieldCheck className="h-3 w-3" />
                admin
              </Badge>
            )}
          </div>
          <Button size="sm" onClick={onNew}>
            <Plus className="h-4 w-4" />
            <span className="hidden sm:inline">New</span>
          </Button>
          <Button
            variant="ghost"
            size="icon"
            onClick={() => void api.logout()}
            aria-label="Sign out"
            title="Sign out"
          >
            <LogOut className="h-4 w-4" />
          </Button>
        </div>
      </div>
    </header>
  );
}
