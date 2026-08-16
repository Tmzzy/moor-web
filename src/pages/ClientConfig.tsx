// SPDX-License-Identifier: Apache-2.0
// Modified from the original Moor project for this Web/Docker distribution; see NOTICE.

import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { api } from "@/lib/api/client";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Tabs } from "@/components/ui/tabs";
import { CodeBlock } from "@/components/shared/CodeBlock";
import { PageHeader } from "@/components/shared/PageHeader";
import { Terminal } from "lucide-react";
import { cn } from "@/lib/utils";
import { routes } from "@/lib/api-routes";
import { ConverterPanel } from "@/components/converter/ConverterPanel";
import { McpTokenPanel } from "@/components/security/McpTokenPanel";
import { useProfiles } from "@/hooks/useProfiles";
import type { ClientSnippet } from "@moor/types";

export function ClientConfig() {
  const [activeTab, setActiveTab] = useState("snippets");
  const [selectedProfileId, setSelectedProfileId] = useState<string>();
  const { profiles } = useProfiles();
  const { data: snippets } = useQuery<ClientSnippet[]>({
    queryKey: ["snippets"],
    queryFn: () => api<ClientSnippet[]>(routes.import.snippets()),
  });

  const displaySnippets = snippets ?? [];
  const selectedProfile = profiles.find((profile) => profile.id === selectedProfileId);

  return (
    <div className="space-y-6 animate-fade-in-up">
      <PageHeader
        title="Client Configuration"
        subtitle="Configure your AI agents to connect to Moor"
      />

      <div className="flex max-w-md flex-col gap-1.5">
        <Label htmlFor="client-config-profile">Profile</Label>
        <Select value={selectedProfileId} onValueChange={setSelectedProfileId}>
          <SelectTrigger id="client-config-profile" aria-label="MCP token profile">
            <SelectValue placeholder="Select a profile" />
          </SelectTrigger>
          <SelectContent>
            {profiles.map((profile) => (
              <SelectItem key={profile.id} value={profile.id}>
                {profile.name}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </div>

      {selectedProfile ? (
        <McpTokenPanel
          key={selectedProfile.id}
          profileId={selectedProfile.id}
          profileName={selectedProfile.name}
        />
      ) : null}

      <Tabs
        value={activeTab}
        onValueChange={setActiveTab}
        tabs={[
          { value: "snippets", label: "Snippets" },
          { value: "converter", label: "Converter" },
        ]}
      />

      {activeTab === "snippets" && (
        <>
          <div className="grid grid-cols-1 lg:grid-cols-2 gap-5">
            {displaySnippets.map((s, index) => (
              <Card
                key={s.client}
                className={cn(
                  "animate-fade-in-up transition-shadow-smooth hover:shadow-[0_12px_40px_-12px_rgba(0,0,0,0.06)]",
                  `stagger-${index + 1}`,
                )}
              >
                <CardHeader className="pb-3">
                  <div className="flex items-center gap-3">
                    <div className="h-9 w-9 rounded-xl bg-surface-300 border border-[var(--fg-08)] flex items-center justify-center">
                      <Terminal className="h-4 w-4 text-[var(--fg-50)]" />
                    </div>
                    <div>
                      <CardTitle className="text-base">{s.client}</CardTitle>
                      <p className="font-body text-xs text-[var(--fg-45)] mt-0.5">
                        {s.description}
                      </p>
                    </div>
                  </div>
                </CardHeader>
                <CardContent className="space-y-4">
                  <CodeBlock code={s.snippet} label="Configuration" />
                  {s.cliCommand && <CodeBlock code={s.cliCommand} label="CLI Command" />}
                </CardContent>
              </Card>
            ))}
          </div>
        </>
      )}

      {activeTab === "converter" && <ConverterPanel />}
    </div>
  );
}
