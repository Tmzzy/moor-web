import { useState } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Card, CardContent } from "@/components/ui/card";
import { PageHeader } from "@/components/shared/PageHeader";
import { useProfiles } from "@/hooks/useProfiles";
import { Plus, Trash2, Code, FlaskConical, User, Home } from "lucide-react";
import { useNavigate } from "react-router-dom";
import { cn } from "@/lib/utils";

const profileIcons = [Code, FlaskConical, User, Home, Code, FlaskConical, User, Home];
const profileAccents = [
  "bg-cursor-orange/10 text-cursor-orange border-cursor-orange/20",
  "bg-grep/15 text-grep border-grep/20",
  "bg-read/15 text-read border-read/20",
  "bg-edit/15 text-edit border-edit/20",
];

export function Profiles() {
  const { profiles, createProfile, deleteProfile } = useProfiles();
  const [newName, setNewName] = useState("");
  const [creating, setCreating] = useState(false);
  const navigate = useNavigate();

  const handleCreate = async () => {
    if (!newName.trim()) return;
    await createProfile(newName.trim());
    setNewName("");
    setCreating(false);
  };

  return (
    <div className="space-y-8 animate-fade-in-up">
      <PageHeader
        title="Profiles"
        subtitle="Manage server groupings and tool visibility"
        action={
          !creating && (
            <Button onClick={() => setCreating(true)}>
              <Plus className="h-4 w-4 mr-2" /> New Profile
            </Button>
          )
        }
      />

      {/* Create Form */}
      {creating && (
        <Card className="animate-scale-in border-cursor-orange/20">
          <CardContent className="p-4 flex items-center gap-3">
            <Input
              placeholder="Profile name"
              value={newName}
              onChange={(e) => setNewName(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && handleCreate()}
              autoFocus
              className="max-w-sm"
            />
            <Button onClick={handleCreate}>Create</Button>
            <Button
              variant="outline"
              onClick={() => {
                setCreating(false);
                setNewName("");
              }}
            >
              Cancel
            </Button>
          </CardContent>
        </Card>
      )}

      {/* Profiles Grid */}
      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-4">
        {profiles.map((profile, index) => {
          const Icon = profileIcons[index % profileIcons.length];
          const accent = profileAccents[index % profileAccents.length];
          return (
            <Card
              key={profile.id}
              className="group cursor-pointer transition-all duration-200 hover:border-[var(--fg-15)] hover:shadow-[0_12px_40px_-12px_rgba(0,0,0,0.06)]"
              onClick={() => navigate(`/profiles/${profile.id}`)}
            >
              <CardContent className="p-5 relative">
                <div
                  className={cn(
                    "h-10 w-10 rounded-xl flex items-center justify-center border mb-4 transition-colors",
                    accent,
                  )}
                >
                  <Icon className="h-[18px] w-[18px]" />
                </div>
                <h3 className="mb-5 font-headline text-base text-cursor-dark">{profile.name}</h3>
                <div className="flex items-center gap-2 pt-4 border-t border-[var(--fg-06)]">
                  <span className="h-2 w-2 rounded-full bg-success-muted" />
                  <span className="font-body text-xs text-[var(--fg-40)]">
                    {profile.serverCount ?? 0} servers
                  </span>
                </div>
                {/* Actions overlay */}
                <div
                  className="absolute top-4 right-4 flex items-center gap-1 opacity-100 transition-opacity sm:opacity-0 sm:group-hover:opacity-100"
                  onClick={(e) => e.stopPropagation()}
                >
                  <Button
                    variant="ghost"
                    size="icon"
                    className="h-11 w-11 text-[var(--fg-35)] hover:text-error-warm"
                    aria-label={`Delete ${profile.name} profile`}
                    onClick={(e) => {
                      e.stopPropagation();
                      void deleteProfile(profile.id);
                    }}
                  >
                    <Trash2 className="h-4 w-4" />
                  </Button>
                </div>
              </CardContent>
            </Card>
          );
        })}

        {/* New Profile CTA */}
        <button
          onClick={() => setCreating(true)}
          className={cn(
            "rounded-xl border-2 border-dashed border-[var(--fg-12)]",
            "flex flex-col items-center justify-center text-center min-h-[180px] p-5",
            "text-[var(--fg-35)] hover:text-cursor-orange hover:border-cursor-orange/30 hover:bg-cursor-orange/[0.02]",
            "transition-all duration-200 cursor-pointer group",
          )}
        >
          <div className="h-11 w-11 rounded-full bg-surface-300 group-hover:bg-cursor-orange/10 flex items-center justify-center mb-3 transition-colors">
            <Plus className="h-5 w-5" />
          </div>
          <h3 className="font-headline text-sm font-medium text-cursor-dark group-hover:text-cursor-orange transition-colors">
            New Profile
          </h3>
          <p className="font-body text-xs text-[var(--fg-40)] mt-1">Create a server grouping</p>
        </button>
      </div>
    </div>
  );
}
