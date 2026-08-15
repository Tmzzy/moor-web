import { useCallback } from "react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { api, apiPost, apiPut, apiDelete } from "@/lib/api/client";
import { routes } from "@/lib/api-routes";
import { profileKeys } from "@/lib/query-keys";
import type { Profile, ProfileDetail } from "@moor/types";

export function useProfiles() {
  const queryClient = useQueryClient();

  const {
    data: profiles = [],
    isLoading: loading,
    error,
  } = useQuery<Profile[]>({
    queryKey: profileKeys.list(),
    queryFn: ({ signal }) => api<Profile[]>(routes.profiles.list(), { signal }),
  });

  const refresh = useCallback(async () => {
    await queryClient.invalidateQueries({ queryKey: profileKeys.list() });
  }, [queryClient]);

  const createProfile = useMutation({
    mutationFn: (name: string) => apiPost<Profile>(routes.profiles.create(), { name }),
    onSuccess: (profile) => {
      queryClient.setQueryData<Profile[]>(profileKeys.list(), (prev) => [...(prev ?? []), profile]);
    },
  });

  const deleteProfile = useMutation({
    mutationFn: (id: string) => apiDelete(routes.profiles.delete(id)).then(() => id),
    onSuccess: (id) => {
      queryClient.setQueryData<Profile[]>(profileKeys.list(), (prev) =>
        prev?.filter((p) => p.id !== id),
      );
    },
  });

  const updateProfile = useMutation({
    mutationFn: async ({ id, updates }: { id: string; updates: { name?: string } }) => {
      await apiPut(routes.profiles.update(id), updates);
    },
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: profileKeys.list() });
    },
  });

  const updateProfileServer = useMutation({
    mutationFn: async ({
      profileId,
      serverId,
      updates,
    }: {
      profileId: string;
      serverId: string;
      updates: { enabled?: boolean; disabledTools?: string[] };
    }) => {
      await apiPut(routes.profiles.updateServer(profileId, serverId), updates);
    },
  });

  return {
    profiles,
    loading,
    error: error?.message ?? null,
    refresh,
    createProfile: createProfile.mutateAsync,
    deleteProfile: deleteProfile.mutateAsync,
    updateProfile: updateProfile.mutateAsync,
    updateProfileServer: updateProfileServer.mutateAsync,
  };
}

export function useProfile(id: string | undefined) {
  const queryClient = useQueryClient();

  const {
    data: profile,
    isLoading,
    error,
  } = useQuery<ProfileDetail | null>({
    queryKey: profileKeys.detail(id!),
    queryFn: ({ signal }) => api<ProfileDetail | null>(routes.profiles.detail(id!), { signal }),
    enabled: !!id,
  });

  const refresh = useCallback(() => {
    void queryClient.invalidateQueries({ queryKey: profileKeys.detail(id!) });
  }, [queryClient, id]);

  return { profile, isLoading, error, refresh };
}
