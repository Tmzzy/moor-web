import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Switch } from "@/components/ui/switch";
import { Badge } from "@/components/ui/badge";
import { Server, FolderOpen } from "lucide-react";
import { useParams, useNavigate } from "react-router-dom";
import { useProfiles, useProfile } from "@/hooks/useProfiles";
import { DetailPageHeader } from "@/components/shared/DetailPageHeader";
import { PageLoading } from "@/components/shared/PageLoading";
import { EmptyState } from "@/components/shared/EmptyState";
import { cn } from "@/lib/utils";
import { McpTokenPanel } from "@/components/security/McpTokenPanel";

export function ProfileDetail() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const { updateProfileServer } = useProfiles();
  const { profile, isLoading: loading, refresh } = useProfile(id);

  if (loading) {
    return <PageLoading message="Loading profile..." />;
  }

  if (!profile || !id) {
    return <EmptyState icon={FolderOpen} message="Profile not found" />;
  }

  const toggleServer = async (serverId: string, enabled: boolean) => {
    await updateProfileServer({ profileId: id, serverId, updates: { enabled } });
    refresh();
  };

  const enabledCount = profile.servers.filter((s) => s.profileServer.enabled).length;

  return (
    <div className="space-y-6 animate-fade-in-up">
      <DetailPageHeader
        title={profile.name}
        subtitle={`${enabledCount} of ${profile.servers.length} servers enabled`}
        onBack={() => navigate("/profiles")}
      />

      <McpTokenPanel key={profile.id} profileId={profile.id} profileName={profile.name} />

      {/* Server Selection */}
      <Card>
        <CardHeader className="pb-3">
          <CardTitle className="text-base flex items-center gap-2">
            <Server className="h-4 w-4 text-[var(--fg-40)]" /> Server Selection
          </CardTitle>
        </CardHeader>
        <CardContent className="space-y-1">
          {profile.servers.length === 0 ? (
            <EmptyState icon={Server} message="No servers available. Add servers first." />
          ) : (
            profile.servers.map((server) => (
              <div
                key={server.id}
                className={cn(
                  "flex items-center justify-between py-3 px-4 rounded-xl transition-all duration-200",
                  server.profileServer.enabled
                    ? "bg-surface-100 border border-[var(--fg-08)]"
                    : "hover:bg-surface-300/40",
                )}
              >
                <div className="flex items-center gap-3">
                  <Switch
                    checked={server.profileServer.enabled}
                    onCheckedChange={(v) => toggleServer(server.id, v)}
                  />
                  <div
                    className={cn(
                      "h-8 w-8 rounded-lg flex items-center justify-center border shrink-0",
                      server.status === "running"
                        ? "bg-success-muted/8 text-success-muted border-success-muted/15"
                        : "bg-surface-300 text-[var(--fg-30)] border-[var(--fg-06)]",
                    )}
                  >
                    <Server className="h-3.5 w-3.5" />
                  </div>
                  <div>
                    <span className="font-headline text-sm text-cursor-dark">{server.name}</span>
                    <Badge variant="subtle" className="ml-2 text-[10px]">
                      {server.connectionType}
                    </Badge>
                  </div>
                </div>
                {server.profileServer.enabled && (
                  <div className="flex items-center gap-1.5">
                    <span className="h-1.5 w-1.5 rounded-full bg-success-muted" />
                    <span className="font-body text-xs text-[var(--fg-40)]">Enabled</span>
                  </div>
                )}
              </div>
            ))
          )}
        </CardContent>
      </Card>
    </div>
  );
}
