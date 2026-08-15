import { useCallback, useState, type SetStateAction } from "react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { api, apiPost, apiPut, apiDelete } from "@/lib/api/client";
import { routes } from "@/lib/api-routes";
import { profileKeys, serverKeys } from "@/lib/query-keys";
import { useSSEEvent } from "@/contexts/SSEContext";
import type {
  Server,
  ServerCreateInput,
  ServerDetail,
  ServerUpdateInput,
  ToolDetail,
} from "@moor/types";
import { getErrorMessage } from "@/lib/utils";
import {
  applyServerAction,
  mergeServerStatusEvent,
  syncUpdatedServerCaches,
  type ServerAction,
  type ServerStatusEventPayload,
} from "./server-patch-utils";

type ServerMutationResult = "success" | "failed";

export function useServerList() {
  const queryClient = useQueryClient();

  const {
    data: servers = [],
    isLoading: loading,
    error,
  } = useQuery<Server[]>({
    queryKey: serverKeys.list(),
    queryFn: ({ signal }) => api<Server[]>(routes.servers.list(), { signal }),
  });

  const [serverActions, setServerActions] = useState<Record<string, ServerAction>>({});

  const setServerAction = useCallback((id: string, action: ServerAction) => {
    setServerActions((prev) => ({ ...prev, [id]: action }));
  }, []);

  const clearServerAction = useCallback((id: string) => {
    setServerActions((prev) => {
      const next = { ...prev };
      delete next[id];
      return next;
    });
  }, []);

  const setData = useCallback(
    (updater: SetStateAction<Server[]>) => {
      queryClient.setQueryData<Server[]>(serverKeys.list(), (old) => {
        const prev = old ?? [];
        return typeof updater === "function" ? updater(prev) : updater;
      });
    },
    [queryClient],
  );

  const mergeStatusEvent = useCallback(
    (eventData: ServerStatusEventPayload) => {
      setData((prev) => mergeServerStatusEvent(prev, eventData));
      if (eventData.status !== "starting") {
        clearServerAction(eventData.serverId);
      }
    },
    [clearServerAction, setData],
  );

  const refreshSilently = useCallback(async () => {
    await queryClient.refetchQueries({ queryKey: serverKeys.list() });
  }, [queryClient]);

  const refresh = useCallback(async () => {
    await queryClient.invalidateQueries({ queryKey: serverKeys.list() });
  }, [queryClient]);

  useSSEEvent("server:status", (data) => mergeStatusEvent(data));
  useSSEEvent("server:tools", () => void refreshSilently());

  return {
    servers,
    loading,
    error: error?.message ?? null,
    refresh,
    refreshSilently,
    serverActions,
    setServerAction,
    clearServerAction,
    setData,
  };
}

export function useServerActions(callbacks?: {
  setServerAction?: (id: string, action: ServerAction) => void;
  clearServerAction?: (id: string) => void;
}) {
  const queryClient = useQueryClient();
  const { setServerAction, clearServerAction } = callbacks ?? {};

  const setServersData = useCallback(
    (updater: SetStateAction<Server[]>) => {
      queryClient.setQueryData<Server[]>(serverKeys.list(), (old) => {
        const prev = old ?? [];
        return typeof updater === "function" ? updater(prev) : updater;
      });
    },
    [queryClient],
  );

  const setServerDetailPatch = useCallback(
    (id: string, patch: Partial<ServerDetail>) => {
      queryClient.setQueryData<ServerDetail>(serverKeys.detail(id), (prev) =>
        prev ? { ...prev, ...patch } : prev,
      );
    },
    [queryClient],
  );

  const refreshAfterServerMutation = useCallback(
    async (id: string) => {
      await queryClient.refetchQueries({ queryKey: serverKeys.list() });
      await queryClient.invalidateQueries({ queryKey: serverKeys.detail(id) });
    },
    [queryClient],
  );

  const addServer = useMutation({
    mutationFn: async (config: ServerCreateInput) => {
      return apiPost<Server>(routes.servers.create(), config);
    },
    onSuccess: (server) => {
      queryClient.setQueryData<Server[]>(serverKeys.list(), (prev) => [...(prev ?? []), server]);
    },
  });

  const startServer = useMutation<ServerMutationResult, Error, string>({
    mutationFn: async (id: string) => {
      setServerAction?.(id, "starting");
      setServersData((prev) => applyServerAction(prev, id, "starting"));
      setServerDetailPatch(id, { status: "starting", errorMessage: null });
      try {
        await apiPost(routes.servers.start(id), {});
        return "success";
      } catch (err) {
        const errorMessage = getErrorMessage(err);
        setServersData((prev) =>
          mergeServerStatusEvent(prev, {
            serverId: id,
            status: "error",
            errorMessage,
          }),
        );
        setServerDetailPatch(id, { status: "error", errorMessage });
        return "failed";
      }
    },
    onSettled: (result, _err, id) => {
      clearServerAction?.(id);
      if (result === "success") void refreshAfterServerMutation(id);
    },
  });

  const stopServer = useMutation<ServerMutationResult, Error, string>({
    mutationFn: async (id: string) => {
      setServerAction?.(id, "stopping");
      setServersData((prev) => applyServerAction(prev, id, "stopping"));
      setServerDetailPatch(id, { errorMessage: null });
      try {
        await apiPost(routes.servers.stop(id), {});
        return "success";
      } catch (err) {
        const errorMessage = getErrorMessage(err);
        setServersData((prev) =>
          mergeServerStatusEvent(prev, {
            serverId: id,
            status: "error",
            errorMessage,
          }),
        );
        setServerDetailPatch(id, { status: "error", errorMessage });
        return "failed";
      }
    },
    onSettled: (result, _err, id) => {
      clearServerAction?.(id);
      if (result === "success") void refreshAfterServerMutation(id);
    },
  });

  const updateServer = useMutation({
    mutationFn: async ({ id, updates }: { id: string; updates: ServerUpdateInput }) => {
      return apiPut<Server>(routes.servers.update(id), updates);
    },
    onSuccess: (updated, { id }) => {
      return syncUpdatedServerCaches(queryClient, updated, id);
    },
  });

  const updateServerProfiles = useMutation({
    mutationFn: async ({ id, profileIds }: { id: string; profileIds: string[] }) => {
      return apiPut<{ profileIds: string[] }>(routes.servers.profiles(id), { profileIds });
    },
    onSuccess: async ({ profileIds }, { id }) => {
      queryClient.setQueryData<ServerDetail>(serverKeys.detail(id), (previous) =>
        previous ? { ...previous, profileIds } : previous,
      );
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: profileKeys.list() }),
        queryClient.invalidateQueries({ queryKey: serverKeys.toolsRoot(id) }),
      ]);
    },
  });

  const removeServer = useMutation({
    mutationFn: async (id: string) => {
      await apiDelete(routes.servers.delete(id));
    },
    onSuccess: (_data, id) => {
      queryClient.setQueryData<Server[]>(serverKeys.list(), (prev) =>
        prev?.filter((s) => s.id !== id),
      );
    },
  });

  const reorderServers = useMutation({
    mutationFn: async (nextServers: Server[]) => {
      const ordered = await apiPut<Server[]>(routes.servers.order(), {
        serverIds: nextServers.map((server) => server.id),
      });
      return ordered;
    },
    onMutate: async (nextServers) => {
      await queryClient.cancelQueries({ queryKey: serverKeys.list() });
      const previous = queryClient.getQueryData<Server[]>(serverKeys.list());
      queryClient.setQueryData<Server[]>(serverKeys.list(), nextServers);
      return { previous };
    },
    onSuccess: (ordered) => {
      queryClient.setQueryData<Server[]>(serverKeys.list(), ordered);
    },
    onError: (_err, _vars, context) => {
      if (context?.previous) {
        queryClient.setQueryData<Server[]>(serverKeys.list(), context.previous);
      }
    },
  });

  return {
    addServer: addServer.mutateAsync,
    updateServer: updateServer.mutateAsync,
    updateServerProfiles: updateServerProfiles.mutateAsync,
    startServer: async (id: string) => {
      await startServer.mutateAsync(id);
    },
    stopServer: async (id: string) => {
      await stopServer.mutateAsync(id);
    },
    removeServer: removeServer.mutateAsync,
    reorderServers: reorderServers.mutateAsync,
  };
}

export function useServers() {
  const list = useServerList();
  const actions = useServerActions({
    setServerAction: list.setServerAction,
    clearServerAction: list.clearServerAction,
  });

  return {
    servers: list.servers,
    loading: list.loading,
    error: list.error,
    refresh: list.refresh,
    refreshSilently: list.refreshSilently,
    serverActions: list.serverActions,
    ...actions,
  };
}

export function useServer(id: string | undefined) {
  const queryClient = useQueryClient();

  const {
    data: server,
    isLoading,
    error,
  } = useQuery<ServerDetail>({
    queryKey: serverKeys.detail(id!),
    queryFn: ({ signal }) => api<ServerDetail>(routes.servers.detail(id!), { signal }),
    enabled: !!id,
  });

  useSSEEvent("server:status", (eventData) => {
    if (eventData.serverId === id) {
      void queryClient.invalidateQueries({ queryKey: serverKeys.detail(id!) });
    }
  });

  return { server, isLoading, error };
}

export function useServerTools(serverId: string | undefined, profileId?: string) {
  const queryClient = useQueryClient();

  const { data: queriedTools = [] } = useQuery<ToolDetail[]>({
    queryKey: serverKeys.tools(serverId!, profileId),
    queryFn: ({ signal }) =>
      api<ToolDetail[]>(routes.servers.tools(serverId!, profileId!), { signal }),
    enabled: !!serverId && !!profileId,
  });

  const refresh = useCallback(() => {
    if (!serverId) return;
    void queryClient.invalidateQueries({ queryKey: serverKeys.toolsRoot(serverId) });
  }, [queryClient, serverId]);

  return { tools: profileId ? queriedTools : [], refresh };
}

export type { ServerAction };
